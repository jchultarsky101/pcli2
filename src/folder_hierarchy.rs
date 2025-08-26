//! Folder hierarchy management for the Physna CLI client.
//!
//! This module provides functionality for building, managing, and manipulating
//! folder hierarchies retrieved from the Physna API. It includes features for
//! path-based lookups, tree printing, and hierarchical filtering.

use crate::model::FolderResponse;
use crate::physna_v3::PhysnaApiClient;
use ptree::TreeBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::trace;

/// Represents a single folder node in the folder hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderNode {
    /// The folder data from the Physna API
    pub folder: FolderResponse,
    /// UUIDs of child folders
    pub children: Vec<String>,
}

impl FolderNode {
    /// Create a new FolderNode from a FolderResponse
    pub fn new(folder: FolderResponse) -> Self {
        Self {
            folder,
            children: Vec::new(),
        }
    }
    
    /// Get the ID of the folder
    pub fn id(&self) -> &str {
        &self.folder.id
    }
    
    /// Get the name of the folder
    pub fn name(&self) -> &str {
        &self.folder.name
    }
    
    /// Get the parent folder ID, if any
    pub fn parent_id(&self) -> Option<&String> {
        self.folder.parent_folder_id.as_ref()
    }
}

/// Represents the complete folder hierarchy for a tenant
#[derive(Serialize, Deserialize)]
pub struct FolderHierarchy {
    /// Map of folder UUID to FolderNode
    nodes: HashMap<String, FolderNode>,
    /// Root folder IDs (folders with no parent)
    root_ids: Vec<String>,
}

impl FolderHierarchy {
    /// Create a new empty FolderHierarchy
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_ids: Vec::new(),
        }
    }
    
    /// Build a folder hierarchy by fetching all folders from the Physna API
    /// 
    /// This method fetches all folders for a tenant using pagination and constructs
    /// a complete folder hierarchy with parent-child relationships.
    /// 
    /// # Arguments
    /// * `client` - A mutable reference to the Physna API client
    /// * `tenant_id` - The ID of the tenant whose folders to fetch
    /// 
    /// # Returns
    /// * `Ok(FolderHierarchy)` - The complete folder hierarchy for the tenant
    /// * `Err` - If there was an error during API calls or data processing
    pub async fn build_from_api(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut hierarchy = Self::new();
        
        // Fetch all folders using pagination with per_page of 200 for better performance (API max is 1000)
        let mut page = 1;
        let per_page = 200;
        loop {
            trace!("Fetching folder page {} for tenant {} ({} folders so far)", page, tenant_id, hierarchy.nodes.len());
            let response = client.list_folders(tenant_id, Some(page), Some(per_page)).await?;
            
            let folders_on_page = response.folders.len();
            trace!("Fetched {} folders on page {}", folders_on_page, page);
            
            // Add all folders to the hierarchy
            for folder in response.folders {
                let folder_id = folder.id.clone();
                let parent_id = folder.parent_folder_id.clone();
                
                // Create node and add to hierarchy
                let node = FolderNode::new(folder);
                hierarchy.nodes.insert(folder_id.clone(), node);
                
                // If folder has a parent, add it as child to the parent
                if let Some(parent_id) = &parent_id {
                    if let Some(parent_node) = hierarchy.nodes.get_mut(parent_id) {
                        parent_node.children.push(folder_id.clone());
                    }
                }
            }
            
            // Check if we've reached the last page
            // The API uses 1-based indexing for pages
            if response.page_data.current_page >= response.page_data.last_page {
                trace!("Reached last page of folders for tenant {} after {} pages", tenant_id, page);
                break;
            }
            
            page += 1;
        }
        
        // Identify root folders (folders with no parent or parent not in the list)
        for (id, node) in &hierarchy.nodes {
            if node.parent_id().is_none() || !hierarchy.nodes.contains_key(node.parent_id().unwrap()) {
                hierarchy.root_ids.push(id.clone());
            }
        }
        
        // Second pass to add children to parents that might have been processed after their children
        let node_ids: Vec<String> = hierarchy.nodes.keys().cloned().collect();
        let parent_child_relations: Vec<(String, String)> = node_ids
            .iter()
            .filter_map(|id| {
                if let Some(node) = hierarchy.nodes.get(id) {
                    if let Some(parent_id) = node.parent_id() {
                        return Some((parent_id.clone(), id.clone()));
                    }
                }
                None
            })
            .collect();
        
        for (parent_id, child_id) in parent_child_relations {
            if let Some(parent_node) = hierarchy.nodes.get_mut(&parent_id) {
                if !parent_node.children.contains(&child_id) {
                    parent_node.children.push(child_id);
                }
            }
        }
        
        Ok(hierarchy)
    }
    
    /// Convert the folder hierarchy to a flat FolderList
    /// 
    /// This method creates a FolderList with all folders in the hierarchy,
    /// each with its full path computed from the hierarchy.
    /// 
    /// # Returns
    /// A FolderList containing all folders with their computed paths
    pub fn to_folder_list(&self) -> crate::model::FolderList {
        let mut folder_list = crate::model::FolderList::empty();
        
        // Process each node to create folders with proper paths
        for (id, node) in &self.nodes {
            let path = self.get_path_for_folder(id).unwrap_or_else(|| node.name().to_string());
            let folder = crate::model::Folder::from_folder_response(node.folder.clone(), path);
            folder_list.insert(folder);
        }
        
        folder_list
    }
    
    /// Get a folder node by its ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the folder to retrieve
    /// 
    /// # Returns
    /// * `Some(&FolderNode)` - If a folder with the specified ID exists
    /// * `None` - If no folder with the specified ID exists
    pub fn get_folder_by_id(&self, id: &str) -> Option<&FolderNode> {
        self.nodes.get(id)
    }
    
    /// Get a folder node by its path
    /// 
    /// # Arguments
    /// * `path` - The path of the folder to retrieve (e.g., "Root/Child/Grandchild")
    /// 
    /// # Returns
    /// * `Some(&FolderNode)` - If a folder with the specified path exists
    /// * `None` - If no folder with the specified path exists
    pub fn get_folder_by_path(&self, path: &str) -> Option<&FolderNode> {
        if path.is_empty() || path == "/" {
            // Return first root folder if there's only one, otherwise return None
            if self.root_ids.len() == 1 {
                return self.nodes.get(&self.root_ids[0]);
            }
            return None;
        }
        
        let clean_path = if path.starts_with('/') {
            &path[1..]
        } else {
            path
        };
        
        let path_parts: Vec<&str> = clean_path.split('/').collect();
        
        // Start from root folders
        self.find_folder_by_path_parts(&self.root_ids, &path_parts)
    }
    
    /// Find a folder node by path parts recursively
    /// 
    /// # Arguments
    /// * `folder_ids` - The IDs of folders to search within
    /// * `path_parts` - The remaining path parts to match
    /// 
    /// # Returns
    /// * `Some(&FolderNode)` - If a folder matching the path parts is found
    /// * `None` - If no matching folder is found
    fn find_folder_by_path_parts(&self, folder_ids: &[String], path_parts: &[&str]) -> Option<&FolderNode> {
        if path_parts.is_empty() {
            return None;
        }
        
        let current_part = path_parts[0];
        
        // Find folder with matching name among the given folder IDs
        for folder_id in folder_ids {
            if let Some(node) = self.nodes.get(folder_id) {
                if node.name() == current_part {
                    if path_parts.len() == 1 {
                        // Found the target folder
                        return Some(node);
                    } else {
                        // Continue searching in children
                        return self.find_folder_by_path_parts(&node.children, &path_parts[1..]);
                    }
                }
            }
        }
        
        None
    }
    
    /// Get the full path for a folder by its ID
    /// 
    /// # Arguments
    /// * `folder_id` - The ID of the folder whose path to retrieve
    /// 
    /// # Returns
    /// * `Some(String)` - The full path of the folder (e.g., "Root/Child/Grandchild")
    /// * `None` - If no folder with the specified ID exists
    pub fn get_path_for_folder(&self, folder_id: &str) -> Option<String> {
        let mut path_parts = Vec::new();
        let mut current_id = folder_id;
        
        // Traverse up the hierarchy to build the path
        while let Some(node) = self.nodes.get(current_id) {
            path_parts.push(node.name());
            
            if let Some(parent_id) = node.parent_id() {
                current_id = parent_id;
            } else {
                break;
            }
        }
        
        // Reverse the path parts to get the correct order
        path_parts.reverse();
        
        if path_parts.is_empty() {
            None
        } else {
            Some(path_parts.join("/"))
        }
    }
    
    /// Create a new FolderHierarchy containing only the subtree under the specified path
    /// 
    /// # Arguments
    /// * `path` - The path of the folder to use as the root of the new hierarchy
    /// 
    /// # Returns
    /// * `Some(FolderHierarchy)` - A new hierarchy containing only the subtree
    /// * `None` - If no folder exists at the specified path
    pub fn filter_by_path(&self, path: &str) -> Option<FolderHierarchy> {
        // Find the folder node at the specified path
        let target_node = self.get_folder_by_path(path)?;
        
        // Create a new hierarchy with only the subtree
        let mut filtered_hierarchy = FolderHierarchy::new();
        
        // Add the target folder and all its descendants
        self.add_subtree_to_hierarchy(&mut filtered_hierarchy, target_node, true);
        
        Some(filtered_hierarchy)
    }
    
    /// Recursively add a subtree to a hierarchy
    /// 
    /// # Arguments
    /// * `hierarchy` - The hierarchy to add the subtree to
    /// * `node` - The root node of the subtree to add
    /// * `is_root` - Whether this node is the root of the new hierarchy
    fn add_subtree_to_hierarchy(&self, hierarchy: &mut FolderHierarchy, node: &FolderNode, is_root: bool) {
        // Create a new node with adjusted parent relationship
        let mut new_node = node.clone();
        
        // If this is the root of our filtered hierarchy, remove the parent relationship
        if is_root {
            // Create a new folder response with no parent
            let mut new_folder = new_node.folder.clone();
            new_folder.parent_folder_id = None;
            new_node.folder = new_folder;
            
            // Add this node to root_ids since it's the root of our filtered hierarchy
            hierarchy.root_ids.push(node.id().to_string());
        }
        
        // Add the current node
        hierarchy.nodes.insert(node.id().to_string(), new_node);
        
        // Recursively add all children
        for child_id in &node.children {
            if let Some(child_node) = self.nodes.get(child_id) {
                self.add_subtree_to_hierarchy(hierarchy, child_node, false);
            }
        }
    }
    
    /// Print the folder hierarchy as a tree structure
    /// 
    /// This method prints the folder hierarchy to stdout using a tree-like format
    /// with proper indentation to show parent-child relationships.
    pub fn print_tree(&self) {
        // Sort root folders by name
        let mut sorted_roots: Vec<(&String, &FolderNode)> = self.root_ids
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|node| (id, node)))
            .collect();
        sorted_roots.sort_by(|a, b| a.1.name().cmp(b.1.name()));
        
        for (_root_id, node) in sorted_roots {
            let mut tree = TreeBuilder::new(node.name().to_string());
            
            // Build children for this root (sorted by name)
            let mut sorted_children: Vec<(&String, &FolderNode)> = node.children
                .iter()
                .filter_map(|id| self.nodes.get(id).map(|node| (id, node)))
                .collect();
            sorted_children.sort_by(|a, b| a.1.name().cmp(b.1.name()));
            
            for (_child_id, child_node) in sorted_children {
                self.build_tree_node(&mut tree, child_node);
            }
            
            let tree = tree.build();
            // Ignore broken pipe errors (e.g., when piping to head)
            let _ = ptree::print_tree(&tree);
        }
    }
    
    /// Recursively build a tree node for printing
    /// 
    /// # Arguments
    /// * `tree` - The TreeBuilder to add nodes to
    /// * `node` - The current node to process
    fn build_tree_node(&self, tree: &mut TreeBuilder, node: &FolderNode) {
        tree.begin_child(node.name().to_string());
        
        // Sort children by name
        let mut sorted_children: Vec<(&String, &FolderNode)> = node.children
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|node| (id, node)))
            .collect();
        sorted_children.sort_by(|a, b| a.1.name().cmp(b.1.name()));
        
        for (_child_id, child_node) in sorted_children {
            self.build_tree_node(tree, child_node);
        }
        
        tree.end_child();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_folder_hierarchy() {
        let mut hierarchy = FolderHierarchy::new();
        
        // Create test folders
        let root_folder = FolderResponse {
            id: "root-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            name: "Root".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            updated_at: "2023-01-01T00:00:00Z".to_string(),
            assets_count: 0,
            folders_count: 2,
            parent_folder_id: None,
            owner_id: None,
        };
        
        let child1_folder = FolderResponse {
            id: "child-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            name: "Child1".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            updated_at: "2023-01-01T00:00:00Z".to_string(),
            assets_count: 0,
            folders_count: 1,
            parent_folder_id: Some("root-1".to_string()),
            owner_id: None,
        };
        
        let grandchild_folder = FolderResponse {
            id: "grandchild-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            name: "Grandchild1".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            updated_at: "2023-01-01T00:00:00Z".to_string(),
            assets_count: 0,
            folders_count: 0,
            parent_folder_id: Some("child-1".to_string()),
            owner_id: None,
        };
        
        // Add nodes to hierarchy
        hierarchy.nodes.insert("root-1".to_string(), FolderNode::new(root_folder));
        hierarchy.nodes.insert("child-1".to_string(), FolderNode::new(child1_folder));
        hierarchy.nodes.insert("grandchild-1".to_string(), FolderNode::new(grandchild_folder));
        
        // Set up relationships
        hierarchy.root_ids.push("root-1".to_string());
        hierarchy.nodes.get_mut("root-1").unwrap().children.push("child-1".to_string());
        hierarchy.nodes.get_mut("child-1").unwrap().children.push("grandchild-1".to_string());
        
        // Test path lookup
        let folder = hierarchy.get_folder_by_path("Root/Child1/Grandchild1");
        assert!(folder.is_some());
        assert_eq!(folder.unwrap().id(), "grandchild-1");
        
        // Test path building
        let path = hierarchy.get_path_for_folder("grandchild-1");
        assert_eq!(path, Some("Root/Child1/Grandchild1".to_string()));
    }
}
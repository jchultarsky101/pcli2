//! Folder hierarchy management for the Physna CLI client.
//!
//! This module provides functionality for building, managing, and manipulating
//! folder hierarchies retrieved from the Physna API. It includes features for
//! path-based lookups, tree printing, and hierarchical filtering.

use crate::model::FolderResponse;
use crate::physna_v3::{PhysnaApiClient};
use ptree::TreeBuilder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;
use tracing::trace;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FolderHierarchyError {
    #[error("{0}")]
    ApiError(#[from] crate::physna_v3::ApiError),
}

/// Represents a single folder node in the folder hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderNode {
    /// The folder data from the Physna API
    pub folder: FolderResponse,
    /// UUIDs of child folders
    pub children: Vec<Uuid>,
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
    pub fn uuid(&self) -> &Uuid {
        &self.folder.uuid
    }
    
    /// Get the name of the folder
    pub fn name(&self) -> &str {
        &self.folder.name
    }
    
    /// Get the parent folder ID, if any
    pub fn parent_uuid(&self) -> Option<&Uuid> {
        self.folder.parent_folder_uuid.as_ref()
    }
}

/// Represents the complete folder hierarchy for a tenant
#[derive(Serialize, Deserialize, Clone)]
pub struct FolderHierarchy {
    /// Map of folder UUID to FolderNode
    pub nodes: HashMap<Uuid, FolderNode>,
    /// Root folder IDs (folders with no parent)
    pub root_uuids: Vec<Uuid>,
}

impl Default for FolderHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderHierarchy {
    /// Create a new empty FolderHierarchy
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_uuids: Vec::new(),
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
    pub async fn build_from_api(client: &mut PhysnaApiClient, tenant_uuid: &Uuid) -> Result<Self, FolderHierarchyError> {
        let mut hierarchy = Self::new();
        
        // Fetch all folders using pagination with per_page of 200 for better performance (API max is 1000)
        let mut page = 1;
        let per_page = 200;
        loop {
            trace!("Fetching folder page {} for tenant {} ({} folders so far)", page, tenant_uuid.to_string(), hierarchy.nodes.len());
            let response = client.list_folders(tenant_uuid, Some(page), Some(per_page)).await?;
            
            let folders_on_page = response.folders.len();
            trace!("Fetched {} folders on page {}", folders_on_page, page);
            
            // Add all folders to the hierarchy
            for folder in response.folders {
                let folder_uuid = folder.uuid.clone();
                let parent_uuid = folder.parent_folder_uuid.clone();
                
                // Create node and add to hierarchy
                let node = FolderNode::new(folder);
                hierarchy.nodes.insert(folder_uuid.clone(), node);
                
                // If folder has a parent, add it as child to the parent
                if let Some(parent_uuid) = &parent_uuid {
                    if let Some(parent_node) = hierarchy.nodes.get_mut(parent_uuid) {
                        parent_node.children.push(folder_uuid.clone());
                    }
                } else {
                    // No parent - this is a root folder
                    hierarchy.root_uuids.push(folder_uuid.clone());
                }
            }
            
            // Check if we've reached the last page
            // The API uses 1-based indexing for pages
            if response.page_data.current_page >= response.page_data.last_page {
                trace!("Reached last page of folders for tenant {} after {} pages", tenant_uuid, page);
                break;
            }
            
            page += 1;
        }
        
        // Second pass to add children to parents that might have been processed after their children
        let node_uuids: Vec<Uuid> = hierarchy.nodes.keys().cloned().collect();
        let parent_child_relations: Vec<(Uuid, Uuid)> = node_uuids
            .iter()
            .filter_map(|uuid| {
                if let Some(node) = hierarchy.nodes.get(uuid) {
                    if let Some(parent_uuid) = node.parent_uuid() {
                        return Some((parent_uuid.clone(), uuid.clone()));
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
    
    /// Convert the folder hierarchy to a flat FolderList containing only direct children of the root folder
    /// 
    /// This method creates a FolderList with only the direct children of the root folders
    /// in this hierarchy, rather than all folders. This is useful for efficient folder listing
    /// when only immediate children are needed.
    /// 
    /// # Returns
    /// A FolderList containing only direct children with their computed paths
    pub fn to_direct_children_list(&self) -> crate::model::FolderList {
        let mut folder_list = crate::model::FolderList::empty();
        
        // For each root folder, add it to the list
        for root_uuid in &self.root_uuids {
            if let Some(root_node) = self.nodes.get(root_uuid) {
                // Add the root folder itself
                let root_path = self.get_path_for_folder(root_uuid).unwrap_or_else(|| root_node.name().to_string());
                let root_folder = crate::model::Folder::from_folder_response(root_node.folder.clone(), root_path);
                folder_list.add(root_folder);
            }
        }
        
        folder_list
    }
    
    /// Get direct children of a folder by path
    /// 
    /// This method returns a FolderList containing only the direct children of the specified folder path.
    /// This is useful for listing only immediate subfolders without recursively listing all descendants.
    /// 
    /// # Arguments
    /// * `folder_path` - The path of the folder whose children to retrieve
    /// 
    /// # Returns
    /// A FolderList containing only the direct children of the specified folder
    pub fn get_children_by_path(&self, folder_path: &str) -> Option<crate::model::FolderList> {
        // Find the folder node at the specified path
        let target_node = self.get_folder_by_path(folder_path)?;
        
        let mut folder_list = crate::model::FolderList::empty();
        
        // Add only the direct children of this folder
        for child_uuid in &target_node.children {
            if let Some(child_node) = self.nodes.get(child_uuid) {
                let child_path = self.get_path_for_folder(child_uuid).unwrap_or_else(|| child_node.name().to_string());
                let child_folder = crate::model::Folder::from_folder_response(child_node.folder.clone(), child_path);
                folder_list.add(child_folder);
            }
        }
        
        Some(folder_list)
    }
    
    /// Get a folder node by its ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the folder to retrieve
    /// 
    /// # Returns
    /// * `Some(&FolderNode)` - If a folder with the specified ID exists
    /// * `None` - If no folder with the specified ID exists
    pub fn get_folder_by_uuid(&self, uuid: &Uuid) -> Option<&FolderNode> {
        self.nodes.get(uuid)
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
            let clean_path = path.strip_prefix('/').unwrap_or(path);
            let path_parts: Vec<&str> = clean_path.split('/').collect();
        
            // Start from root folders
            self.find_folder_by_path_parts(&self.root_uuids, &path_parts)
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
    fn find_folder_by_path_parts(&self, folder_ids: &[Uuid], path_parts: &[&str]) -> Option<&FolderNode> {
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
    pub fn get_path_for_folder(&self, folder_uuid: &Uuid) -> Option<String> {
        let mut path_parts = Vec::new();
        let mut current_uuid = folder_uuid;
        
        // Traverse up the hierarchy to build the path
        while let Some(node) = self.nodes.get(current_uuid) {
            path_parts.push(node.name());
            
            if let Some(parent_uuid) = node.parent_uuid() {
                current_uuid = parent_uuid;
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
            new_folder.parent_folder_uuid = None;
            new_node.folder = new_folder;
            
            // Add this node to root_ids since it's the root of our filtered hierarchy
            hierarchy.root_uuids.push(node.uuid().clone());
        }
        
        // Add the current node
        hierarchy.nodes.insert(node.uuid().clone(), new_node);
        
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
        let mut sorted_roots: Vec<(&Uuid, &FolderNode)> = self.root_uuids
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|node| (id, node)))
            .collect();
        sorted_roots.sort_by(|a, b| a.1.name().cmp(b.1.name()));
        
        for (_root_id, node) in sorted_roots {
            let mut tree = TreeBuilder::new(node.name().to_string());
            
            // Build children for this root (sorted by name)
            let mut sorted_children: Vec<(&Uuid, &FolderNode)> = node.children
                .iter()
                .filter_map(|uuid| self.nodes.get(uuid).map(|node| (uuid, node)))
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
        let mut sorted_children: Vec<(&Uuid, &FolderNode)> = node.children
            .iter()
            .filter_map(|uuid| self.nodes.get(uuid).map(|node| (uuid, node)))
            .collect();
        sorted_children.sort_by(|a, b| a.1.name().cmp(b.1.name()));
        
        for (_child_id, child_node) in sorted_children {
            self.build_tree_node(tree, child_node);
        }
        
        tree.end_child();
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
            folder_list.add(folder);
        }
        
        folder_list
    }
}


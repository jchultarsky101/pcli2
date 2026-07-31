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
use thiserror::Error;
use tracing::trace;
use uuid::Uuid;

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
    pub async fn build_from_api(
        client: &mut PhysnaApiClient,
        tenant_uuid: &Uuid,
    ) -> Result<Self, FolderHierarchyError> {
        let mut hierarchy = Self::new();

        // Fetch all folders using pagination with per_page of 200 for better performance (API max is 1000)
        let mut page = 1;
        let per_page = 200;
        loop {
            trace!(
                "Fetching folder page {} for tenant {} ({} folders so far)",
                page,
                tenant_uuid.to_string(),
                hierarchy.nodes.len()
            );
            let response = client
                .list_folders(tenant_uuid, Some(page), Some(per_page))
                .await?;

            let folders_on_page = response.folders.len();
            trace!("Fetched {} folders on page {}", folders_on_page, page);

            // Add all folders to the hierarchy
            for folder in response.folders {
                let folder_uuid = folder.uuid;
                let parent_uuid = folder.parent_folder_uuid;

                // Create node and add to hierarchy
                let node = FolderNode::new(folder);
                hierarchy.nodes.insert(folder_uuid, node);

                // If folder has a parent, add it as child to the parent
                if let Some(parent_uuid) = &parent_uuid {
                    if let Some(parent_node) = hierarchy.nodes.get_mut(parent_uuid) {
                        parent_node.children.push(folder_uuid);
                    }
                } else {
                    // No parent - this is a root folder
                    hierarchy.root_uuids.push(folder_uuid);
                }
            }

            // Check if we've reached the last page
            // The API uses 1-based indexing for pages
            if response.page_data.current_page >= response.page_data.last_page {
                trace!(
                    "Reached last page of folders for tenant {} after {} pages",
                    tenant_uuid,
                    page
                );
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
                        return Some((*parent_uuid, *uuid));
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
                let root_path = self
                    .get_path_for_folder(root_uuid)
                    .unwrap_or_else(|| root_node.name().to_string());
                let root_folder =
                    crate::model::Folder::from_folder_response(root_node.folder.clone(), root_path);
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
                let child_path = self
                    .get_path_for_folder(child_uuid)
                    .unwrap_or_else(|| child_node.name().to_string());
                let child_folder = crate::model::Folder::from_folder_response(
                    child_node.folder.clone(),
                    child_path,
                );
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

    /// Collect the UUIDs of a folder and every folder beneath it, breadth-first.
    ///
    /// The starting folder is always the first element. Folders already visited are
    /// skipped, so a malformed hierarchy containing a cycle terminates rather than
    /// looping forever.
    ///
    /// # Arguments
    /// * `root_uuid` - The folder to start from
    ///
    /// # Returns
    /// The starting folder's UUID followed by all of its descendants. Empty if the
    /// starting folder is not part of this hierarchy.
    pub fn subtree_uuids(&self, root_uuid: &Uuid) -> Vec<Uuid> {
        if !self.nodes.contains_key(root_uuid) {
            return Vec::new();
        }

        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut ordered = Vec::new();

        queue.push_back(*root_uuid);
        visited.insert(*root_uuid);

        while let Some(uuid) = queue.pop_front() {
            ordered.push(uuid);
            if let Some(node) = self.nodes.get(&uuid) {
                for child in &node.children {
                    if visited.insert(*child) {
                        queue.push_back(*child);
                    }
                }
            }
        }

        ordered
    }

    /// Collect the UUIDs of every folder in the hierarchy, breadth-first from each root.
    ///
    /// Used when an operation targets the tenant root, which has no folder UUID of its
    /// own but conceptually contains every folder.
    pub fn all_subtree_uuids(&self) -> Vec<Uuid> {
        let mut visited = std::collections::HashSet::new();
        let mut ordered = Vec::new();

        for root_uuid in &self.root_uuids {
            for uuid in self.subtree_uuids(root_uuid) {
                if visited.insert(uuid) {
                    ordered.push(uuid);
                }
            }
        }

        ordered
    }

    /// Get a folder node by its path
    ///
    /// # Arguments
    /// * `path` - The path of the folder to retrieve (e.g., "Root/Child/Grandchild")
    ///
    /// # Returns
    /// * `Some(&FolderNode)` - If a folder with the specified path exists
    /// * `None` - If no folder with the specified path exists
    pub fn get_node_by_path(&self, path: &str) -> Option<&FolderNode> {
        self.get_folder_by_path(path)
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
        // Handle the root path specially
        if path == "/" {
            // For root path, we can't return a single node since it represents all root folders
            // This case is handled specially in filter_by_path
            return None;
        }

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
    fn find_folder_by_path_parts(
        &self,
        folder_ids: &[Uuid],
        path_parts: &[&str],
    ) -> Option<&FolderNode> {
        if path_parts.is_empty() {
            return None;
        }

        let current_part = path_parts[0];

        // Find folder with matching name among the given folder IDs
        // Use case-insensitive comparison for better cross-platform compatibility
        // (Windows users expect case-insensitive folder matching)
        for folder_id in folder_ids {
            if let Some(node) = self.nodes.get(folder_id) {
                if node.name().eq_ignore_ascii_case(current_part) {
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
        // Handle the root path specially
        if path == "/" {
            // For root path, return the entire hierarchy
            return Some(self.clone());
        }

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
    fn add_subtree_to_hierarchy(
        &self,
        hierarchy: &mut FolderHierarchy,
        node: &FolderNode,
        is_root: bool,
    ) {
        // Create a new node with adjusted parent relationship
        let mut new_node = node.clone();

        // If this is the root of our filtered hierarchy, remove the parent relationship
        if is_root {
            // Create a new folder response with no parent
            let mut new_folder = new_node.folder.clone();
            new_folder.parent_folder_uuid = None;
            new_node.folder = new_folder;

            // Add this node to root_ids since it's the root of our filtered hierarchy
            hierarchy.root_uuids.push(*node.uuid());
        }

        // Add the current node
        hierarchy.nodes.insert(*node.uuid(), new_node);

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
        let mut sorted_roots: Vec<(&Uuid, &FolderNode)> = self
            .root_uuids
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|node| (id, node)))
            .collect();
        sorted_roots.sort_by(|a, b| a.1.name().cmp(b.1.name()));

        for (_root_id, node) in sorted_roots {
            let mut tree = TreeBuilder::new(node.name().to_string());

            // Build children for this root (sorted by name)
            let mut sorted_children: Vec<(&Uuid, &FolderNode)> = node
                .children
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
        let mut sorted_children: Vec<(&Uuid, &FolderNode)> = node
            .children
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
            let path = self
                .get_path_for_folder(id)
                .unwrap_or_else(|| node.name().to_string());
            let folder = crate::model::Folder::from_folder_response(node.folder.clone(), path);
            folder_list.add(folder);
        }

        folder_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_case_insensitive_folder_matching() {
        // Create a folder hierarchy with mixed-case folder names
        let mut nodes = HashMap::new();

        // Create root folder "Photos and Models"
        let root_folder = FolderResponse {
            uuid: Uuid::new_v4(),
            tenant_uuid: Uuid::new_v4(),
            name: "Photos and Models".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            assets_count: 0,
            folders_count: 0,
            parent_folder_uuid: None,
            owner_id: None,
        };

        let root_uuid = root_folder.uuid;
        nodes.insert(root_uuid, FolderNode::new(root_folder));

        let hierarchy = FolderHierarchy {
            nodes,
            root_uuids: vec![root_uuid],
        };

        // Test exact case match
        assert!(hierarchy.get_folder_by_path("Photos and Models").is_some());

        // Test lowercase match
        assert!(hierarchy.get_folder_by_path("photos and models").is_some());

        // Test uppercase match
        assert!(hierarchy.get_folder_by_path("PHOTOS AND MODELS").is_some());

        // Test mixed case match
        assert!(hierarchy.get_folder_by_path("photos AND Models").is_some());

        // Test with leading slash
        assert!(hierarchy.get_folder_by_path("/Photos and Models").is_some());
        assert!(hierarchy.get_folder_by_path("/photos and models").is_some());
    }

    #[test]
    fn test_nested_case_insensitive_matching() {
        // Create a nested folder hierarchy
        let mut nodes = HashMap::new();

        let parent_folder = FolderResponse {
            uuid: Uuid::new_v4(),
            tenant_uuid: Uuid::new_v4(),
            name: "Parent".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            assets_count: 0,
            folders_count: 1,
            parent_folder_uuid: None,
            owner_id: None,
        };

        let parent_uuid = parent_folder.uuid;

        let child_folder = FolderResponse {
            uuid: Uuid::new_v4(),
            tenant_uuid: Uuid::new_v4(),
            name: "Child Folder".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            assets_count: 0,
            folders_count: 0,
            parent_folder_uuid: Some(parent_uuid),
            owner_id: None,
        };

        let child_uuid = child_folder.uuid;

        nodes.insert(parent_uuid, FolderNode::new(parent_folder));
        nodes.insert(child_uuid, FolderNode::new(child_folder));

        // Add child to parent's children list
        if let Some(parent_node) = nodes.get_mut(&parent_uuid) {
            parent_node.children.push(child_uuid);
        }

        let hierarchy = FolderHierarchy {
            nodes,
            root_uuids: vec![parent_uuid],
        };

        // Test nested path with different cases
        assert!(hierarchy
            .get_folder_by_path("Parent/Child Folder")
            .is_some());
        assert!(hierarchy
            .get_folder_by_path("parent/child folder")
            .is_some());
        assert!(hierarchy
            .get_folder_by_path("PARENT/CHILD FOLDER")
            .is_some());
        assert!(hierarchy
            .get_folder_by_path("parent/Child Folder")
            .is_some());
    }

    /// Build a folder with the given name, parent and children, for traversal tests.
    fn folder(name: &str, parent: Option<Uuid>) -> FolderResponse {
        FolderResponse {
            uuid: Uuid::new_v4(),
            tenant_uuid: Uuid::new_v4(),
            name: name.to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            assets_count: 0,
            folders_count: 0,
            parent_folder_uuid: parent,
            owner_id: None,
        }
    }

    /// Assemble a hierarchy from `(node, children)` pairs.
    fn hierarchy_of(folders: Vec<FolderResponse>, edges: &[(Uuid, Uuid)]) -> FolderHierarchy {
        let mut nodes = HashMap::new();
        let mut root_uuids = Vec::new();

        for folder in folders {
            if folder.parent_folder_uuid.is_none() {
                root_uuids.push(folder.uuid);
            }
            nodes.insert(folder.uuid, FolderNode::new(folder));
        }

        for (parent, child) in edges {
            nodes.get_mut(parent).unwrap().children.push(*child);
        }

        FolderHierarchy { nodes, root_uuids }
    }

    #[test]
    fn an_unknown_folder_has_an_empty_subtree() {
        // The signal a recursive scan uses to detect a stale cache: the path resolved
        // against the API, but the folder is absent from the cached hierarchy, so its
        // descendants cannot be enumerated. Returning empty here must be treated as an
        // error by the caller rather than as "this folder has no assets" - a folder
        // holding 22,378 assets across 511 subfolders once reported 1.
        let hierarchy = FolderHierarchy::default();
        assert!(hierarchy.subtree_uuids(&Uuid::new_v4()).is_empty());
    }

    #[test]
    fn subtree_uuids_includes_the_folder_and_every_descendant() {
        // Regression: folder match reports listed only a folder's direct assets, so a
        // container folder such as "Creo Files" - which holds nothing but subfolders -
        // reported zero assets even though its subtree was full of them.
        let root = folder("Creo Files", None);
        let root_uuid = root.uuid;
        let child = folder("Demo - Local", Some(root_uuid));
        let child_uuid = child.uuid;
        let grandchild = folder("Nested", Some(child_uuid));
        let grandchild_uuid = grandchild.uuid;
        let sibling = folder("Creo", None);
        let sibling_uuid = sibling.uuid;

        let hierarchy = hierarchy_of(
            vec![root, child, grandchild, sibling],
            &[(root_uuid, child_uuid), (child_uuid, grandchild_uuid)],
        );

        let subtree = hierarchy.subtree_uuids(&root_uuid);
        assert_eq!(subtree.len(), 3);
        assert_eq!(subtree[0], root_uuid, "the folder itself comes first");
        assert!(subtree.contains(&child_uuid));
        assert!(subtree.contains(&grandchild_uuid));
        assert!(
            !subtree.contains(&sibling_uuid),
            "an unrelated root folder must not be pulled in"
        );

        // A leaf folder is its own complete subtree.
        assert_eq!(
            hierarchy.subtree_uuids(&grandchild_uuid),
            vec![grandchild_uuid]
        );

        // Every root's subtree, with no duplicates.
        let all = hierarchy.all_subtree_uuids();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn subtree_uuids_handles_unknown_folders_and_cycles() {
        assert!(FolderHierarchy::new()
            .subtree_uuids(&Uuid::new_v4())
            .is_empty());

        // A malformed hierarchy whose children loop back must terminate rather than
        // recursing forever.
        let a = folder("a", None);
        let a_uuid = a.uuid;
        let b = folder("b", Some(a_uuid));
        let b_uuid = b.uuid;
        let hierarchy = hierarchy_of(vec![a, b], &[(a_uuid, b_uuid), (b_uuid, a_uuid)]);

        assert_eq!(hierarchy.subtree_uuids(&a_uuid), vec![a_uuid, b_uuid]);
    }
}

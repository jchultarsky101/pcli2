//! Efficient folder path resolution cache.
//! 
//! This module provides caching for folder path resolution to avoid repeated API calls
//! when traversing folder hierarchies. It caches the contents of each folder level
//! to make subsequent path lookups more efficient.

use crate::model::FolderResponse;
use crate::physna_v3::PhysnaApiClient;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache entry for a specific folder's contents
#[derive(Debug, Clone)]
struct FolderContents {
    /// The timestamp when this cache entry was created
    timestamp: u64,
    /// The subfolders in this folder
    subfolders: Vec<FolderResponse>,
}

/// Cache for folder path resolution to avoid repeated API calls
pub struct FolderPathCache {
    /// Map of folder ID to folder contents
    contents: HashMap<String, FolderContents>,
    /// Map of folder path to folder ID for quick resolution
    path_to_id: HashMap<String, String>,
}

impl FolderPathCache {
    /// Default cache expiration time in seconds (24 hours)
    const DEFAULT_CACHE_EXPIRATION: u64 = 24 * 60 * 60;

    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            contents: HashMap::new(),
            path_to_id: HashMap::new(),
        }
    }

    /// Check if a cache entry is expired
    fn is_expired(&self, timestamp: u64) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        current_time - timestamp > Self::DEFAULT_CACHE_EXPIRATION
    }

    /// Get the folder ID by resolving a path efficiently
    pub async fn get_folder_id_by_path(
        &mut self,
        client: &mut PhysnaApiClient,
        tenant_id: &str,
        path: &str,
    ) -> Result<Option<String>, crate::physna_v3::ApiError> {
        // Normalize the path by removing leading slash
        let normalized_path = path.strip_prefix('/').unwrap_or(path);
        if normalized_path.is_empty() {
            // Root path - get root contents
            return self.resolve_root_path(client, tenant_id).await;
        }

        // Split the path into components
        let path_parts: Vec<&str> = normalized_path.split('/').collect();
        
        // Start with root
        let mut current_folder_id: Option<String> = None;
        let mut current_path = String::new();
        
        for (i, part) in path_parts.iter().enumerate() {
            let path_segment = if current_path.is_empty() {
                part.to_string()
            } else {
                format!("{}/{}", current_path, part)
            };
            
            // Check if we have cached contents for the parent folder
            let parent_folder_id = current_folder_id.as_deref();
            let subfolders = self.get_subfolders_for_parent(
                client, 
                tenant_id, 
                parent_folder_id, 
            ).await?;
            
            // Find the folder with the target name
            let target_folder = subfolders.iter()
                .find(|folder| &folder.name == part);
            
            if let Some(folder) = target_folder {
                if i == path_parts.len() - 1 {
                    // This is the final component, we found the target folder
                    self.path_to_id.insert(normalized_path.to_string(), folder.id.clone());
                    return Ok(Some(folder.id.clone()));
                } else {
                    // Move to the next level
                    current_folder_id = Some(folder.id.clone());
                    current_path = path_segment;
                }
            } else {
                // Folder not found in this path
                return Ok(None);
            }
        }
        
        Ok(None)
    }

    /// Get subfolders for a parent folder, using cache or API as needed
    async fn get_subfolders_for_parent(
        &mut self,
        client: &mut PhysnaApiClient,
        tenant_id: &str,
        parent_folder_id: Option<&str>,
    ) -> Result<Vec<FolderResponse>, crate::physna_v3::ApiError> {
        // If parent_folder_id is None, this is the root level
        let cache_key = parent_folder_id.unwrap_or("ROOT").to_string();

        // Check if we have cached data for this parent and if it's not expired
        if let Some(contents) = self.contents.get(&cache_key) {
            if !self.is_expired(contents.timestamp) {
                return Ok(contents.subfolders.clone());
            }
            // Cache is expired, fall through to fetch from API
        }

        // Fetch from API
        let subfolders = if parent_folder_id.is_none() {
            // Get root contents
            let response = client.get_root_contents(tenant_id, "folders", Some(1), Some(1000)).await?;
            response.folders
        } else {
            // Get contents of a specific folder
            let response = client.get_folder_contents(tenant_id, parent_folder_id.unwrap(), "folders", Some(1), Some(1000)).await?;
            response.folders
        };

        // Cache the result
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let contents = FolderContents {
            timestamp: now,
            subfolders: subfolders.clone(),
        };
        
        self.contents.insert(cache_key, contents);

        Ok(subfolders)
    }

    /// Resolve the root path by getting root contents
    async fn resolve_root_path(
        &mut self,
        client: &mut PhysnaApiClient,
        tenant_id: &str,
    ) -> Result<Option<String>, crate::physna_v3::ApiError> {
        // For root path, get root contents which will have no parent
        let subfolders = self.get_subfolders_for_parent(client, tenant_id, None).await?;
        
        if subfolders.len() == 1 {
            // If there's only one root folder, return it
            Ok(Some(subfolders[0].id.clone()))
        } else if subfolders.is_empty() {
            // No root folders
            Ok(None)
        } else {
            // Multiple root folders - return the first one (or could error/ask user)
            Ok(Some(subfolders[0].id.clone()))
        }
    }
}

impl Default for FolderPathCache {
    fn default() -> Self {
        Self::new()
    }
}
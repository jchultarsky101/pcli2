//! Folder caching functionality for the Physna CLI client.
//!
//! This module provides functionality for caching folder hierarchies to improve
//! performance by reducing API calls. It uses bincode serialization for efficient
//! storage and retrieval of folder data.

use crate::folder_hierarchy::FolderHierarchy;
use crate::physna_v3::PhysnaApiClient;
use std::fs;
use std::path::PathBuf;
use bincode;

/// Manages caching of folder hierarchies for Physna tenants
pub struct FolderCache;

impl FolderCache {
    /// Get the cache directory path
    /// 
    /// In a test environment (when PCLI2_TEST_CACHE_DIR is set), it uses that directory.
    /// Otherwise, it uses the system's cache directory with a "pcli2/folder_cache" subdirectory.
    pub fn get_cache_dir() -> PathBuf {
        // Check if we're in a test environment
        if let Ok(test_cache_dir) = std::env::var("PCLI2_TEST_CACHE_DIR") {
            PathBuf::from(test_cache_dir).join("folder_cache")
        } else {
            let cache_dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
            cache_dir.join("pcli2").join("folder_cache")
        }
    }
    
    /// Get the cache file path for a specific tenant
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cache file path to retrieve
    /// 
    /// # Returns
    /// The full path to the tenant's cache file
    pub fn get_cache_file_path(tenant_id: &str) -> PathBuf {
        Self::get_cache_dir().join(format!("{}.bin", tenant_id))
    }
    
    /// Load cached folder hierarchy for a tenant
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cached folder hierarchy to load
    /// 
    /// # Returns
    /// * `Some(FolderHierarchy)` - If a valid cache file exists for the tenant
    /// * `None` - If no cache file exists or if deserialization fails
    pub fn load(tenant_id: &str) -> Option<FolderHierarchy> {
        let cache_file = Self::get_cache_file_path(tenant_id);
        
        if cache_file.exists() {
            match fs::read(&cache_file) {
                Ok(data) => {
                    bincode::deserialize::<FolderHierarchy>(&data).ok()
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }
    
    /// Save folder hierarchy to cache for a tenant
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant to cache the folder hierarchy for
    /// * `hierarchy` - The folder hierarchy to cache
    /// 
    /// # Returns
    /// * `Ok(())` - If the folder hierarchy was successfully cached
    /// * `Err` - If there was an error during serialization or file operations
    pub fn save(tenant_id: &str, hierarchy: &FolderHierarchy) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = bincode::serialize(hierarchy)?;
        
        // Create cache directory if it doesn't exist
        let cache_dir = Self::get_cache_dir();
        fs::create_dir_all(&cache_dir)?;
        
        let cache_file = Self::get_cache_file_path(tenant_id);
        fs::write(cache_file, serialized)?;
        
        Ok(())
    }
    
    /// Get folder hierarchy from cache or fetch from API if not available/cached
    /// 
    /// This method first attempts to load the folder hierarchy from cache. If it's not
    /// available in cache, it fetches the data from the Physna API and caches it.
    /// 
    /// # Arguments
    /// * `client` - A mutable reference to the Physna API client
    /// * `tenant_id` - The ID of the tenant whose folder hierarchy to retrieve
    /// 
    /// # Returns
    /// * `Ok(FolderHierarchy)` - The folder hierarchy for the tenant
    /// * `Err` - If there was an error during cache operations or API calls
    pub async fn get_or_fetch(
        client: &mut PhysnaApiClient,
        tenant_id: &str,
    ) -> Result<FolderHierarchy, Box<dyn std::error::Error>> {
        // Try to load from cache first
        if let Some(cached) = Self::load(tenant_id) {
            // For now, we'll always use the cached version since we can't check expiration
            // without serializing the timestamp
            return Ok(cached);
        }
        
        // If not in cache, fetch from API
        let hierarchy = FolderHierarchy::build_from_api(client, tenant_id).await?;
        
        // Save to cache
        if let Err(e) = Self::save(tenant_id, &hierarchy) {
            tracing::warn!("Failed to cache folder hierarchy: {}", e);
        }
        
        Ok(hierarchy)
    }
    
    /// Refresh the cache for a specific tenant (force fetch from API)
    /// 
    /// This method always fetches the latest folder hierarchy from the Physna API
    /// and updates the cache, regardless of whether valid cached data exists.
    /// 
    /// # Arguments
    /// * `client` - A mutable reference to the Physna API client
    /// * `tenant_id` - The ID of the tenant whose folder hierarchy to refresh
    /// 
    /// # Returns
    /// * `Ok(FolderHierarchy)` - The refreshed folder hierarchy for the tenant
    /// * `Err` - If there was an error during the API call or cache operations
    pub async fn refresh(
        client: &mut PhysnaApiClient,
        tenant_id: &str,
    ) -> Result<FolderHierarchy, Box<dyn std::error::Error>> {
        let hierarchy = FolderHierarchy::build_from_api(client, tenant_id).await?;
        
        // Save to cache
        if let Err(e) = Self::save(tenant_id, &hierarchy) {
            tracing::warn!("Failed to cache folder hierarchy: {}", e);
        }
        
        Ok(hierarchy)
    }
    
    /// Invalidate cache for a specific tenant
    /// 
    /// This method removes the cached folder hierarchy for the specified tenant.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cache to invalidate
    /// 
    /// # Returns
    /// * `Ok(())` - If the cache was successfully invalidated or didn't exist
    /// * `Err` - If there was an error during file operations
    pub fn invalidate(tenant_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let cache_file = Self::get_cache_file_path(tenant_id);
        if cache_file.exists() {
            fs::remove_file(cache_file)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_folder_cache_get_cache_dir() {
        // Test that we can get the cache directory path
        let cache_dir = FolderCache::get_cache_dir();
        assert!(cache_dir.ends_with("pcli2/folder_cache"));
    }

    #[test]
    fn test_folder_cache_get_cache_file_path() {
        // Test that we can get the cache file path for a tenant
        let cache_file = FolderCache::get_cache_file_path("test-tenant");
        assert!(cache_file.ends_with("pcli2/folder_cache/test-tenant.bin"));
    }

    #[test]
    fn test_folder_cache_invalidate_nonexistent() {
        // Test that we can invalidate a cache file that doesn't exist
        let temp_dir = TempDir::new().unwrap();
        
        // Temporarily override the cache directory
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());
        
        // This should not panic or return an error
        let result = FolderCache::invalidate("nonexistent-tenant");
        assert!(result.is_ok());
        
        // Clean up
        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }
}
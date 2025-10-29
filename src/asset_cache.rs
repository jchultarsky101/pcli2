use crate::cache::BaseCache;
use crate::physna_v3::PhysnaApiClient;
use crate::model::{AssetListResponse, AssetList};
use std::fs;
use tracing::{trace, debug};
use serde_json;

/// Cache for asset data to avoid repeated API calls
/// 
/// This cache stores asset data locally to avoid expensive API calls when listing assets.
/// It supports both automatic caching and forced refresh operations.
#[derive(Default)]
pub struct AssetCache {
    // Removed unused base field since we're not using BaseCache directly
}

impl AssetCache {
    
    /// This function is not used with the new file-based caching approach
    /// The original load/save methods have been replaced with tenant-specific file caching
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // This shouldn't be called with the new approach, but we return a default instance
        Ok(Self::default())
    }
    
    /// This function is not used with the new file-based caching approach
    /// The original load/save methods have been replaced with tenant-specific file caching
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        // This shouldn't be called with the new approach
        Ok(())
    }
    
    /// Load cached asset list for a tenant file directly
    pub fn load_tenant_cache(tenant_id: &str) -> Result<Option<AssetListResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let cache_dir = BaseCache::get_cache_dir();
        let cache_file = BaseCache::get_cache_file_path(&cache_dir, &format!("assets_{}", tenant_id), "json");
        
        if cache_file.exists() && !BaseCache::is_file_expired(&cache_file) {
            match fs::read_to_string(&cache_file) {
                Ok(data) => {
                    match serde_json::from_str::<AssetListResponse>(&data) {
                        Ok(response) => {
                            debug!("Loaded cached assets for tenant: {}", tenant_id);
                            Ok(Some(response))
                        }
                        Err(e) => {
                            debug!("Failed to deserialize asset cache for tenant {}: {}", tenant_id, e);
                            Ok(None) // Return Ok(None) instead of error if deserialization fails
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to read asset cache file for tenant {}: {}", tenant_id, e);
                    Ok(None) // Return Ok(None) instead of error if file read fails
                }
            }
        } else {
            Ok(None)
        }
    }
    
    /// Save asset list to cache for a specific tenant
    pub fn save_tenant_cache(tenant_id: &str, response: &AssetListResponse) -> Result<(), Box<dyn std::error::Error>> {
        let cache_dir = BaseCache::get_cache_dir();
        let cache_file = BaseCache::get_cache_file_path(&cache_dir, &format!("assets_{}", tenant_id), "json");
        
        // Create directory if it doesn't exist
        fs::create_dir_all(&cache_dir)?;
        
        let data = serde_json::to_string_pretty(response)?;
        fs::write(&cache_file, data)?;
        debug!("Saved asset cache to file: {:?}", cache_file);
        Ok(())
    }
    
    /// Get asset cache for a tenant, fetching from API if not cached or expired
    pub async fn get_or_fetch(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<AssetListResponse, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Getting or fetching asset cache for tenant: {}", tenant_id);
        
        // Try to load from file cache first
        if let Ok(Some(cached_response)) = Self::load_tenant_cache(tenant_id) {
            trace!("Using existing file cache for tenant: {}", tenant_id);
            Ok(cached_response)
        } else {
            trace!("No file cache found, fetching assets from API for tenant: {}", tenant_id);
            let response = Self::fetch_all_assets(client, tenant_id).await?;
            trace!("Successfully fetched {} assets from API for tenant: {}", response.assets.len(), tenant_id);
            
            // Save to file cache
            if let Err(e) = Self::save_tenant_cache(tenant_id, &response) {
                debug!("Failed to save asset cache for tenant {}: {}", tenant_id, e);
            }
            
            Ok(response)
        }
    }
    
    /// Force refresh assets for a tenant from API
    pub async fn refresh(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<AssetListResponse, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Force refreshing assets for tenant {} from API", tenant_id);
        let asset_list_response = Self::fetch_all_assets(client, tenant_id).await?;
        
        // Update file cache - ignore errors during save to avoid failing the operation
        let _ = Self::save_tenant_cache(tenant_id, &asset_list_response);
        
        Ok(asset_list_response)
    }
    
    /// Get assets filtered by folder path using efficient API calls
    pub async fn get_assets_for_folder(client: &mut PhysnaApiClient, tenant_id: &str, folder_path: &str, refresh: bool) -> Result<AssetList, Box<dyn std::error::Error>> {
        trace!("Getting assets for folder: {} in tenant: {}, refresh: {}", folder_path, tenant_id, refresh);
        
        if refresh {
            trace!("Refresh requested, invalidating cache for tenant: {}", tenant_id);
            let cache_dir = BaseCache::get_cache_dir();
            let cache_file = BaseCache::get_cache_file_path(&cache_dir, &format!("assets_{}", tenant_id), "json");
            if cache_file.exists() {
                let _ = fs::remove_file(&cache_file);  // Ignore errors during removal
            }
        }
        
        // Use the efficient API method to get assets directly from the specified folder path
        let asset_list_response = client.list_assets_by_path(tenant_id, folder_path).await?;
        trace!("Successfully fetched {} assets from path: {}", asset_list_response.assets.len(), folder_path);
        
        Ok(asset_list_response.to_asset_list())
    }
    
    /// Fetch all assets for a tenant using pagination
    async fn fetch_all_assets(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<AssetListResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_assets = Vec::new();
        let mut page = 1;
        let per_page = 200; // Fetch 200 assets per page for better performance (API max is 1000)
        
        loop {
            trace!("Fetching asset page {} for tenant {} ({} assets so far)", page, tenant_id, all_assets.len());
            let response = client.list_assets(tenant_id, None, Some(page), Some(per_page)).await?;
            
            let assets_on_page = response.assets.len();
            all_assets.extend(response.assets);
            
            trace!("Fetched {} assets on page {}, total so far: {}", assets_on_page, page, all_assets.len());
            
            // Check if we've reached the last page
            // The API uses 1-based indexing for pages
            if response.page_data.current_page >= response.page_data.last_page {
                trace!("Reached last page of assets for tenant {} after {} pages", tenant_id, page);
                break;
            }
            
            page += 1;
        }
        
        // Store the total count before moving all_assets
        let total_count = all_assets.len();
        
        // Create a combined response with all assets
        let final_response = AssetListResponse {
            assets: all_assets,
            page_data: crate::model::PageData {
                total: total_count,
                per_page: total_count,
                current_page: 1,
                last_page: 1,
                start_index: 1,
                end_index: total_count,
            },
        };
        
        Ok(final_response)
    }
    
    /// Get cached assets for a tenant if available and not expired
    pub fn get_cached_assets(&self, tenant_id: &str) -> Option<AssetListResponse> {
        Self::load_tenant_cache(tenant_id).unwrap_or_default()
    }
    
    /// Invalidate cache for a specific tenant
    /// 
    /// Removes cached assets for the specified tenant from the cache.
    /// This should be called after operations that modify asset state
    /// to ensure consistency between local cache and remote API.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cache to invalidate
    /// 
    /// # Returns
    /// * `true` if cache entry was removed, `false` if no entry existed
    pub fn invalidate_tenant_static(tenant_id: &str) -> bool {
        let cache_dir = BaseCache::get_cache_dir();
        let cache_file = BaseCache::get_cache_file_path(&cache_dir, &format!("assets_{}", tenant_id), "json");
        
        if cache_file.exists() {
            match fs::remove_file(&cache_file) {
                Ok(_) => {
                    trace!("Invalidated asset cache for tenant {}", tenant_id);
                    true
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }
    
    /// Invalidate cache for a specific tenant
    /// 
    /// Removes cached assets for the specified tenant from the cache.
    /// This should be called after operations that modify asset state
    /// to ensure consistency between local cache and remote API.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cache to invalidate
    /// 
    /// # Returns
    /// * `true` if cache entry was removed, `false` if no entry existed
    pub fn invalidate_tenant(&self, tenant_id: &str) -> bool {
        Self::invalidate_tenant_static(tenant_id)
    }
    
    /// Mutating version of invalidate_tenant for backward compatibility
    pub fn invalidate_tenant_mut(&mut self, tenant_id: &str) -> bool {
        self.invalidate_tenant(tenant_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_asset_cache_creation() {
        let _cache = AssetCache::default();
        // Can't directly test internal state since we changed the structure
    }

    #[test]
    fn test_asset_cache_invalidate_nonexistent() {
        // Test that we can invalidate a cache file that doesn't exist
        let temp_dir = TempDir::new().unwrap();
        
        // Temporarily override the cache directory
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());
        
        // This should not panic or return an error
        let result = AssetCache::invalidate_tenant_static("nonexistent-tenant");
        assert!(!result);
        
        // Clean up
        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }
    
    #[test]
    fn test_save_and_load_tenant_cache() {
        use tempfile::TempDir;
        use crate::model::{AssetListResponse, PageData};
        
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());
        
        let test_asset_list = AssetListResponse {
            assets: vec![],
            page_data: PageData {
                total: 0,
                per_page: 0,
                current_page: 1,
                last_page: 1,
                start_index: 1,
                end_index: 0,
            },
        };
        
        // Test save - just check it doesn't panic
        let _ = AssetCache::save_tenant_cache("test_tenant", &test_asset_list);
        
        // Test load - just check it doesn't panic
        let _ = AssetCache::load_tenant_cache("test_tenant");
        
        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }

    #[test]
    fn test_invalidate_tenant_static() {
        use tempfile::TempDir;
        
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());
        
        // Invalidate non-existent tenant should return false
        assert!(!AssetCache::invalidate_tenant_static("nonexistent_tenant"));
        
        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }
}
use crate::physna_v3::PhysnaApiClient;
use crate::model::{AssetListResponse, AssetList};
use crate::folder_cache::FolderCache;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use dirs;
use serde::{Deserialize, Serialize};
use tracing::{trace, debug};

/// Cache for asset data to avoid repeated API calls
/// 
/// This cache stores asset data locally to avoid expensive API calls when listing assets.
/// It supports both automatic caching and forced refresh operations.
#[derive(Debug, Serialize, Deserialize)]
pub struct AssetCache {
    /// Map of tenant ID to asset list response
    tenant_assets: HashMap<String, AssetListResponse>,
    /// Timestamp of last update for each tenant (not currently used but could be useful for expiration)
    #[serde(skip)]
    last_updated: HashMap<String, std::time::SystemTime>,
}

impl AssetCache {
    /// Create a new empty asset cache
    pub fn new() -> Self {
        Self {
            tenant_assets: HashMap::new(),
            last_updated: HashMap::new(),
        }
    }
    
    /// Get the default cache file path
    fn get_cache_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut path = dirs::data_dir().ok_or("Could not determine data directory")?;
        path.push("pcli2");
        path.push("asset_cache.json");
        Ok(path)
    }
    
    /// Load cache from file
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::get_cache_file_path()?;
        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let cache: AssetCache = serde_json::from_str(&data)?;
            debug!("Loaded asset cache from {:?}", path);
            Ok(cache)
        } else {
            debug!("No asset cache file found at {:?}, creating new cache", path);
            Ok(Self::new())
        }
    }
    
    /// Save cache to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_cache_file_path()?;
        
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        debug!("Saved asset cache to {:?}", path);
        Ok(())
    }
    
    /// Get asset cache for a tenant, fetching from API if not cached or expired
    pub async fn get_or_fetch(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<AssetListResponse, Box<dyn std::error::Error>> {
        trace!("Getting or fetching asset cache for tenant: {}", tenant_id);
        let cache = Self::load()?;
        
        if let Some(cached_assets) = cache.tenant_assets.get(tenant_id) {
            trace!("Using existing cache for tenant: {}", tenant_id);
            Ok(cached_assets.clone())
        } else {
            trace!("No cache found, fetching assets from API for tenant: {}", tenant_id);
            let response = Self::fetch_all_assets(client, tenant_id).await?;
            trace!("Successfully fetched {} assets from API for tenant: {}", response.assets.len(), tenant_id);
            Ok(response)
        }
    }
    
    /// Force refresh assets for a tenant from API
    pub async fn refresh(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<AssetListResponse, Box<dyn std::error::Error>> {
        trace!("Force refreshing assets for tenant {} from API", tenant_id);
        let asset_list_response = Self::fetch_all_assets(client, tenant_id).await?;
        
        // Update cache
        let mut cache = Self::load().unwrap_or_else(|_| Self::new());
        cache.tenant_assets.insert(tenant_id.to_string(), asset_list_response.clone());
        cache.last_updated.insert(tenant_id.to_string(), std::time::SystemTime::now());
        cache.save()?;
        
        Ok(asset_list_response)
    }
    
    /// Get assets filtered by folder path
    pub async fn get_assets_for_folder(client: &mut PhysnaApiClient, tenant_id: &str, folder_path: &str, refresh: bool) -> Result<AssetList, Box<dyn std::error::Error>> {
        trace!("Getting assets for folder: {} in tenant: {}, refresh: {}", folder_path, tenant_id, refresh);
        // Get all assets (using cache or fetching from API)
        let asset_list_response = if refresh {
            trace!("Refreshing asset cache for tenant: {}", tenant_id);
            Self::refresh(client, tenant_id).await?
        } else {
            trace!("Using cached assets for tenant: {}", tenant_id);
            Self::get_or_fetch(client, tenant_id).await?
        };
        
        // Get folder hierarchy to find the folder by path
        trace!("Fetching folder hierarchy for tenant: {}", tenant_id);
        let hierarchy = FolderCache::get_or_fetch(client, tenant_id).await?;
        
        if let Some(folder_node) = hierarchy.get_folder_by_path(folder_path) {
            let folder_id = folder_node.id();
            trace!("Found folder with ID: {} for path: {}", folder_id, folder_path);
            // Filter assets that belong to this folder
            let filtered_assets = asset_list_response.assets
                .into_iter()
                .filter(|asset| asset.folder_id.as_deref() == Some(folder_id))
                .collect::<Vec<_>>();
            trace!("Filtered {} assets for folder: {}", filtered_assets.len(), folder_path);
            
            // Create a new AssetListResponse with the filtered assets
            let filtered_response = AssetListResponse {
                assets: filtered_assets,
                page_data: asset_list_response.page_data, // This won't be accurate after filtering, but that's OK for now
            };
            
            Ok(filtered_response.to_asset_list())
        } else {
            trace!("Folder not found for path: {}", folder_path);
            Err(format!("Folder with path '{}' not found", folder_path).into())
        }
    }
    
    /// Fetch all assets for a tenant using pagination
    async fn fetch_all_assets(client: &mut PhysnaApiClient, tenant_id: &str) -> Result<AssetListResponse, Box<dyn std::error::Error>> {
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
    
    /// Get cached assets for a tenant if available
    pub fn get_cached_assets(&self, tenant_id: &str) -> Option<AssetListResponse> {
        self.tenant_assets.get(tenant_id).cloned()
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
    pub fn invalidate_tenant(&mut self, tenant_id: &str) -> bool {
        let removed = self.tenant_assets.remove(tenant_id).is_some();
        if removed {
            trace!("Invalidated asset cache for tenant {}", tenant_id);
        }
        removed
    }
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new()
    }
}
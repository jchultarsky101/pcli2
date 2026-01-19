//! Cache for tenant information to avoid repeated API calls
//!
//! This cache stores tenant data locally to avoid expensive API calls when
//! checking tenant information. It caches the list of available tenants
//! for the current user.

use std::path::PathBuf;
use tracing::{debug, trace, warn};
use crate::model::TenantSetting;
use crate::cache::CacheError;

/// Cache for tenant information to avoid repeated API calls
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TenantCache {
    /// Map of tenant UUID to cached tenant setting
    pub tenants: Vec<TenantSetting>,
    /// Unix timestamp when the cache was last updated (for future use)
    #[serde(rename = "lastUpdated")]
    pub last_updated_timestamp: Option<u64>,
}

impl TenantCache {
    /// Create a new empty tenant cache
    pub fn new() -> Self {
        Self {
            tenants: Vec::new(),
            last_updated_timestamp: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()),
        }
    }

    /// Check if cache is expired (default expiration: 1 hour)
    pub fn is_expired(&self) -> bool {
        if let Some(timestamp) = self.last_updated_timestamp {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            // Cache expires after 1 hour (3600 seconds)
            now - timestamp > 3600
        } else {
            true // If no timestamp, treat as expired
        }
    }

    /// Get the default cache file path
    ///
    /// # Returns
    /// * `Ok(PathBuf)` - The path to the tenant cache file
    /// * `Err` - If there was an error getting the cache directory
    fn get_cache_file_path() -> Result<PathBuf, CacheError> {
        // Load configuration to get the active environment name
        let configuration = crate::configuration::Configuration::load_or_create_default()
            .map_err(|e| CacheError::Other(format!("Could not load configuration: {}", e)))?;

        let environment_name = configuration.get_active_environment()
            .unwrap_or_else(|| "default".to_string());

        // Sanitize environment name to be a valid filename
        let sanitized_env_name = environment_name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");

        // Check for PCLI2_CACHE_DIR environment variable first
        if let Ok(cache_dir_str) = std::env::var("PCLI2_CACHE_DIR") {
            let mut cache_path = PathBuf::from(cache_dir_str);
            cache_path.push(format!("tenant_cache_{}.json", sanitized_env_name));
            return Ok(cache_path);
        }

        let mut path = dirs::cache_dir().ok_or_else(|| {
            CacheError::Other("Could not determine cache directory".to_string())
        })?;
        path.push("pcli2");
        path.push(format!("tenant_cache_{}.json", sanitized_env_name));
        Ok(path)
    }

    /// Load cache from file
    ///
    /// # Returns
    /// * `Ok(TenantCache)` - The loaded cache
    /// * `Err` - If there was an error loading the cache
    pub fn load() -> Result<TenantCache, CacheError> {
        let path = Self::get_cache_file_path()?;

        if !path.exists() {
            debug!("No tenant cache file found, creating new cache");
            return Ok(Self::new());
        }

        let data = std::fs::read_to_string(path)?;
        let cache: TenantCache = serde_json::from_str(&data)?;

        debug!("Loaded tenant cache from file");
        Ok(cache)
    }

    /// Save cache to file
    ///
    /// # Returns
    /// * `Ok(())` - If the cache was successfully saved
    /// * `Err` - If there was an error saving the cache
    pub fn save(&self) -> Result<(), CacheError> {
        let path = Self::get_cache_file_path()?;
        
        // Create cache directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let cache_to_save = Self {
            tenants: self.tenants.clone(),
            last_updated_timestamp: self.last_updated_timestamp,
        };

        let data = serde_json::to_string_pretty(&cache_to_save)?;
        std::fs::write(path, data)?;
        debug!("Saved tenant cache to file");
        Ok(())
    }

    /// Get all tenants from cache, fetching from API if not cached or expired
    ///
    /// # Arguments
    /// * `api` - Reference to the API client to use for fetching if needed
    /// * `refresh` - Whether to force refresh the cache
    ///
    /// # Returns
    /// * `Ok(Vec<TenantSetting>)` - The list of tenants (from cache or API)
    pub async fn get_all_tenants(
        api: &mut crate::physna_v3::PhysnaApiClient,
        refresh: bool,
    ) -> Result<Vec<TenantSetting>, crate::physna_v3::ApiError> {
        if refresh {
            trace!("Force refresh requested, fetching tenants from API");
            let tenants = api.list_tenants().await?;
            // Update cache
            let mut cache = Self::load().unwrap_or_else(|_| Self::new());
            cache.tenants = tenants.clone();
            cache.last_updated_timestamp = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs());
            if let Err(e) = cache.save() {
                warn!("Failed to save tenant cache: {}", e);
            }
            return Ok(tenants);
        }

        let mut cache = Self::load().unwrap_or_else(|_| Self::new());

        // Check if we have cached data and if it's still valid (not expired)
        if !cache.tenants.is_empty() && !cache.is_expired() {
            trace!("Using existing cache for tenants");
            return Ok(cache.tenants.clone());
        }

        trace!("Cache expired or empty, fetching tenants from API");
        let tenants = api.list_tenants().await?;

        // Update cache
        cache.tenants = tenants.clone();
        cache.last_updated_timestamp = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs());
        if let Err(e) = cache.save() {
            warn!("Failed to save tenant cache: {}", e);
        }

        Ok(tenants)
    }

    /// Get cached tenants if available and not expired
    ///
    /// # Returns
    /// * `Some(Vec<TenantSetting>)` - If tenants are cached and not expired
    /// * `None` - If tenants are not cached or are expired
    pub fn get_cached_tenants(&self) -> Option<Vec<TenantSetting>> {
        if !self.tenants.is_empty() && !self.is_expired() {
            Some(self.tenants.clone())
        } else {
            None
        }
    }

    /// Invalidate cache for all tenants
    ///
    /// This method clears the cached tenant list.
    pub fn invalidate_all() -> Result<(), CacheError> {
        let path = Self::get_cache_file_path()?;
        if path.exists() {
            std::fs::remove_file(&path)?;
            trace!("Invalidated all tenant cache");
        }
        Ok(())
    }
}

impl Default for TenantCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_cache_creation() {
        let cache = TenantCache::new();
        assert!(cache.tenants.is_empty());
        assert!(cache.last_updated_timestamp.is_some());
    }

    #[test]
    fn test_tenant_cache_is_expired_initially_false() {
        let cache = TenantCache::new();
        // A new cache shouldn't be expired immediately
        assert!(!cache.is_expired());
    }

    #[test]
    fn test_tenant_cache_creation_has_timestamp() {
        let cache = TenantCache::new();
        assert!(cache.last_updated_timestamp.is_some());
    }
}
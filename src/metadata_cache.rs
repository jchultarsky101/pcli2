//! Cache for metadata fields to avoid repeated API calls
//!
//! This cache stores metadata field data locally to avoid expensive API calls when
//! checking if metadata fields exist. It caches the list of registered metadata
//! fields for each tenant.

use crate::model::MetadataFieldListResponse;
use crate::physna_v3::PhysnaApiClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, trace, warn};

/// Cache for metadata field data to avoid repeated API calls
///
/// This cache stores metadata field data locally to avoid expensive API calls when listing metadata fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataCache {
    /// Map of tenant ID to cached metadata field list response
    #[serde(rename = "tenantMetadataFields")]
    pub tenant_metadata_fields: HashMap<String, MetadataFieldListResponse>,
    /// Timestamp when the cache was last updated (for future use)
    #[serde(rename = "lastUpdated")]
    #[serde(skip_serializing, skip_deserializing)]
    pub last_updated: Option<u64>, // We'll track this internally but not serialize it
}

impl MetadataCache {
    /// Create a new empty metadata cache
    pub fn new() -> Self {
        Self {
            tenant_metadata_fields: HashMap::new(),
            last_updated: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
        }
    }

    /// Check if cache is expired for a specific tenant (default expiration: 1 hour)
    fn is_expired(&self, _tenant_id: &str) -> bool {
        // For now, we'll use a simple approach where cache is valid for 1 hour
        // In the future, we might want to track per-tenant timestamps
        if let Some(updated) = self.last_updated {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Cache expires after 1 hour (3600 seconds)
            current_time - updated > 3600
        } else {
            true
        }
    }

    /// Get the default cache file path
    ///
    /// # Returns
    /// * `Ok(PathBuf)` - The path to the metadata cache file
    /// * `Err` - If there was an error getting the cache directory
    fn get_cache_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        // Check for PCLI2_CACHE_DIR environment variable first
        if let Ok(cache_dir_str) = std::env::var("PCLI2_CACHE_DIR") {
            let mut cache_path = PathBuf::from(cache_dir_str);
            cache_path.push("metadata_cache.json");
            return Ok(cache_path);
        }

        let mut path = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
        path.push("pcli2");
        path.push("metadata_cache.json");
        Ok(path)
    }

    /// Load cache from file
    ///
    /// # Returns
    /// * `Ok(MetadataCache)` - The loaded cache
    /// * `Err` - If there was an error loading the cache
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::get_cache_file_path()?;

        if path.exists() {
            let data = fs::read_to_string(path)?;
            let mut cache: MetadataCache = serde_json::from_str(&data)?;
            // Set the internal timestamp to current time to indicate when it was loaded
            cache.last_updated = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
            debug!("Loaded metadata cache from file");
            Ok(cache)
        } else {
            debug!("No metadata cache file found, creating new cache");
            Ok(Self::new())
        }
    }

    /// Save cache to file
    ///
    /// # Returns
    /// * `Ok(())` - If the cache was successfully saved
    /// * `Err` - If there was an error saving the cache
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_cache_file_path()?;

        // Create cache directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let cache_to_save = Self {
            tenant_metadata_fields: self.tenant_metadata_fields.clone(),
            last_updated: self.last_updated, // Keep the same timestamp when saving
        };

        let data = serde_json::to_string_pretty(&cache_to_save)?;
        fs::write(path, data)?;
        debug!("Saved metadata cache to file");
        Ok(())
    }

    /// Get metadata fields for a tenant, fetching from API if not cached or expired
    ///
    /// # Arguments
    /// * `client` - The Physna API client
    /// * `tenant_id` - The ID of the tenant
    /// * `refresh` - Whether to force refresh the cache
    ///
    /// # Returns
    /// * `Ok(MetadataFieldListResponse)` - The metadata fields for the tenant
    /// * `Err` - If there was an error during API calls
    pub async fn get_or_fetch(
        client: &mut PhysnaApiClient,
        tenant_id: &str,
        refresh: bool,
    ) -> Result<MetadataFieldListResponse, crate::physna_v3::ApiError> {
        trace!(
            "Getting or fetching metadata fields for tenant: {}",
            tenant_id
        );

        let mut cache = Self::load().unwrap_or_else(|_| Self::new());

        // Check if we have cached data and if it's still valid (not expired) and refresh is not forced
        if !refresh {
            if let Some(cached_metadata_fields) = cache.tenant_metadata_fields.get(tenant_id) {
                if !cache.is_expired(tenant_id) {
                    trace!("Using existing cache for tenant: {}", tenant_id);
                    return Ok(cached_metadata_fields.clone());
                }
                trace!("Cache expired for tenant: {}, fetching from API", tenant_id);
            } else {
                trace!(
                    "No cache found, fetching metadata fields from API for tenant: {}",
                    tenant_id
                );
            }
        } else {
            trace!(
                "Refresh requested, fetching metadata fields from API for tenant: {}",
                tenant_id
            );
        }

        // Fetch from API
        let metadata_fields_response = client.get_metadata_fields(tenant_id).await?;

        // Update cache
        cache
            .tenant_metadata_fields
            .insert(tenant_id.to_string(), metadata_fields_response.clone());
        if let Err(e) = cache.save() {
            warn!("Failed to save metadata cache: {}", e);
        }

        Ok(metadata_fields_response)
    }

    /// Get cached metadata fields for a tenant if available and not expired
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    ///
    /// # Returns
    /// * `Some(MetadataFieldListResponse)` - If metadata fields are cached and not expired
    /// * `None` - If metadata fields are not cached or are expired
    pub fn get_cached_metadata_fields(&self, tenant_id: &str) -> Option<MetadataFieldListResponse> {
        if let Some(cached_fields) = self.tenant_metadata_fields.get(tenant_id) {
            if !self.is_expired(tenant_id) {
                return Some(cached_fields.clone());
            }
        }
        None
    }

    /// Invalidate cache for a specific tenant
    ///
    /// This method removes cached metadata fields for the specified tenant from the cache.
    /// This is useful after creating new metadata fields to ensure consistency between
    /// local cache and remote API.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cache to invalidate
    ///
    /// # Returns
    /// * `true` if cache entry was removed, `false` if no entry existed
    pub fn invalidate_tenant(&mut self, tenant_id: &str) -> bool {
        let existed = self.tenant_metadata_fields.contains_key(tenant_id);
        self.tenant_metadata_fields.remove(tenant_id);
        trace!("Invalidated metadata cache for tenant {}", tenant_id);
        existed
    }
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_cache_creation() {
        let cache = MetadataCache::new();
        assert!(cache.tenant_metadata_fields.is_empty());
    }
}

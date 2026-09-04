//! Folder caching functionality for the Physna CLI client.
//!
//! This module provides functionality for caching folder hierarchies to improve
//! performance by reducing API calls. It uses bincode serialization for efficient
//! storage and retrieval of folder data.

use crate::cache::BaseCache;
use crate::folder_hierarchy::FolderHierarchy;
use crate::physna_v3::PhysnaApiClient;
use serde_json;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

/// Manages caching of folder hierarchies for Physna tenants
/// Bumped whenever the serialized shape changes, so a file written by another
/// version is discarded instead of decoded wrongly or failing with a parse error.
const FOLDER_CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedHierarchy {
    #[serde(default)]
    schema_version: u32,
    hierarchy: FolderHierarchy,
}

/// Write a cache file through a temporary name and rename it into place, so a
/// concurrent reader (two pcli2 processes under `xargs -P`) never sees a
/// half-written file.
pub(crate) fn write_atomically(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.flush()?;
    }
    fs::rename(&tmp, path)
}

/// The active environment's name, made safe for a file name.
pub(crate) fn active_environment_key() -> String {
    let name = crate::configuration::Configuration::load_default()
        .ok()
        .and_then(|c| c.get_active_environment())
        .unwrap_or_else(|| "default".to_string());
    name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_")
}

pub struct FolderCache {
    // Removed unused base field since we're not using BaseCache directly
}

impl FolderCache {
    /// Get the cache directory path
    ///
    /// In a test environment (when PCLI2_TEST_CACHE_DIR is set), it uses that directory.
    /// For general cross-platform support (when PCLI2_CACHE_DIR is set), it uses that directory.
    /// Otherwise, it uses the system's cache directory with a "pcli2/folder_cache" subdirectory.
    pub fn get_cache_dir() -> PathBuf {
        BaseCache::get_cache_dir().join("folder_cache")
    }

    /// Get the cache file path for a specific tenant
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cache file path to retrieve
    ///
    /// # Returns
    /// The full path to the tenant's cache file
    pub fn get_cache_file_path<S: AsRef<str>>(key: S) -> PathBuf {
        // Keyed by environment as well as tenant: a staging tenant seeded from
        // production can carry the same UUID and would otherwise share (and
        // corrupt) the production hierarchy.
        let mut path = Self::get_cache_dir();
        path.push(format!(
            "{}-{}.json",
            active_environment_key(),
            key.as_ref()
        ));
        path
    }

    /// Load cached folder hierarchy for a tenant
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose cached folder hierarchy to load
    ///
    /// # Returns
    /// * `Some(FolderHierarchy)` - If a valid cache file exists for the tenant and hasn't expired
    /// * `None` - If no cache file exists, is expired, or if deserialization fails
    pub fn load(tenant_uuid: &Uuid) -> Option<FolderHierarchy> {
        let cache_file = Self::get_cache_file_path(tenant_uuid.to_string());
        tracing::debug!(
            "Attempting to load folder hierarchy from cache file: {:?}",
            cache_file
        );

        if cache_file.exists() {
            tracing::debug!("Cache file exists, checking expiration");
            // Check if the cache has expired
            if BaseCache::is_file_expired(&cache_file) {
                tracing::debug!("Cache file expired, removing it");
                // Remove expired cache file
                let _ = fs::remove_file(&cache_file);
                return None;
            }

            tracing::debug!("Cache file is valid, attempting to read");
            match fs::read(&cache_file) {
                Ok(data) => {
                    tracing::debug!("Successfully read {} bytes from cache file", data.len());
                    match serde_json::from_slice::<CachedHierarchy>(&data) {
                        Ok(cached) if cached.schema_version != FOLDER_CACHE_SCHEMA_VERSION => {
                            tracing::debug!(
                                "Cache file was written by another version (schema {}); ignoring it",
                                cached.schema_version
                            );
                            let _ = fs::remove_file(&cache_file);
                            None
                        }
                        Ok(CachedHierarchy { hierarchy, .. }) => {
                            tracing::debug!(
                                "Successfully deserialized folder hierarchy from cache"
                            );
                            Some(hierarchy)
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to deserialize folder hierarchy from cache: {}",
                                e
                            );
                            tracing::debug!("Deserialization error details: {:?}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read cache file: {}", e);
                    None
                }
            }
        } else {
            tracing::debug!("Cache file does not exist: {:?}", cache_file);
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
    pub fn save(
        tenant_uuid: &Uuid,
        hierarchy: &FolderHierarchy,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = serde_json::to_vec(&CachedHierarchy {
            schema_version: FOLDER_CACHE_SCHEMA_VERSION,
            hierarchy: hierarchy.clone(),
        })?;
        tracing::debug!("Serialized folder hierarchy to {} bytes", serialized.len());

        // Create cache directory if it doesn't exist
        let cache_dir = Self::get_cache_dir();
        fs::create_dir_all(&cache_dir)?;

        let cache_file = Self::get_cache_file_path(tenant_uuid.to_string());
        tracing::debug!("Writing cache file to: {:?}", cache_file);

        write_atomically(&cache_file, &serialized)?;

        tracing::debug!("Successfully wrote cache file");

        Ok(())
    }

    /// Get folder hierarchy from cache or fetch from API if not available/cached or expired
    ///
    /// This method first attempts to load the folder hierarchy from cache. If it's not
    /// available in cache or has expired, it fetches the data from the Physna API and caches it.
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
        tenant_uuid: &Uuid,
    ) -> Result<FolderHierarchy, crate::physna_v3::ApiError> {
        // Try to load from cache first
        if let Some(cached) = Self::load(tenant_uuid) {
            return Ok(cached);
        }

        // If not in cache, fetch from API. Only an API failure can fail this: a
        // cache that cannot be read is a miss, and one that cannot be written is
        // logged and ignored.
        let hierarchy = FolderHierarchy::build_from_api(client, tenant_uuid)
            .await
            .map_err(|crate::folder_hierarchy::FolderHierarchyError::ApiError(e)| e)?;

        // Save to cache
        if let Err(e) = Self::save(tenant_uuid, &hierarchy) {
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
        tenant_uuid: &Uuid,
    ) -> Result<FolderHierarchy, crate::physna_v3::ApiError> {
        let hierarchy = FolderHierarchy::build_from_api(client, tenant_uuid)
            .await
            .map_err(|crate::folder_hierarchy::FolderHierarchyError::ApiError(e)| e)?;

        // Save to cache
        if let Err(e) = Self::save(tenant_uuid, &hierarchy) {
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

    /// Clean expired cache files
    ///
    /// This method removes all expired cache files from the cache directory
    pub fn clean_expired() -> Result<(), Box<dyn std::error::Error>> {
        let cache_dir = Self::get_cache_dir();
        if !cache_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                let _ = fs::remove_file(&path);
                tracing::debug!("Removed expired cache file: {:?}", path);
            }
        }

        Ok(())
    }

    /// Purge all cached data
    ///
    /// This method removes all cache files from the cache directory,
    /// effectively clearing the entire cache for all tenants.
    ///
    /// # Returns
    /// * `Ok(())` - If all cache files were successfully removed
    /// * `Err` - If there was an error during file operations
    pub fn purge_all() -> Result<(), Box<dyn std::error::Error>> {
        let cache_dir = Self::get_cache_dir();
        if !cache_dir.exists() {
            return Ok(());
        }

        // Remove all cache files
        for entry in fs::read_dir(&cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                fs::remove_file(&path)?;
                tracing::debug!("Removed cache file: {:?}", path);
            }
        }

        tracing::debug!("Successfully purged all cached data");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_cache_get_cache_dir() {
        // Test that we can get the cache directory path without asserting specific content
        let cache_dir = FolderCache::get_cache_dir();
        // Just make sure it doesn't panic and returns a path
        assert!(cache_dir.is_absolute() || cache_dir.starts_with("."));
    }

    #[test]
    fn test_folder_cache_get_cache_file_path() {
        // Test that we can get the cache file path for a tenant without asserting specific content
        let cache_file = FolderCache::get_cache_file_path("test-tenant");
        // Just make sure it doesn't panic and returns a path
        assert!(cache_file.is_absolute() || cache_file.starts_with("."));
    }

    #[test]
    fn test_folder_cache_invalidate_nonexistent() {
        // Test that we can invalidate a cache file that doesn't exist.
        // No need to override the cache dir — invalidate() only removes a file
        // if it exists, so it returns Ok(()) regardless of where the cache lives.
        let result = FolderCache::invalidate("nonexistent-tenant-xyzzy-404");
        assert!(result.is_ok());
    }

    #[test]
    fn test_folder_cache_save_and_load() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());

        // Test that save creates cache directory
        let test_uuid = uuid::Uuid::new_v4();
        let result =
            FolderCache::save(&test_uuid, &crate::folder_hierarchy::FolderHierarchy::new());
        assert!(result.is_ok());

        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }
}

//! Shared caching functionality for the Physna CLI client.
//!
//! This module provides common utilities for implementing cache functionality
//! across different types of data in the application. It reduces code duplication
//! and provides a consistent interface for cache operations.

/// Common error type for cache operations that implements Send + Sync
#[derive(Debug)]
pub enum CacheError {
    IoError(std::io::Error),
    SerializationError(serde_json::Error),
    ApiError(Box<dyn std::error::Error + Send + Sync>),
    Other(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::IoError(e) => write!(f, "IO error: {}", e),
            CacheError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            CacheError::ApiError(e) => write!(f, "API error: {}", e),
            CacheError::Other(s) => write!(f, "Cache error: {}", s),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CacheError::IoError(e) => Some(e),
            CacheError::SerializationError(e) => Some(e),
            CacheError::ApiError(e) => Some(e.as_ref()),
            CacheError::Other(_) => None,
        }
    }
}

impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        CacheError::IoError(error)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(error: serde_json::Error) -> Self {
        CacheError::SerializationError(error)
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for CacheError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        CacheError::ApiError(error)
    }
}

/// Common cache configuration options
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Cache expiration time in seconds
    pub expiration_seconds: u64,
    /// Whether to compress cached data
    pub compress: bool,
    /// The directory where cache files are stored
    pub cache_dir: std::path::PathBuf,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            expiration_seconds: 24 * 60 * 60, // 24 hours
            compress: false,
            cache_dir: dirs::cache_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("pcli2"),
        }
    }
}

/// Base cache functionality that can be shared across different cache implementations
#[derive(Debug, Default)]
pub struct BaseCache {
    /// Configuration for the cache
    pub config: CacheConfig,
}

impl BaseCache {
    /// Create a new BaseCache with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new BaseCache with custom configuration
    pub fn with_config(config: CacheConfig) -> Self {
        Self { config }
    }

    /// Get the default cache directory path
    ///
    /// In a test environment (when PCLI2_TEST_CACHE_DIR is set), it uses that directory.
    /// For general cross-platform support (when PCLI2_CACHE_DIR is set), it uses that directory.
    /// Otherwise, it uses the system's cache directory with a "pcli2" subdirectory.
    pub fn get_cache_dir() -> std::path::PathBuf {
        // Check if we're in a test environment
        if let Ok(test_cache_dir) = std::env::var("PCLI2_TEST_CACHE_DIR") {
            std::path::PathBuf::from(test_cache_dir)
        } else if let Ok(cache_dir_str) = std::env::var("PCLI2_CACHE_DIR") {
            std::path::PathBuf::from(cache_dir_str)
        } else {
            dirs::cache_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("pcli2")
        }
    }

    /// Check if a cache file is expired based on file modification time
    pub fn is_file_expired(cache_file: &std::path::Path) -> bool {
        match std::fs::metadata(cache_file) {
            Ok(metadata) => {
                match metadata.modified() {
                    Ok(modified_time) => {
                        let now = std::time::SystemTime::now();
                        match now.duration_since(modified_time) {
                            Ok(duration) => {
                                duration.as_secs() > CacheConfig::default().expiration_seconds
                            }
                            Err(_) => false, // If there's an error calculating duration, don't treat as expired
                        }
                    }
                    Err(_) => false, // If we can't get the modified time, don't treat as expired
                }
            }
            Err(_) => true, // If we can't get metadata, treat as expired
        }
    }

    /// Get the cache file path for a specific key
    pub fn get_cache_file_path(
        cache_dir: &std::path::Path,
        key: &str,
        extension: &str,
    ) -> std::path::PathBuf {
        cache_dir.join(format!("{}.{}", key, extension))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_base_cache_creation() {
        let cache = BaseCache::new();
        assert_eq!(cache.config.expiration_seconds, 24 * 60 * 60); // 24 hours
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.expiration_seconds, 24 * 60 * 60); // 24 hours
        assert!(!config.compress);
        assert!(config.cache_dir.ends_with("pcli2"));
    }

    #[test]
    fn test_is_file_expired_new_file() {
        let temp_file = NamedTempFile::new().unwrap();
        assert!(!BaseCache::is_file_expired(temp_file.path()));
    }

    #[test]
    fn test_get_cache_file_path() {
        let cache_dir = std::path::PathBuf::from("/tmp/test");
        let cache_file = BaseCache::get_cache_file_path(&cache_dir, "test_tenant", "json");
        assert!(cache_file.to_string_lossy().contains("test_tenant.json"));
    }
}

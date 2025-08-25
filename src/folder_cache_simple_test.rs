#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
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
#[cfg(test)]
mod tests {
    use crate::folder_cache::FolderCache;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
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
    fn test_folder_cache_save_and_load() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        
        // Set the test cache directory environment variable
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());
        
        // Create some test data
        let tenant_id = "test-tenant";
        let mut folder_paths = HashMap::new();
        folder_paths.insert("folder-1".to_string(), "Root/Folder 1".to_string());
        folder_paths.insert("folder-2".to_string(), "Root/Folder 2".to_string());
        
        // Save the data to cache
        let result = FolderCache::save(tenant_id, &folder_paths);
        assert!(result.is_ok());
        
        // Load the data from cache
        let loaded = FolderCache::load(tenant_id);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap(), folder_paths);
        
        // Clean up
        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }

    #[test]
    fn test_folder_cache_invalidate_nonexistent() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        
        // Set the test cache directory environment variable
        std::env::set_var("PCLI2_TEST_CACHE_DIR", temp_dir.path());
        
        // This should not panic or return an error
        let result = FolderCache::invalidate("nonexistent-tenant");
        assert!(result.is_ok());
        
        // Clean up
        std::env::remove_var("PCLI2_TEST_CACHE_DIR");
    }
}
#[cfg(test)]
mod tests {
    use crate::folder_cache::FolderCache;
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};
    
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
        let result = FolderCache::invalidate("nonexistent-tenant");
        assert!(result.is_ok());
    }
}
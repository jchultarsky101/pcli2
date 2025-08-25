use pcli2::folder_cache::FolderCache;
use std::collections::HashMap;

fn main() {
    println!("Testing folder cache functionality...");
    
    // Create some test folder paths
    let mut folder_paths = HashMap::new();
    folder_paths.insert("folder-1".to_string(), "Root/Folder 1".to_string());
    folder_paths.insert("folder-2".to_string(), "Root/Folder 2".to_string());
    folder_paths.insert("folder-3".to_string(), "Root/Folder 1/Subfolder 1".to_string());
    
    println!("Created test folder paths:");
    for (uuid, path) in &folder_paths {
        println!("  {}: {}", uuid, path);
    }
    
    // Test getting cache directory
    let cache_dir = FolderCache::get_cache_dir();
    println!("Cache directory: {:?}", cache_dir);
    
    // Test getting cache file path
    let cache_file = FolderCache::get_cache_file_path("test-tenant");
    println!("Cache file path for test-tenant: {:?}", cache_file);
    
    println!("Folder cache test completed successfully!");
}
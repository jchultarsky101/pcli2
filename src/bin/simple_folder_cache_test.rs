use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Simple test to verify folder cache functionality
fn main() {
    println!("Testing folder cache functionality...");
    
    // Test getting cache directory
    let cache_dir = get_cache_dir();
    println!("Cache directory: {:?}", cache_dir);
    
    // Test getting cache file path
    let cache_file = get_cache_file_path("test-tenant");
    println!("Cache file path for test-tenant: {:?}", cache_file);
    
    // Create some test folder paths
    let mut folder_paths = HashMap::new();
    folder_paths.insert("folder-1".to_string(), "Root/Folder 1".to_string());
    folder_paths.insert("folder-2".to_string(), "Root/Folder 2".to_string());
    folder_paths.insert("folder-3".to_string(), "Root/Folder 1/Subfolder 1".to_string());
    
    println!("Created test folder paths:");
    for (uuid, path) in &folder_paths {
        println!("  {}: {}", uuid, path);
    }
    
    // Test saving to cache
    match save_to_cache("test-tenant", &folder_paths) {
        Ok(()) => println!("Successfully saved folder paths to cache"),
        Err(e) => println!("Error saving folder paths to cache: {}", e),
    }
    
    // Test loading from cache
    match load_from_cache("test-tenant") {
        Some(loaded_paths) => {
            println!("Successfully loaded folder paths from cache:");
            for (uuid, path) in &loaded_paths {
                println!("  {}: {}", uuid, path);
            }
        }
        None => println!("No folder paths found in cache"),
    }
    
    // Test invalidating cache
    match invalidate_cache("test-tenant") {
        Ok(()) => println!("Successfully invalidated cache for test-tenant"),
        Err(e) => println!("Error invalidating cache: {}", e),
    }
    
    println!("Folder cache test completed!");
}

/// Get the cache directory path
fn get_cache_dir() -> PathBuf {
    let cache_dir = dirs::cache_dir().unwrap_or_else(|| std::env::temp_dir());
    cache_dir.join("pcli2").join("folder_cache")
}

/// Get the cache file path for a specific tenant
fn get_cache_file_path(tenant_id: &str) -> PathBuf {
    get_cache_dir().join(format!("{}.bin", tenant_id))
}

/// Save folder paths to cache
fn save_to_cache(
    tenant_id: &str,
    folder_paths: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let serialized = bincode::serialize(folder_paths)?;
    
    // Create cache directory if it doesn't exist
    let cache_dir = get_cache_dir();
    fs::create_dir_all(&cache_dir)?;
    
    let cache_file = get_cache_file_path(tenant_id);
    fs::write(cache_file, serialized)?;
    
    Ok(())
}

/// Load folder paths from cache
fn load_from_cache(tenant_id: &str) -> Option<HashMap<String, String>> {
    let cache_file = get_cache_file_path(tenant_id);
    
    if cache_file.exists() {
        match fs::read(&cache_file) {
            Ok(data) => {
                match bincode::deserialize::<HashMap<String, String>>(&data) {
                    Ok(folder_paths) => Some(folder_paths),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    } else {
        None
    }
}

/// Invalidate cache for a specific tenant
fn invalidate_cache(tenant_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cache_file = get_cache_file_path(tenant_id);
    if cache_file.exists() {
        fs::remove_file(cache_file)?;
    }
    Ok(())
}
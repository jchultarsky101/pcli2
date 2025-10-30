//! Resolution utilities for the Physna CLI client.
//!
//! This module provides common resolution utilities for converting between
//! different identifiers (like paths to UUIDs) that are used throughout the application.

use crate::physna_v3::{PhysnaApiClient, ApiError};
use crate::asset_cache::AssetCache;
use crate::folder_cache::FolderCache;
use tracing::{debug, trace};

/// Resolve a tenant name or ID to a tenant ID
/// 
/// This function handles the case where users provide either a tenant name or ID
/// via the --tenant parameter. It checks if the provided identifier looks like
/// a UUID (tenant ID) or a human-readable name, and resolves names to IDs by
/// calling the list_tenants API endpoint.
/// 
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant_identifier` - The tenant name or ID to resolve
/// 
/// # Returns
/// * `Ok(String)` - The resolved tenant ID
/// * `Err(ApiError)` - If the tenant cannot be found
pub async fn resolve_tenant_identifier_to_id(
    client: &mut PhysnaApiClient,
    tenant_identifier: String,
) -> Result<String, ApiError> {
    debug!("Resolving tenant identifier: {}", tenant_identifier);
    
    // First, try to list all tenants to see if we can resolve the identifier
    let tenants = client.list_tenants().await?;
    
    // Look for an exact match by tenant ID first
    for tenant in &tenants {
        if tenant.tenant_id == tenant_identifier {
            debug!("Tenant identifier {} appears to be a direct ID match", tenant_identifier);
            return Ok(tenant.tenant_id.clone());
        }
    }
    
    // Then look for a match by name
    for tenant in &tenants {
        if tenant.tenant_display_name == tenant_identifier || 
           tenant.tenant_short_name.as_str() == tenant_identifier {
            debug!("Resolved tenant identifier '{}' to ID '{}'", tenant_identifier, tenant.tenant_id);
            return Ok(tenant.tenant_id.clone());
        }
    }
    
    // If we can't find the tenant, return an error
    Err(ApiError::AuthError(format!("Tenant '{}' not found", tenant_identifier)))
}

/// Efficiently resolve an asset path to its UUID by:
/// 1. Splitting the path into folder path and asset name
/// 2. Resolving the folder path to a folder ID
/// 3. Listing only assets in that specific folder
/// 4. Finding the asset by name within that folder
/// 
/// This is much more efficient than fetching all assets in the system and filtering locally.
/// 
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant` - The tenant ID
/// * `path` - The full path to the asset (e.g., "/Root/Folder/asset.stl")
/// 
/// # Returns
/// * `Ok(String)` - The UUID of the asset if found
/// * `Err(ApiError)` - If the asset is not found or there's an error
pub async fn resolve_asset_path_to_uuid(
    client: &mut PhysnaApiClient,
    tenant: &str,
    path: &str,
) -> Result<String, ApiError> {
    debug!("Resolving asset path to UUID: {}", path);
    
    // Split the path into folder path and asset name
    let (folder_path, asset_name) = if let Some(last_slash) = path.rfind('/') {
        let folder_path = if last_slash == 0 { "/" } else { &path[..last_slash] };
        let asset_name = &path[last_slash + 1..];
        (folder_path, asset_name)
    } else {
        // No slashes, it's in the root folder
        ("/", path)
    };
    
    debug!("Split path into folder: '{}' and asset name: '{}'", folder_path, asset_name);
    
    // Get folder ID by path
    match client.get_folder_id_by_path(tenant, folder_path).await {
        Ok(Some(folder_id)) => {
            debug!("Found folder ID: {} for path: {}", folder_id, folder_path);
            // List ALL assets in this specific folder to ensure we find the target asset even if it's on another page
            match client.list_all_assets_in_folder(tenant, &folder_id).await {
                Ok(asset_list_response) => {
                    trace!("Found {} assets in folder {}", asset_list_response.assets.len(), folder_path);
                    
                    // Find the asset by name within this folder
                    // Extract the asset name from each asset's path and compare with our target asset name
                    if let Some(asset_response) = asset_list_response.assets.iter().find(|asset| {
                        let extracted_name = if let Some(last_slash) = asset.path.rfind('/') {
                            let name = &asset.path[last_slash + 1..];
                            name == asset_name
                        } else {
                            // No slashes in asset path, compare directly
                            &asset.path == asset_name
                        };
                        extracted_name
                    }) {
                        trace!("Found asset with UUID: {}", asset_response.id);
                        Ok(asset_response.id.clone())
                    } else {
                        Err(ApiError::ConflictError(
                            format!("Asset '{}' not found in folder '{}'", asset_name, folder_path)
                        ))
                    }
                }
                Err(e) => {
                    debug!("Error listing all assets in folder '{}': {}", folder_path, e);
                    Err(e)
                }
            }
        }
        Ok(None) => {
            Err(ApiError::ConflictError(
                format!("Folder path '{}' not found", folder_path)
            ))
        }
        Err(e) => {
            debug!("Error resolving folder path '{}': {}", folder_path, e);
            Err(e)
        }
    }
}

/// Efficiently resolve a folder path to its UUID by leveraging cached folder hierarchies
/// 
/// This function uses the folder cache to efficiently resolve folder paths to folder IDs.
/// 
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant` - The tenant ID
/// * `path` - The full path to the folder (e.g., "/Root/Folder/Subfolder")
/// 
/// # Returns
/// * `Ok(String)` - The UUID of the folder if found
/// * `Err(ApiError)` - If the folder is not found or there's an error
pub async fn resolve_folder_path_to_uuid(
    client: &mut PhysnaApiClient,
    tenant: &str,
    path: &str,
) -> Result<String, ApiError> {
    debug!("Resolving folder path to UUID: {}", path);
    
    // Use the proven FolderHierarchy implementation to resolve the path
    match FolderCache::get_or_fetch(client, tenant).await {
        Ok(hierarchy) => {
            // Use the hierarchy to find the folder by path
            if let Some(folder_node) = hierarchy.get_folder_by_path(path) {
                debug!("Found folder at path '{}': {}", path, folder_node.folder.id);
                Ok(folder_node.folder.id.clone())
            } else {
                debug!("Folder not found at path: {}", path);
                Err(ApiError::ConflictError(
                    format!("Folder path '{}' not found", path)
                ))
            }
        }
        Err(e) => {
            debug!("Error building folder hierarchy: {}", e);
            Err(ApiError::ConflictError(
                format!("Failed to build folder hierarchy: {}", e)
            ))
        }
    }
}

/// Resolve an asset path to its UUID using cached asset data when possible
/// 
/// This function uses the asset cache to resolve asset paths to UUIDs efficiently.
/// 
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant` - The tenant ID
/// * `path` - The full path to the asset (e.g., "/Root/Folder/asset.stl")
/// 
/// # Returns
/// * `Ok(String)` - The UUID of the asset if found
/// * `Err(ApiError)` - If the asset is not found or there's an error
pub async fn resolve_asset_path_to_uuid_cached(
    client: &mut PhysnaApiClient,
    tenant: &str,
    path: &str,
) -> Result<String, ApiError> {
    debug!("Resolving asset path to UUID using cache: {}", path);
    
    // Get asset list response from cache or API
    match AssetCache::get_or_fetch(client, tenant).await {
        Ok(asset_list_response) => {
            trace!("Found {} assets in cache", asset_list_response.assets.len());
            
            // Find the asset by path in the asset list
            if let Some(asset_response) = asset_list_response.assets.iter().find(|asset| {
                asset.path == path || asset.path.ends_with(&format!("/{}", path)) || asset.path == path
            }) {
                trace!("Found asset with UUID: {}", asset_response.id);
                Ok(asset_response.id.clone())
            } else {
                debug!("Asset not found by path: {}", path);
                Err(ApiError::ConflictError(
                    format!("Asset '{}' not found", path)
                ))
            }
        }
        Err(e) => {
            debug!("Error fetching asset cache: {}", e);
            Err(ApiError::RetryFailed(format!("Failed to fetch asset cache: {}", e)))
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_path_splitting() {
        // This test just verifies the path splitting logic works
        let path = "/Root/Folder/asset.stl";
        let (folder_path, asset_name) = if let Some(last_slash) = path.rfind('/') {
            let folder_path = if last_slash == 0 { "/" } else { &path[..last_slash] };
            let asset_name = &path[last_slash + 1..];
            (folder_path, asset_name)
        } else {
            ("/", path)
        };
        
        assert_eq!(folder_path, "/Root/Folder");
        assert_eq!(asset_name, "asset.stl");
    }
}
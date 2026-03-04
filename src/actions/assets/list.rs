//! List assets functionality.
//!
//! This module provides functionality for listing assets in folders.

use crate::{
    commands::params::PARAMETER_FOLDER_PATH,
    configuration::Configuration,
    error::CliError,
    format::OutputFormatter,
    model::{normalize_path, AssetList},
    param_utils::{get_format_parameter_value, get_tenant},
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

/// List assets in a folder or tenant.
///
/// This function handles the "asset list" command, retrieving assets
/// from the Physna API based on the provided parameters.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the assets were listed successfully
/// * `Err(CliError)` - If an error occurred during the listing
pub async fn list_assets(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Listing assets...");

    let format = get_format_parameter_value(sub_matches).await;
    let configuration = Configuration::load_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let is_recursive = sub_matches.get_flag("recursive");

    // Require a folder path when using the recursive flag
    if is_recursive && !sub_matches.contains_id(PARAMETER_FOLDER_PATH) {
        return Err(CliError::MissingRequiredArgument(
            "Folder path must be specified when using --recursive flag".to_string(),
        ));
    }

    // If a path is specified, get assets filtered by folder path
    if let Some(path) = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH) {
        trace!("Listing assets for folder path: {}", path);

        let path = normalize_path(path);
        trace!("Normalized folder path: {}", &path);

        if is_recursive {
            // Recursively list assets in the folder and all subfolders
            let all_assets = list_assets_recursively(&mut api, &tenant.uuid, &path).await?;
            println!("{}", all_assets.format(format)?);
        } else {
            let assets = api
                .list_assets_by_parent_folder_path(&tenant.uuid, path.as_str())
                .await?;
            println!("{}", assets.format(format)?);
        }
    } else {
        // Without a folder path, just list top-level assets (non-recursive)
        let assets = api
            .list_assets_by_parent_folder_uuid(&tenant.uuid, None)
            .await?;
        println!("{}", assets.format(format)?);
    };

    Ok(())
}

/// List assets in a folder and all its subfolders using folder hierarchy
///
/// # Arguments
///
/// * `api` - The Physna API client
/// * `tenant_id` - The tenant UUID
/// * `folder_path` - The folder path to list assets from
///
/// # Returns
///
/// * `Ok(AssetList)` - The list of assets
/// * `Err(CliError)` - If an error occurred during the listing
async fn list_assets_recursively(
    api: &mut PhysnaApiClient,
    tenant_id: &Uuid,
    folder_path: &str,
) -> Result<AssetList, CliError> {
    use crate::folder_hierarchy::FolderHierarchy;

    // Build the complete folder hierarchy for the tenant
    let hierarchy = FolderHierarchy::build_from_api(api, tenant_id).await?;

    // Filter the hierarchy to only include the specified path and its subfolders
    let filtered_hierarchy = hierarchy
        .filter_by_path(folder_path)
        .ok_or_else(|| CliError::FolderNotFound(folder_path.to_string()))?;

    let mut all_assets = AssetList::empty();

    // Process each folder in the filtered hierarchy to get its assets
    for (folder_uuid, folder_node) in &filtered_hierarchy.nodes {
        // Get the path for this folder from the hierarchy
        let folder_path: String = filtered_hierarchy
            .get_path_for_folder(folder_uuid)
            .unwrap_or_else(|| folder_node.name().to_string());

        // List assets in this specific folder
        let folder_assets = api
            .list_assets_by_parent_folder_path(tenant_id, &folder_path)
            .await?;

        // Add assets from this folder to the result
        for asset in folder_assets.get_all_assets() {
            all_assets.insert(asset.clone());
        }
    }

    Ok(all_assets)
}

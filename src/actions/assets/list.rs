//! List assets functionality.
//!
//! This module provides functionality for listing assets in folders.

use crate::{
    commands::params::PARAMETER_FOLDER_PATH,
    configuration::Configuration,
    error::CliError,
    format::OutputFormatter,
    model::normalize_path,
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

    // Check if reload flag is set to clear the cache
    let reload_cache = sub_matches.get_flag(crate::commands::params::PARAMETER_RELOAD);
    if reload_cache {
        trace!("Reload flag set, clearing folder cache before listing assets...");
        crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()).unwrap_or_else(
            |e| {
                tracing::debug!("Failed to invalidate folder cache: {}", e);
            },
        );
    }

    // A folder given by UUID is turned into its canonical path so the two forms
    // share one code path.
    let folder_path_param: Option<String> = match (
        sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_FOLDER_UUID),
        sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH),
    ) {
        (Some(folder_uuid), _) => Some(
            crate::actions::utils::canonical_folder_path(&mut api, &tenant.uuid, folder_uuid)
                .await?,
        ),
        (None, Some(path)) => Some(path.clone()),
        (None, None) => None,
    };

    // If a path is specified, get assets filtered by folder path
    if let Some(path) = folder_path_param.as_ref() {
        trace!("Listing assets for folder path: {}", path);

        let path = normalize_path(path);
        trace!("Normalized folder path: {}", &path);

        if is_recursive {
            // Recursively list assets in the folder and all subfolders
            // Root path "/" is always valid; only check existence for non-root paths
            if path != "/" {
                let hierarchy =
                    crate::actions::utils::hierarchy_containing(&mut api, &tenant.uuid, &path)
                        .await?;

                if hierarchy.get_node_by_path(&path).is_none() {
                    return Err(crate::actions::folders::folder_not_found(&hierarchy, &path));
                }
            }

            // Walks the subtree by folder UUID. The walk this replaced rebuilt each
            // folder's path from a detached copy of the hierarchy, so below the top
            // level it looked up `B/C` instead of `/A/B/C`: nested folders were
            // reported missing, or a different top-level folder of the same name was
            // listed. It also left out assets stored directly at the root.
            let all_assets = api
                .list_assets_by_parent_folder_path_recursive(&tenant.uuid, &path, |_, _, _| {})
                .await?;
            crate::format::print_output(&all_assets.format(format)?);
        } else if path == "/" {
            // Root path - list assets at the root level (no parent folder)
            let assets = api
                .list_assets_by_parent_folder_uuid(&tenant.uuid, None)
                .await?;
            crate::format::print_output(&assets.format(format)?);
        } else {
            // First verify the folder exists by building the hierarchy
            let hierarchy =
                crate::actions::utils::hierarchy_containing(&mut api, &tenant.uuid, &path).await?;

            // Check if the path exists (case-sensitive)
            if hierarchy.get_node_by_path(&path).is_none() {
                return Err(crate::actions::folders::folder_not_found(&hierarchy, &path));
            }

            let assets = api
                .list_assets_by_parent_folder_path(&tenant.uuid, path.as_str())
                .await?;
            crate::format::print_output(&assets.format(format)?);
        }
    } else {
        // Without a folder path, just list top-level assets (non-recursive)
        let assets = api
            .list_assets_by_parent_folder_uuid(&tenant.uuid, None)
            .await?;
        crate::format::print_output(&assets.format(format)?);
    };

    Ok(())
}

//! Delete asset functionality.
//!
//! This module provides functionality for deleting assets and asset metadata.

use crate::{
    commands::params::{PARAMETER_PATH, PARAMETER_UUID},
    configuration::Configuration,
    error::CliError,
    param_utils::get_tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;

/// Delete an asset by UUID or path.
///
/// This function handles the "asset delete" command, removing a specific asset
/// identified by either its UUID or path from the Physna API.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was deleted successfully
/// * `Err(CliError)` - If an error occurred during deletion
pub async fn delete_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset delete\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
    let yes_flag = sub_matches.get_flag("yes");

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Ask for confirmation unless --yes flag is provided
    if !yes_flag {
        let asset_identifier = asset_path_param
            .map(|s| s.to_string())
            .unwrap_or_else(|| asset.uuid().to_string());
        let delete_msg = format!("Delete asset '{}'?", asset_identifier);

        let confirm = inquire::Confirm::new(&delete_msg)
            .with_default(false)
            .with_help_message("This action cannot be undone")
            .prompt();

        match confirm {
            Ok(true) => {} // User confirmed
            Ok(false) => {
                println!("Deletion cancelled.");
                return Ok(());
            }
            Err(e) => {
                // Error in prompting (e.g., not a TTY), treat as cancellation
                eprintln!(
                    "Error prompting for confirmation: {}. Use --yes to skip confirmation.",
                    e
                );
                return Ok(());
            }
        }
    }

    // Delete the asset
    ctx.api()
        .delete_asset(&tenant_uuid.to_string(), &asset.uuid().to_string())
        .await?;

    Ok(())
}

/// Delete specific metadata fields from an asset.
///
/// This function handles the "asset metadata delete" command, which removes
/// specified metadata fields from a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the metadata was deleted successfully
/// * `Err(CliError)` - If an error occurred during the deletion
pub async fn delete_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Execute \"asset metadata delete\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Get metadata name parameter from command line
    let metadata_names = sub_matches
        .get_many::<String>("field_name")
        .ok_or(CliError::MissingRequiredArgument("field_name".to_string()))?
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either asset UUID or path must be provided".to_string(),
        ));
    };

    // Delete the specified metadata fields using the dedicated API endpoint
    let metadata_keys: Vec<&str> = metadata_names.iter().map(|s| s.as_ref()).collect();
    api.delete_asset_metadata(
        &tenant.uuid.to_string(),
        &asset.uuid().to_string(),
        metadata_keys,
    )
    .await?;

    Ok(())
}

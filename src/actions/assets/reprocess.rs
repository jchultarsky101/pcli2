//! Reprocess asset functionality.
//!
//! This module provides functionality for reprocessing assets.

use crate::{
    commands::params::{PARAMETER_PATH, PARAMETER_UUID},
    error::CliError,
};
use clap::ArgMatches;
use tracing::trace;

/// Reprocess an asset by UUID or path.
///
/// This function handles the "asset reprocess" command, triggering reprocessing
/// of a specific asset identified by either its UUID or path in the Physna API.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was reprocessed successfully
/// * `Err(CliError)` - If an error occurred during reprocessing
pub async fn reprocess_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset reprocess\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

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

    // Trigger reprocessing of the asset
    ctx.api()
        .reprocess_asset(&tenant_uuid, &asset.uuid())
        .await?;

    // No output on success (following UNIX convention)
    Ok(())
}

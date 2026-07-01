//! Dependency diff action.
//!
//! Implements the `asset dependency-diff` command: it resolves two assets (a
//! reference and a candidate, each by UUID or path), fetches their recursive
//! dependency trees, computes a structural diff, and prints the result in the
//! requested format.

use crate::{
    commands::params::{
        PARAMETER_CANDIDATE_PATH, PARAMETER_CANDIDATE_UUID, PARAMETER_REFERENCE_PATH,
        PARAMETER_REFERENCE_UUID,
    },
    dependency_diff::compute_dependency_diff,
    error::CliError,
    format::OutputFormatter,
    param_utils::get_format_parameter_value,
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

/// Compare the dependency trees of two assets and print their diff.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the dependency diff was printed successfully
/// * `Err(CliError)` - If either asset could not be resolved or an API/formatting error occurred
pub async fn compare_asset_dependencies(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset dependency-diff\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    let reference_uuid = sub_matches.get_one::<Uuid>(PARAMETER_REFERENCE_UUID);
    let reference_path = sub_matches.get_one::<String>(PARAMETER_REFERENCE_PATH);
    let candidate_uuid = sub_matches.get_one::<Uuid>(PARAMETER_CANDIDATE_UUID);
    let candidate_path = sub_matches.get_one::<String>(PARAMETER_CANDIDATE_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve both assets, annotating which input failed for a clear error message.
    let reference_asset =
        crate::actions::utils::resolve_asset(ctx.api(), &tenant_uuid, reference_uuid, reference_path)
            .await
            .map_err(|e| CliError::AssetResolutionError("reference".to_string(), e.to_string()))?;

    let candidate_asset =
        crate::actions::utils::resolve_asset(ctx.api(), &tenant_uuid, candidate_uuid, candidate_path)
            .await
            .map_err(|e| CliError::AssetResolutionError("candidate".to_string(), e.to_string()))?;

    // Fetch the full recursive dependency trees for both assets.
    let reference_tree = ctx
        .api()
        .get_asset_dependencies_by_path(&tenant_uuid, reference_asset.path().as_str())
        .await?;
    let candidate_tree = ctx
        .api()
        .get_asset_dependencies_by_path(&tenant_uuid, candidate_asset.path().as_str())
        .await?;

    // Compute the structural diff and print it in the requested format.
    let diff = compute_dependency_diff(&reference_tree, &candidate_tree);
    println!("{}", diff.format(format)?);

    Ok(())
}

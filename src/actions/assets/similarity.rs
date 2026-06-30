//! Asset similarity functionality.
//!
//! This module provides functionality for retrieving the pairwise match scores
//! between two specific assets using the Physna "match scores" endpoint.

use crate::{
    commands::params::{
        PARAMETER_CANDIDATE_PATH, PARAMETER_CANDIDATE_UUID, PARAMETER_REFERENCE_PATH,
        PARAMETER_REFERENCE_UUID,
    },
    error::CliError,
    format::OutputFormatter,
};
use clap::ArgMatches;
use tracing::trace;

/// Get the pairwise match scores between two assets.
///
/// This function handles the "asset similarity" command. It resolves both a
/// reference (source) and a candidate (target) asset from either a UUID or a
/// path, retrieves their pairwise match scores from the API, and prints the
/// result in the requested format (JSON or CSV).
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the operation was successful
/// * `Err(CliError)` - If an error occurred
pub async fn asset_similarity(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing asset similarity command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let reference_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_REFERENCE_UUID);
    let reference_path_param = sub_matches.get_one::<String>(PARAMETER_REFERENCE_PATH);
    let candidate_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_CANDIDATE_UUID);
    let candidate_path_param = sub_matches.get_one::<String>(PARAMETER_CANDIDATE_PATH);

    // Use FormatParams for consistent format parameter handling
    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve both assets from either UUID or path using the shared helper.
    let reference_asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        reference_uuid_param,
        reference_path_param,
    )
    .await?;

    let candidate_asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        candidate_uuid_param,
        candidate_path_param,
    )
    .await?;

    // Retrieve the pairwise match scores.
    let scores = ctx
        .api()
        .match_scores(
            &tenant_uuid,
            &reference_asset.uuid(),
            &candidate_asset.uuid(),
        )
        .await?;

    // Build the comparison URL using the configurable UI base URL, mirroring the
    // behaviour of the other match commands.
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();
    let base_url = ui_base_url.trim_end_matches('/');
    let comparison_url = if base_url.ends_with("/tenants") {
        format!(
            "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
            base_url,
            tenant_name,
            reference_asset.uuid(),
            candidate_asset.uuid(),
            tenant_uuid,
            tenant_uuid,
            scores.geometric.match_percentage
        )
    } else {
        format!(
            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
            base_url,
            tenant_name,
            reference_asset.uuid(),
            candidate_asset.uuid(),
            tenant_uuid,
            tenant_uuid,
            scores.geometric.match_percentage
        )
    };

    let similarity = crate::model::AssetSimilarity {
        reference_asset_path: reference_asset.path(),
        reference_asset_uuid: reference_asset.uuid(),
        candidate_asset_path: candidate_asset.path(),
        candidate_asset_uuid: candidate_asset.uuid(),
        geometric: scores.geometric,
        volumetric: scores.volumetric,
        comparison_url: Some(comparison_url),
    };

    println!("{}", similarity.format(format)?);

    Ok(())
}

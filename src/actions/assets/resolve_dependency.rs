//! `asset resolve-dependency`: link a missing dependency of an assembly to an
//! existing asset.
//!
//! An assembly whose referenced part was not found stays in the
//! `missing-dependencies` state. `asset dependencies` shows the missing
//! paths; this command names the asset that should stand in for one of them,
//! and the server re-indexes the assembly.

use crate::{
    actions::CliActionError,
    commands::params::{
        PARAMETER_DEPENDENCY, PARAMETER_PATH, PARAMETER_TARGET_PATH, PARAMETER_TARGET_UUID,
        PARAMETER_UUID,
    },
    error::CliError,
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

/// Resolve `--dependency` of the assembly (`--uuid`/`--path`) with the target
/// asset (`--target-uuid`/`--target-path`).
///
/// The dependency must be one of the assembly's missing ones: the listing is
/// read first, and a path that is not in it stops the command with the list
/// (exit 64), so a typo never reaches the server. Silent on success;
/// `--dry-run` names everything instead.
pub async fn resolve_asset_dependency(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset resolve-dependency\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();

    let assembly_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
    let assembly_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
    let target_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_TARGET_UUID);
    let target_path_param = sub_matches.get_one::<String>(PARAMETER_TARGET_PATH);
    let dependency = sub_matches
        .get_one::<String>(PARAMETER_DEPENDENCY)
        .cloned()
        .unwrap_or_default();

    let progress = crate::terminal::spinner("Fetching assets...");
    let assembly = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        assembly_uuid_param,
        assembly_path_param,
    )
    .await
    .map_err(|e| CliError::resolving("assembly", e));
    let target = match &assembly {
        Ok(_) => crate::actions::utils::resolve_asset(
            ctx.api(),
            &tenant_uuid,
            target_uuid_param,
            target_path_param,
        )
        .await
        .map_err(|e| CliError::resolving("target", e)),
        Err(_) => Err(CliError::MissingRequiredArgument(String::new())),
    };
    progress.finish_and_clear();
    let assembly = assembly?;
    let target = target?;

    // Only a missing dependency can be resolved. Match the user's spelling
    // leniently (a leading slash is fine) but send the path as the API lists it.
    let progress = crate::terminal::spinner("Reading the assembly's dependencies...");
    let listing = ctx
        .api()
        .get_asset_dependencies_list_by_uuid(&tenant_uuid, &assembly.uuid())
        .await;
    progress.finish_and_clear();
    let listing = listing?;
    let missing: Vec<String> = listing
        .dependencies
        .iter()
        .filter(|d| d.is_missing())
        .map(|d| d.path.clone())
        .collect();
    let wanted = dependency.trim_start_matches('/');
    let Some(dependency_path) = missing
        .iter()
        .find(|p| p.trim_start_matches('/') == wanted)
        .cloned()
    else {
        let detail = if missing.is_empty() {
            format!("'{}' has no missing dependencies.", assembly.path())
        } else {
            format!(
                "The missing dependencies of '{}' are:\n  {}",
                assembly.path(),
                missing.join("\n  ")
            )
        };
        return Err(CliActionError::BusinessLogicError(format!(
            "'{}' is not a missing dependency of '{}'. {}",
            dependency,
            assembly.path(),
            detail
        ))
        .into());
    };

    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would resolve dependency '{}' of '{}' (UUID {}) with asset '{}' (UUID {})",
            dependency_path,
            assembly.path(),
            assembly.uuid(),
            target.path(),
            target.uuid()
        );
        return Ok(());
    }

    let progress = crate::terminal::spinner("Resolving the dependency...");
    let result = ctx
        .api()
        .resolve_asset_dependency(
            &tenant_uuid,
            &assembly.uuid(),
            &dependency_path,
            &target.uuid(),
        )
        .await;
    progress.finish_and_clear();
    result?;

    // No output on success (following UNIX convention)
    Ok(())
}

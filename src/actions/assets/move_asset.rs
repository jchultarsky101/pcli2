//! `asset move`: put an asset in another folder, or in the root.
//!
//! `folder move` existed; assets could only be moved through the web
//! application. The asset keeps its UUID and metadata; only its path changes.

use crate::{
    commands::params::{
        PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_PATH, PARAMETER_UUID,
    },
    error::CliError,
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

/// Move an asset to the folder given by `--folder-uuid` or `--folder-path`.
///
/// `/` (or `/Home`) as the path means the root, which has no UUID. Prints
/// nothing on success (UNIX convention, as `folder move` does); `--dry-run`
/// names the asset and the destination instead of moving anything.
pub async fn move_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset move\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant = ctx.tenant().clone();

    let asset_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    let progress = crate::terminal::spinner("Fetching asset...");
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant.uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await;
    progress.finish_and_clear();
    let asset = asset?;

    // The destination: a UUID, a path that resolves to one, or the root (no UUID).
    let (destination, destination_label): (Option<Uuid>, String) =
        match (folder_uuid_param, folder_path_param) {
            (Some(uuid), _) => (Some(*uuid), uuid.to_string()),
            (None, Some(path)) => {
                let normalized = crate::model::normalize_path(path);
                if normalized == "/" {
                    (None, "/".to_string())
                } else {
                    let uuid = crate::actions::folders::resolve_folder_uuid_by_path(
                        ctx.api(),
                        &tenant,
                        path,
                    )
                    .await?;
                    (Some(uuid), normalized)
                }
            }
            (None, None) => {
                return Err(CliError::MissingRequiredArgument(
                    "Either --folder-uuid or --folder-path must be provided".to_string(),
                ))
            }
        };

    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would move asset '{}' (UUID {}) to folder '{}'",
            asset.path(),
            asset.uuid(),
            destination_label
        );
        return Ok(());
    }

    let progress = crate::terminal::spinner("Moving asset...");
    let moved = ctx
        .api()
        .move_asset(&tenant.uuid, &asset.uuid(), destination)
        .await;
    progress.finish_and_clear();
    let moved = moved?;
    trace!("Asset {} is now at '{}'", moved.uuid(), moved.path());

    // No output on success (following UNIX convention)
    Ok(())
}

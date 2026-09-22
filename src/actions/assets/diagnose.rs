//! `asset diagnose`: why an asset failed to process, from the server's
//! failure diagnostics.
//!
//! Until now a failed asset was only visible as the word `failed` in a
//! `STATE` column or a number in `tenant state`. The API can search its
//! ingestion logs on demand and say what went wrong; this command asks it.

use crate::{
    commands::params::{PARAMETER_METADATA, PARAMETER_PATH, PARAMETER_UUID},
    error::CliError,
    format::OutputFormatter,
    model::{AssetFailureDiagnostics, FailureDiagnosticsStatus},
};
use clap::ArgMatches;
use tracing::trace;

/// The message for a deployment without failure log search (exit 68).
pub const UNAVAILABLE_MESSAGE: &str = "Failure diagnostics are not available on this Physna \
deployment: failure log search is not configured. Ask your Physna administrator, or check the \
asset's state with 'pcli2 asset get'.";

/// Look up why an asset failed and print the answer.
///
/// - `found`: the record is printed, exit 0.
/// - `not-found`: the record is printed with that status, exit 0, and a note
///   on stderr says whether that is because the asset is not failed or because
///   the log entry has aged out.
/// - `unavailable`: an error with [`UNAVAILABLE_MESSAGE`], exit 68.
pub async fn diagnose_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset diagnose\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;
    crate::format_utils::warn_if_given(
        sub_matches,
        PARAMETER_METADATA,
        "failure diagnostics have no metadata columns",
    );

    let tenant_uuid = *ctx.tenant_uuid();

    let progress = crate::terminal::spinner("Fetching asset...");
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await;
    progress.finish_and_clear();
    let asset = asset?;

    let progress = crate::terminal::spinner("Searching failure logs...");
    let diagnostics = ctx
        .api()
        .get_asset_failure_diagnostics(&tenant_uuid, &asset.uuid())
        .await;
    progress.finish_and_clear();
    let diagnostics = diagnostics?;

    if diagnostics.status == FailureDiagnosticsStatus::Unavailable {
        return Err(CliError::FeatureUnavailable(
            UNAVAILABLE_MESSAGE.to_string(),
        ));
    }

    let record = AssetFailureDiagnostics::new(&asset, diagnostics);
    if record.status == FailureDiagnosticsStatus::NotFound {
        eprintln!("{}", not_found_note(record.asset_state.as_deref()));
    }

    crate::format::print_output(&record.format(format)?);
    Ok(())
}

/// Why the server may have found nothing, given the asset's state.
fn not_found_note(state: Option<&str>) -> String {
    match state {
        Some("failed") => "No failure record found: the log entry has aged out of retention, \
                           or the failure was not logged. Reprocessing the asset produces a \
                           fresh one if it fails again."
            .to_string(),
        Some(state) => format!(
            "No failure record found: the asset is `{}`, not `failed`, so there is nothing to \
             diagnose.",
            state
        ),
        None => "No failure record found, and the asset reports no processing state.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_names_the_state_that_makes_a_lookup_pointless() {
        let note = not_found_note(Some("finished"));
        assert!(note.contains("`finished`, not `failed`"));
    }

    #[test]
    fn the_note_explains_a_failed_asset_without_a_record() {
        let note = not_found_note(Some("failed"));
        assert!(note.contains("aged out"));
        assert!(note.contains("Reprocessing"));
    }

    #[test]
    fn the_note_copes_with_a_missing_state() {
        assert!(not_found_note(None).contains("no processing state"));
    }
}

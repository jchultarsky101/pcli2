//! The `report` command group: list, get, download, diagnose, delete and
//! create report jobs.
//!
//! Reports run on the server. `create --wait` follows one to the end with the
//! same best-effort rule as the rest of pcli2: one failed poll is retried, a
//! credential failure is fatal, and three consecutive failures give up.

use crate::{
    actions::CliActionError,
    commands::report::{
        PARAMETER_EXCLUDE_ASSEMBLIES, PARAMETER_EXCLUDE_EXACT_DUPLICATES,
        PARAMETER_EXCLUDE_FOLDER_PATH, PARAMETER_FILE_FORMAT, PARAMETER_ID, PARAMETER_INCLUDE_HOME,
        PARAMETER_MAX_THRESHOLD, PARAMETER_MIN_THRESHOLD, PARAMETER_POLL_INTERVAL,
        PARAMETER_STATUS, PARAMETER_TYPE, PARAMETER_WAIT,
    },
    context::ExecutionContext,
    error::CliError,
    exit_codes::PcliExitCode,
    format::OutputFormatter,
    model::{
        CreateDuplicationReportRequest, FailureDiagnosticsStatus, JobStatus, Report,
        ReportFailureDiagnostics, ReportList,
    },
    physna_v3::ApiError,
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

/// The message for a deployment without failure log search (exit 68).
pub const UNAVAILABLE_MESSAGE: &str = "Failure diagnostics are not available on this Physna \
deployment: failure log search is not configured. Ask your Physna administrator.";

/// Consecutive failed polls after which `--wait` gives up.
const POLL_FAILURE_THRESHOLD: u32 = 3;

fn report_id(sub_matches: &ArgMatches) -> Result<Uuid, CliError> {
    sub_matches
        .get_one::<Uuid>(PARAMETER_ID)
        .copied()
        .ok_or_else(|| CliError::MissingRequiredArgument("--id".to_string()))
}

/// `report list [--type T] [--status S] [--limit N]`.
pub async fn list_reports(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"report list\" command...");
    let format = crate::actions::tenants::plain_format(sub_matches)?;
    let report_type = sub_matches.get_one::<String>(PARAMETER_TYPE).cloned();
    let status = sub_matches.get_one::<String>(PARAMETER_STATUS).cloned();
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied();

    let mut ctx = ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let progress = crate::terminal::spinner("Fetching reports...");
    let reports = ctx
        .api()
        .list_reports(
            &tenant_uuid,
            report_type.as_deref(),
            status.as_deref(),
            limit,
        )
        .await;
    progress.finish_and_clear();
    let reports = reports?;
    crate::format::print_output(&ReportList { reports }.format(format)?);
    Ok(())
}

/// `report get --id ID`.
pub async fn get_report(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"report get\" command...");
    let format = crate::actions::tenants::plain_format(sub_matches)?;
    let id = report_id(sub_matches)?;
    let mut ctx = ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let report = ctx.api().get_report(&tenant_uuid, &id).await?;
    crate::format::print_output(&report.format(format)?);
    Ok(())
}

/// A file name a report can be saved under: its name, or its id, with every
/// path separator and other awkward character replaced.
fn safe_file_stem(report: &Report) -> String {
    let stem: String = report
        .display_name()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let stem = stem.trim().trim_matches('.').to_string();
    if stem.is_empty() {
        report.id.clone()
    } else {
        stem
    }
}

/// `report download --id ID --format csv|xlsx [-o PATH]`.
///
/// Only a COMPLETED report has data to download; anything else is refused
/// before touching the disk (exit 69 while it is still running, 64 when it
/// failed or was cancelled).
pub async fn download_report(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"report download\" command...");
    let id = report_id(sub_matches)?;
    let file_format = sub_matches
        .get_one::<String>(PARAMETER_FILE_FORMAT)
        .cloned()
        .unwrap_or_else(|| "csv".to_string());
    let mut ctx = ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();

    let report = ctx.api().get_report(&tenant_uuid, &id).await?;
    match report.status {
        JobStatus::Completed => {}
        JobStatus::Pending | JobStatus::Running => {
            crate::error_utils::report_error(&format!(
                "Report '{}' is {} ({}% done); only a COMPLETED report can be downloaded. Try again later, or 'report create --wait' next time.",
                report.display_name(),
                report.status.as_str(),
                report.progress
            ));
            return Err(CliError::AlreadyReported(PcliExitCode::TempFail));
        }
        JobStatus::Failed | JobStatus::Cancelled => {
            return Err(CliActionError::BusinessLogicError(format!(
                "Report '{}' is {}; there is nothing to download. 'report diagnose --id {}' explains a failure.",
                report.display_name(),
                report.status.as_str(),
                report.id
            ))
            .into());
        }
    }

    let dest = sub_matches
        .get_one::<std::path::PathBuf>(crate::commands::params::PARAMETER_OUTPUT)
        .cloned()
        .unwrap_or_else(|| {
            std::path::PathBuf::from(format!("{}.{}", safe_file_stem(&report), file_format))
        });

    let progress = crate::terminal::spinner("Downloading report...");
    let written = ctx
        .api()
        .download_report_to_file(&tenant_uuid, &id, &file_format, &dest)
        .await;
    progress.finish_and_clear();
    let written = written?;
    trace!("Wrote {} bytes to {}", written, dest.display());
    // No output on success (following UNIX convention), like `asset download`.
    Ok(())
}

/// `report diagnose --id ID`.
pub async fn diagnose_report(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"report diagnose\" command...");
    let format = crate::actions::tenants::plain_format(sub_matches)?;
    let id = report_id(sub_matches)?;
    let mut ctx = ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();

    let report = ctx.api().get_report(&tenant_uuid, &id).await?;
    let progress = crate::terminal::spinner("Searching failure logs...");
    let diagnostics = ctx
        .api()
        .get_report_failure_diagnostics(&tenant_uuid, &id)
        .await;
    progress.finish_and_clear();
    let diagnostics = diagnostics?;

    if diagnostics.status == FailureDiagnosticsStatus::Unavailable {
        return Err(CliError::FeatureUnavailable(
            UNAVAILABLE_MESSAGE.to_string(),
        ));
    }
    let record = ReportFailureDiagnostics::new(&report, diagnostics);
    if record.status == FailureDiagnosticsStatus::NotFound {
        let note = match report.status {
            JobStatus::Failed => "No failure record found: the log entry has aged out of retention, or the failure was not logged.".to_string(),
            other => format!(
                "No failure record found: the report is {}, not FAILED, so there is nothing to diagnose.",
                other.as_str()
            ),
        };
        eprintln!("{}", note);
    }
    crate::format::print_output(&record.format(format)?);
    Ok(())
}

/// `report delete --id ID [--dry-run]`. Confirms unless `--yes`; silent on success.
pub async fn delete_report(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"report delete\" command...");
    let id = report_id(sub_matches)?;
    let yes_flag = sub_matches.get_flag("yes");
    let mut ctx = ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();

    let report = ctx.api().get_report(&tenant_uuid, &id).await?;
    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would delete report '{}' ({}, {})",
            report.display_name(),
            report.id,
            report.status.as_str()
        );
        return Ok(());
    }
    if !yes_flag
        && !crate::terminal::confirm(
            &format!("Delete report '{}' ({})?", report.display_name(), report.id),
            Some("This action cannot be undone"),
        )?
    {
        eprintln!("Deletion cancelled.");
        return Ok(());
    }
    ctx.api().delete_report(&tenant_uuid, &id).await?;
    Ok(())
}

/// `report create ...`: start a duplication report, optionally wait for it.
pub async fn create_report(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"report create\" command...");
    let format = crate::actions::tenants::plain_format(sub_matches)?;
    let name = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_NAME)
        .cloned();
    let min_threshold = sub_matches
        .get_one::<f64>(PARAMETER_MIN_THRESHOLD)
        .copied()
        .unwrap_or(80.0);
    let max_threshold = sub_matches
        .get_one::<f64>(PARAMETER_MAX_THRESHOLD)
        .copied()
        .unwrap_or(100.0);
    if !(0.0..=100.0).contains(&min_threshold)
        || !(0.0..=100.0).contains(&max_threshold)
        || min_threshold > max_threshold
    {
        return Err(CliActionError::BusinessLogicError(format!(
            "thresholds must be between 0 and 100 with --min-threshold <= --max-threshold (got {} and {})",
            min_threshold, max_threshold
        ))
        .into());
    }
    let many = |id: &str| -> Vec<String> {
        sub_matches
            .get_many::<String>(id)
            .map(|v| v.cloned().collect())
            .unwrap_or_default()
    };
    let folder_paths = many(crate::commands::params::PARAMETER_FOLDER_PATH);
    let excluded_paths = many(PARAMETER_EXCLUDE_FOLDER_PATH);
    let extensions: Vec<String> = many(crate::commands::params::PARAMETER_EXTENSION)
        .into_iter()
        .map(|e| e.trim_start_matches('.').to_string())
        .collect();
    let mut include_home = sub_matches.get_flag(PARAMETER_INCLUDE_HOME);
    let wait = sub_matches.get_flag(PARAMETER_WAIT);
    let poll_interval = sub_matches
        .get_one::<u64>(PARAMETER_POLL_INTERVAL)
        .copied()
        .unwrap_or(5);

    let mut ctx = ExecutionContext::from_args(sub_matches).await?;
    let tenant = ctx.tenant().clone();

    // Folder paths become ids; the root has none, so "/" means the root assets.
    let mut folder_ids = Vec::new();
    for path in &folder_paths {
        if crate::model::normalize_path(path) == "/" {
            if !include_home {
                crate::error_utils::report_warning(
                    &"--folder-path / has no folder id; it is read as --include-home (the root's own assets)",
                );
            }
            include_home = true;
            continue;
        }
        let id =
            crate::actions::folders::resolve_folder_uuid_by_path(ctx.api(), &tenant, path).await?;
        folder_ids.push(id.to_string());
    }
    let mut excluded_folder_ids = Vec::new();
    for path in &excluded_paths {
        let id =
            crate::actions::folders::resolve_folder_uuid_by_path(ctx.api(), &tenant, path).await?;
        excluded_folder_ids.push(id.to_string());
    }

    let request = CreateDuplicationReportRequest {
        name,
        min_threshold,
        max_threshold,
        folder_ids,
        excluded_folder_ids,
        extensions,
        exclude_assemblies: sub_matches.get_flag(PARAMETER_EXCLUDE_ASSEMBLIES),
        exclude_exact_duplicates: sub_matches.get_flag(PARAMETER_EXCLUDE_EXACT_DUPLICATES),
        include_home_folder_assets: include_home,
    };
    let mut report = ctx
        .api()
        .create_duplication_report(&tenant.uuid, &request)
        .await?;

    if wait {
        report = wait_for_report(&mut ctx, &tenant.uuid, &report, poll_interval).await?;
    }
    crate::format::print_output(&report.format(format)?);

    if wait && !matches!(report.status, JobStatus::Completed) {
        crate::error_utils::report_error(&format!(
            "Report '{}' ended {}. 'report diagnose --id {}' explains a failure.",
            report.display_name(),
            report.status.as_str(),
            report.id
        ));
        return Err(CliError::AlreadyReported(PcliExitCode::TempFail));
    }
    Ok(())
}

/// Poll a report until it reaches a terminal status.
///
/// Progress goes to stderr. A credential failure is fatal at once; any other
/// error is retried, and three consecutive ones give up with the last error.
async fn wait_for_report(
    ctx: &mut ExecutionContext,
    tenant_uuid: &Uuid,
    report: &Report,
    poll_interval: u64,
) -> Result<Report, CliError> {
    let id = Uuid::parse_str(&report.id).map_err(|e| {
        CliError::PhysnaExtendedApiError(ApiError::InvalidParameterError(format!(
            "the server sent a report id that is not a UUID ({}): {}",
            report.id, e
        )))
    })?;
    let mut latest = report.clone();
    let mut consecutive_failures: u32 = 0;
    while !latest.status.is_terminal() {
        eprintln!(
            "Report '{}': {} {}%",
            latest.display_name(),
            latest.status.as_str(),
            latest.progress
        );
        tokio::time::sleep(std::time::Duration::from_secs(poll_interval)).await;
        match ctx.api().get_report(tenant_uuid, &id).await {
            Ok(report) => {
                consecutive_failures = 0;
                latest = report;
            }
            Err(e) if e.is_credential_failure() => return Err(e.into()),
            Err(e) => {
                consecutive_failures += 1;
                crate::error_utils::report_warning(&format!(
                    "Polling the report failed ({} of {} in a row): {}",
                    consecutive_failures, POLL_FAILURE_THRESHOLD, e
                ));
                if consecutive_failures >= POLL_FAILURE_THRESHOLD {
                    return Err(e.into());
                }
            }
        }
    }
    eprintln!(
        "Report '{}': {} {}%",
        latest.display_name(),
        latest.status.as_str(),
        latest.progress
    );
    Ok(latest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ReportType;

    fn report(name: Option<&str>) -> Report {
        Report {
            id: "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1".to_string(),
            tenant_id: String::new(),
            status: JobStatus::Completed,
            progress: 100.0,
            report_type: ReportType::Duplication,
            min_threshold: 80.0,
            max_threshold: 100.0,
            created_at: String::new(),
            updated_at: String::new(),
            group_count: 0.0,
            name: name.map(|n| n.to_string()),
            creator: None,
            extensions: None,
            folder_ids: None,
            folder_paths: None,
            excluded_folder_ids: None,
            exclude_assemblies: None,
            exclude_exact_duplicates: None,
            include_home_folder_assets: None,
            metadata_filters: None,
            search_query: None,
            model_index_config_id: None,
        }
    }

    #[test]
    fn the_default_file_name_is_the_report_name_made_safe() {
        assert_eq!(
            safe_file_stem(&report(Some("Custom 9/21/2026"))),
            "Custom 9_21_2026"
        );
        assert_eq!(safe_file_stem(&report(Some("  a:b*c  "))), "a_b_c");
    }

    #[test]
    fn a_nameless_or_blank_named_report_falls_back_to_its_id() {
        assert_eq!(
            safe_file_stem(&report(None)),
            "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1"
        );
        assert_eq!(
            safe_file_stem(&report(Some(" . "))),
            "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1"
        );
    }
}

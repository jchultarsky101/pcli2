//! Report command definitions.
//!
//! Reports are server-side jobs (duplication, simplification, custom). This
//! group lists them, shows one, downloads a finished one, explains a failed
//! one, deletes one, and starts a duplication report.

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::commands::params::{
    dry_run_parameter, format_parameter, format_pretty_parameter, format_with_headers_parameter,
    output_file_parameter, tenant_parameter, COMMAND_CREATE, COMMAND_DELETE, COMMAND_DIAGNOSE,
    COMMAND_DOWNLOAD, COMMAND_GET, COMMAND_LIST, PARAMETER_EXTENSION, PARAMETER_FOLDER_PATH,
    PARAMETER_LIMIT, PARAMETER_NAME,
};

pub const COMMAND_REPORT: &str = "report";
pub const PARAMETER_ID: &str = "id";
pub const PARAMETER_TYPE: &str = "type";
pub const PARAMETER_STATUS: &str = "status";
pub const PARAMETER_FILE_FORMAT: &str = "format";
pub const PARAMETER_MIN_THRESHOLD: &str = "min-threshold";
pub const PARAMETER_MAX_THRESHOLD: &str = "max-threshold";
pub const PARAMETER_EXCLUDE_FOLDER_PATH: &str = "exclude-folder-path";
pub const PARAMETER_EXCLUDE_ASSEMBLIES: &str = "exclude-assemblies";
pub const PARAMETER_EXCLUDE_EXACT_DUPLICATES: &str = "exclude-exact-duplicates";
pub const PARAMETER_INCLUDE_HOME: &str = "include-home";
pub const PARAMETER_WAIT: &str = "wait";
pub const PARAMETER_POLL_INTERVAL: &str = "poll-interval";

/// `--id`: a report's UUID.
fn report_id_parameter() -> Arg {
    Arg::new(PARAMETER_ID)
        .long(PARAMETER_ID)
        .num_args(1)
        .required(true)
        .value_parser(clap::value_parser!(uuid::Uuid))
        .help("The report's ID (see 'report list')")
}

/// `--limit` without a default: the whole listing unless the user caps it.
fn open_limit_parameter() -> Arg {
    Arg::new(PARAMETER_LIMIT)
        .long(PARAMETER_LIMIT)
        .num_args(1)
        .required(false)
        .value_parser(clap::value_parser!(usize))
        .help("Stop after this many reports (default: all)")
}

fn output_format_args(cmd: Command) -> Command {
    cmd.arg(format_parameter().value_parser(["json", "csv"]))
        .arg(format_pretty_parameter())
        .arg(format_with_headers_parameter())
}

/// Define the report command and its subcommands.
pub fn report_command() -> Command {
    Command::new(COMMAND_REPORT)
        .about("Manage reports: duplication, simplification and custom report jobs")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(output_format_args(
            Command::new(COMMAND_LIST)
                .about("List the tenant's reports, newest first")
                .visible_alias("ls")
                .arg(tenant_parameter())
                .arg(
                    Arg::new(PARAMETER_TYPE)
                        .long(PARAMETER_TYPE)
                        .num_args(1)
                        .value_parser(crate::model::ReportType::ALL)
                        .help("Only reports of this type"),
                )
                .arg(
                    Arg::new(PARAMETER_STATUS)
                        .long(PARAMETER_STATUS)
                        .num_args(1)
                        .value_parser(crate::model::JobStatus::ALL)
                        .help("Only reports in this status"),
                )
                .arg(open_limit_parameter()),
        ))
        .subcommand(output_format_args(
            Command::new(COMMAND_GET)
                .about("Show one report: status, progress, settings")
                .arg(tenant_parameter())
                .arg(report_id_parameter()),
        ))
        .subcommand(
            Command::new(COMMAND_DOWNLOAD)
                .visible_alias("dl")
                .about("Download a completed report's data as a CSV or XLSX file")
                .arg(tenant_parameter())
                .arg(report_id_parameter())
                .arg(
                    Arg::new(PARAMETER_FILE_FORMAT)
                        .short('f')
                        .long(PARAMETER_FILE_FORMAT)
                        .num_args(1)
                        .required(true)
                        .value_parser(["csv", "xlsx"])
                        .help("The file format to download"),
                )
                .arg(output_file_parameter().help(
                    "Output file path (default: the report's name, or its ID, plus the extension)",
                )),
        )
        .subcommand(output_format_args(
            Command::new(COMMAND_DIAGNOSE)
                .visible_alias("why")
                .about("Explain why a report failed, from the server's failure diagnostics")
                .long_about(
                    "Explain why a report failed, from the server's failure diagnostics.\n\n\
                     The server searches its job logs on demand, so the answer is only as durable \
                     as log retention. STATUS is `found` when the failure was located, `not-found` \
                     when the report did not fail or the log entry has aged out (exit 0 either way); \
                     the command exits 68 when this deployment has no failure log search.",
                )
                .arg(tenant_parameter())
                .arg(report_id_parameter()),
        ))
        .subcommand(
            Command::new(COMMAND_DELETE)
                .visible_alias("rm")
                .about("Delete a report (asks for confirmation unless --yes)")
                .arg(tenant_parameter())
                .arg(report_id_parameter())
                .arg(dry_run_parameter()),
        )
        .subcommand(output_format_args(
            Command::new(COMMAND_CREATE)
                .about("Start a duplication report; --wait follows it to the end")
                .long_about(
                    "Start a duplication report over the given folders (their subfolders \
                     included) and print the report record. Reports run on the server; use \
                     --wait to poll until it completes, then 'report download' for the data. \
                     A finished report that failed exits 69 and points at 'report diagnose'.",
                )
                .arg(tenant_parameter())
                .arg(
                    Arg::new(PARAMETER_NAME)
                        .long(PARAMETER_NAME)
                        .num_args(1)
                        .help("A name for the report"),
                )
                .arg(
                    Arg::new(PARAMETER_MIN_THRESHOLD)
                        .long(PARAMETER_MIN_THRESHOLD)
                        .num_args(1)
                        .default_value("80")
                        .value_parser(clap::value_parser!(f64))
                        .help("Minimum similarity for two assets to count as duplicates (0 to 100)"),
                )
                .arg(
                    Arg::new(PARAMETER_MAX_THRESHOLD)
                        .long(PARAMETER_MAX_THRESHOLD)
                        .num_args(1)
                        .default_value("100")
                        .value_parser(clap::value_parser!(f64))
                        .help("Maximum similarity to include (0 to 100)"),
                )
                .arg(
                    Arg::new(PARAMETER_FOLDER_PATH)
                        .long(PARAMETER_FOLDER_PATH)
                        .num_args(1)
                        .action(ArgAction::Append)
                        .help("Folder to search, subfolders included (repeatable); '/' means the root assets, see --include-home"),
                )
                .arg(
                    Arg::new(PARAMETER_EXCLUDE_FOLDER_PATH)
                        .long(PARAMETER_EXCLUDE_FOLDER_PATH)
                        .num_args(1)
                        .action(ArgAction::Append)
                        .help("Subfolder of a searched folder to leave out, with its own subfolders (repeatable)"),
                )
                .arg(
                    Arg::new(PARAMETER_EXTENSION)
                        .long(PARAMETER_EXTENSION)
                        .num_args(1)
                        .action(ArgAction::Append)
                        .help("Only assets with this file extension, e.g. stl (repeatable)"),
                )
                .arg(
                    Arg::new(PARAMETER_EXCLUDE_ASSEMBLIES)
                        .long(PARAMETER_EXCLUDE_ASSEMBLIES)
                        .action(ArgAction::SetTrue)
                        .help("Leave assemblies out of the report groups"),
                )
                .arg(
                    Arg::new(PARAMETER_EXCLUDE_EXACT_DUPLICATES)
                        .long(PARAMETER_EXCLUDE_EXACT_DUPLICATES)
                        .action(ArgAction::SetTrue)
                        .help("Leave out exact duplicates (100% match with the same file name)"),
                )
                .arg(
                    Arg::new(PARAMETER_INCLUDE_HOME)
                        .long(PARAMETER_INCLUDE_HOME)
                        .action(ArgAction::SetTrue)
                        .help("Include the assets in the root folder that have no folder of their own"),
                )
                .arg(
                    Arg::new(PARAMETER_WAIT)
                        .long(PARAMETER_WAIT)
                        .action(ArgAction::SetTrue)
                        .help("Poll the report until it is COMPLETED, FAILED or CANCELLED before printing it"),
                )
                .arg(
                    Arg::new(PARAMETER_POLL_INTERVAL)
                        .long(PARAMETER_POLL_INTERVAL)
                        .num_args(1)
                        .default_value("5")
                        .value_parser(clap::value_parser!(u64).range(1..=3600))
                        .help("Seconds between polls with --wait"),
                ),
        ))
}

/// Execute report subcommands based on the provided arguments.
pub async fn execute_report_command(matches: &ArgMatches) -> Result<(), crate::error::CliError> {
    match matches.subcommand() {
        Some((COMMAND_LIST, sub_matches)) => {
            crate::actions::reports::list_reports(sub_matches).await
        }
        Some((COMMAND_GET, sub_matches)) => crate::actions::reports::get_report(sub_matches).await,
        Some((COMMAND_DOWNLOAD, sub_matches)) => {
            crate::actions::reports::download_report(sub_matches).await
        }
        Some((COMMAND_DIAGNOSE, sub_matches)) => {
            crate::actions::reports::diagnose_report(sub_matches).await
        }
        Some((COMMAND_DELETE, sub_matches)) => {
            crate::actions::reports::delete_report(sub_matches).await
        }
        Some((COMMAND_CREATE, sub_matches)) => {
            crate::actions::reports::create_report(sub_matches).await
        }
        _ => Err(crate::error::CliError::UnsupportedSubcommand(
            matches
                .subcommand()
                .map(|(name, _)| name.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        )),
    }
}

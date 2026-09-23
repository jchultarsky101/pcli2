//! Tenant command definitions.
//!
//! This module defines CLI commands related to tenant management.

use crate::commands::params::{
    dry_run_parameter, format_parameter, format_pretty_parameter, format_with_headers_parameter,
    format_with_metadata_parameter, tenant_name_parameter, tenant_parameter, COMMAND_CLEAR,
    COMMAND_DELETE, COMMAND_FAILURES, COMMAND_GET, COMMAND_LIST, COMMAND_METADATA, COMMAND_TENANT,
    COMMAND_USE, PARAMETER_EXTENSION, PARAMETER_FOLDER_PATH, PARAMETER_KIND, PARAMETER_LIMIT,
    PARAMETER_NAME, PARAMETER_NEW_NAME,
};
use clap::{Arg, Command};

/// Create the tenant command with all its subcommands.
pub fn tenant_command() -> Command {
    Command::new(COMMAND_TENANT)
        .about("Manage tenants")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all tenants")
                .visible_alias("ls")
                .arg(format_parameter().value_parser(["json", "csv", "table"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new("state")
                .about("Get asset state counts for the current tenant")
                .arg(crate::commands::params::tenant_parameter())
                .arg(
                    clap::Arg::new("type")
                        .long("type")
                        .num_args(1)
                        .required(false)
                        .value_parser(["indexing", "finished", "failed", "unsupported", "no-3d-data", "missing-dependencies"])
                        .help("Filter assets by state: indexing, finished, failed, unsupported, no-3d-data, or missing-dependencies"),
                )
                .arg(format_parameter().value_parser(["json", "csv", "table"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_FAILURES)
                .about("List recent failures (assets, reports, part-finder reports), newest first")
                .long_about(
                    "List recent failures (assets, reports, part-finder reports), newest first.\n\n\
                     JSON output also carries the tenant-wide totals per kind. The ID of an `asset` \
                     row is what 'asset diagnose --uuid' takes to explain the failure.",
                )
                .arg(tenant_parameter())
                .arg(
                    Arg::new(PARAMETER_KIND)
                        .long(PARAMETER_KIND)
                        .num_args(1)
                        .action(clap::ArgAction::Append)
                        .required(false)
                        .value_parser(crate::model::FailureSource::ALL)
                        .help("Only failures of this kind (repeatable): asset, report, part-finder-report"),
                )
                .arg(
                    Arg::new(PARAMETER_LIMIT)
                        .long(PARAMETER_LIMIT)
                        .num_args(1)
                        .required(false)
                        .value_parser(clap::value_parser!(usize))
                        .help("Stop after this many failures (default: all of them)"),
                )
                .arg(format_parameter().value_parser(["json", "csv", "table"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new("usage")
                .about("How much the tenant used Physna: searches, downloads, active users, assets")
                .long_about(
                    "How much the tenant used Physna over a period of UTC days: searches (by type), \
                     compares, downloads, uploads, reports (by type), active users and per-feature \
                     counts, plus how many assets of each type the tenant holds now (demo assets \
                     uploaded by Physna are not counted).\n\n\
                     The period is the last 30 days by default; --days, --from and --to change it, \
                     up to 366 days. With --daily the output is one row per day instead.\n\n\
                     CSV and table output have one CATEGORY,NAME,COUNT row per number, so a count \
                     Physna adds later is a new row, not a new column. Requires the tenant admin role.",
                )
                .after_help(crate::commands::examples(&[
                    ("The last 30 days", "pcli2 tenant usage"),
                    ("One quarter, as JSON", "pcli2 tenant usage --from 2026-07-01 --to 2026-09-30 --format json"),
                    ("Active users per day over the last week", "pcli2 tenant usage --days 7 --daily --columns DATE,ACTIVE_USERS"),
                ]))
                .arg(tenant_parameter())
                .arg(
                    Arg::new("from")
                        .long("from")
                        .num_args(1)
                        .value_name("YYYY-MM-DD")
                        .value_parser(parse_utc_day)
                        .conflicts_with("days")
                        .help("First UTC day of the period (default: 29 days before --to, 30 days in all)"),
                )
                .arg(
                    Arg::new("to")
                        .long("to")
                        .num_args(1)
                        .value_name("YYYY-MM-DD")
                        .value_parser(parse_utc_day)
                        .help("Last UTC day of the period (default: today)"),
                )
                .arg(
                    Arg::new("days")
                        .long("days")
                        .num_args(1)
                        .value_parser(clap::value_parser!(u32).range(1..=366))
                        .help("The period is this many days, ending with --to (default: 30)"),
                )
                .arg(
                    Arg::new("daily")
                        .long("daily")
                        .action(clap::ArgAction::SetTrue)
                        .help("One row per day: date, searches, compares, downloads, uploads, reports, active users"),
                )
                .arg(format_parameter().value_parser(["json", "csv", "table"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_USE)
                .about("Set the active tenant")
                .after_help(crate::commands::examples(&[
                    ("Pick from a list", "pcli2 tenant use"),
                    ("By short name, in a script", "pcli2 tenant use --name acme"),
                ]))
                .visible_alias("select")
                .arg(tenant_name_parameter()) // --name (tenant short name)
                .arg(crate::commands::params::refresh_parameter()) // --refresh flag to force refresh tenant list
                .arg(format_parameter().value_parser(["json", "csv", "table"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .visible_alias("current")
                .about("Get the active tenant")
                .arg(format_parameter().value_parser(["json", "csv", "tree", "table"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_CLEAR)
                .about("Clear the active tenant")
                .visible_alias("unset"),
        )
        .subcommand(
            Command::new(COMMAND_METADATA)
                .about("Inspect the tenant's metadata-field registry")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_LIST)
                        .about("List all metadata fields registered in the tenant with their types")
                        .visible_alias("ls")
                        .long_about(
                            "List every metadata field registered in the tenant along with its \
                            data type (text, number, boolean, url, ...).\n\n\
                            The CSV output uses the same column headers as the classic \
                            'asset metadata create-batch' input (ASSET_PATH,NAME,VALUE,TYPE), \
                            with the NAME and TYPE columns filled from the registry and the \
                            ASSET_PATH and VALUE columns left empty. This makes it easy to save \
                            the listing and turn it into a batch-upload file: replicate each row \
                            per asset, then fill in ASSET_PATH and VALUE.\n\n\
                            Example:\n\
                            pcli2 tenant metadata list --format csv --headers > fields.csv",
                        )
                        .arg(tenant_parameter())
                        .arg(format_parameter().value_parser(["json", "csv", "tree", "table"]))
                        .arg(format_pretty_parameter())
                        .arg(format_with_headers_parameter()),
                )
                .subcommand(
                    Command::new("rename")
                        .about("Rename a metadata field (its values on assets are kept)")
                        .arg(tenant_parameter())
                        .arg(metadata_field_name_parameter())
                        .arg(
                            Arg::new(PARAMETER_NEW_NAME)
                                .long(PARAMETER_NEW_NAME)
                                .num_args(1)
                                .required(true)
                                .help("The field's new name"),
                        )
                        .arg(dry_run_parameter()),
                )
                .subcommand(
                    Command::new(COMMAND_DELETE)
                        .visible_alias("rm")
                        .about("Delete a metadata field; --force also removes its values from every asset")
                        .long_about(
                            "Delete a metadata field from the tenant's registry.\n\n\
                             Without --force the server refuses a field that assets still use. With \
                             --force the field is deleted and its value is removed from every asset \
                             that had one. Asks for confirmation unless --yes is given.",
                        )
                        .arg(tenant_parameter())
                        .arg(metadata_field_name_parameter())
                        .arg(
                            Arg::new("force")
                                .long("force")
                                .action(clap::ArgAction::SetTrue)
                                .help("Also remove the field's values from every asset that has one"),
                        )
                        .arg(dry_run_parameter()),
                )
                .subcommand(
                    Command::new("assets")
                        .about("List the assets that have a value for a metadata field")
                        .arg(tenant_parameter())
                        .arg(metadata_field_name_parameter())
                        .arg(open_limit_parameter("Stop after this many assets (default: all)"))
                        .arg(format_with_metadata_parameter())
                        .arg(format_with_headers_parameter())
                        .arg(format_pretty_parameter())
                        .arg(format_parameter().value_parser(["json", "csv", "table"])),
                )
                .subcommand(
                    Command::new("coverage")
                        .about("How many of the tenant's assets carry at least one metadata value")
                        .arg(tenant_parameter())
                        .arg(format_parameter().value_parser(["json", "csv", "table"]))
                        .arg(format_pretty_parameter())
                        .arg(format_with_headers_parameter()),
                )
                .subcommand(
                    Command::new("missing")
                        .about("List the assets that have no metadata at all, oldest first")
                        .long_about(
                            "List the assets that have no metadata value at all, oldest first. \
                             Demo assets uploaded by Physna are left out. Narrow the listing with \
                             --folder-path and --extension (both repeatable).",
                        )
                        .arg(tenant_parameter())
                        .arg(
                            Arg::new(PARAMETER_FOLDER_PATH)
                                .long(PARAMETER_FOLDER_PATH)
                                .num_args(1)
                                .action(clap::ArgAction::Append)
                                .help("Only assets under this folder path (repeatable)"),
                        )
                        .arg(
                            Arg::new(PARAMETER_EXTENSION)
                                .long(PARAMETER_EXTENSION)
                                .num_args(1)
                                .action(clap::ArgAction::Append)
                                .help("Only assets with this file extension, e.g. stl (repeatable)"),
                        )
                        .arg(open_limit_parameter("Stop after this many assets (default: all)"))
                        .arg(format_with_metadata_parameter())
                        .arg(format_with_headers_parameter())
                        .arg(format_pretty_parameter())
                        .arg(format_parameter().value_parser(["json", "csv", "table"])),
                ),
        )
}

/// A `YYYY-MM-DD` date, as the activity endpoints take it.
fn parse_utc_day(value: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("'{}' is not a date in the form YYYY-MM-DD", value))
}

/// `--name`: a metadata field, by its exact registered name.
fn metadata_field_name_parameter() -> Arg {
    Arg::new(PARAMETER_NAME)
        .long(PARAMETER_NAME)
        .num_args(1)
        .required(true)
        .help("The metadata field's name, exactly as 'tenant metadata list' shows it")
}

/// `--limit` without a default: the whole listing unless the user caps it.
fn open_limit_parameter(help: &'static str) -> Arg {
    Arg::new(PARAMETER_LIMIT)
        .long(PARAMETER_LIMIT)
        .num_args(1)
        .required(false)
        .value_parser(clap::value_parser!(usize))
        .help(help)
}

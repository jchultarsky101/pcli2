//! Folder command definitions.
//!
//! This module defines CLI commands related to folder management.

use crate::commands::params::{
    dry_run_parameter, folder_identifier_group, folder_path_parameter, folder_uuid_parameter,
    format_parameter, format_pretty_parameter, format_with_headers_parameter,
    format_with_metadata_parameter, limit_parameter, name_parameter, output_file_parameter,
    parent_folder_identifier_group, parent_folder_path_parameter, parent_folder_uuid_parameter,
    tenant_parameter, COMMAND_CREATE, COMMAND_DELETE, COMMAND_FOLDER, COMMAND_GET, COMMAND_LIST,
    COMMAND_MATCH, COMMAND_PART_MATCH, COMMAND_VISUAL_MATCH, FORMAT_CSV, FORMAT_JSON, FORMAT_XLS,
    PARAMETER_PROGRESS,
};
use clap::Command;

/// Create the folder command with all its subcommands.
pub fn folder_command() -> Command {
    Command::new(COMMAND_FOLDER)
        .about("Manage folders")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_CREATE)
                .about("Create a new folder")
                .visible_alias("add")
                .arg(tenant_parameter())
                .arg(name_parameter())
                .arg(parent_folder_path_parameter())
                .arg(parent_folder_uuid_parameter())
                .group(parent_folder_identifier_group())
                .arg(
                    clap::Arg::new("description")
                        .long("description")
                        .num_args(1)
                        .help("An optional description for the new folder (up to 255 characters)"),
                ),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get folder details")
                .visible_alias("cat")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(format_with_metadata_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all folders")
                .after_help(crate::commands::examples(&[
                    ("The folder tree", "pcli2 folder list --format tree"),
                    ("One folder's subfolders as CSV", "pcli2 folder list --folder-path /Home/Parts --format csv --headers"),
                ]))
                .visible_alias("ls")
                .arg(tenant_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_UUID)
                        .long(crate::commands::params::PARAMETER_FOLDER_UUID)
                        .num_args(1)
                        .required(false)
                        .value_parser(clap::value_parser!(uuid::Uuid))
                        .help("Folder UUID")
                )
                .arg(folder_path_parameter().required(false))
                .arg(crate::commands::params::reload_parameter())
                .group(
                    clap::ArgGroup::new("folder-identifier")
                        .args([crate::commands::params::PARAMETER_FOLDER_UUID, crate::commands::params::PARAMETER_FOLDER_PATH])
                        .multiple(false)
                        .required(false)  // Make the group optional
                ),
        )
        .subcommand(
            Command::new(COMMAND_DELETE)
                .about("Delete a folder")
                .visible_alias("rm")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .arg(
                    clap::Arg::new("force")
                        .long("force")
                        .action(clap::ArgAction::SetTrue)
                        .help("Delete the folder together with every asset and subfolder in it (the server deletes recursively). Without it a non-empty folder is refused."),
                )
                .group(folder_identifier_group())
                .arg(dry_run_parameter()),
        )
        .subcommand(
            Command::new("rename")
                .about("Rename a folder")
                .visible_alias("ren")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .arg(name_parameter())
                .group(folder_identifier_group()),
        )
        .subcommand(
            Command::new("description")
                .about("Show, set or clear a folder's description")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Print a folder's description (nothing when it has none)")
                        .arg(tenant_parameter())
                        .arg(folder_uuid_parameter())
                        .arg(folder_path_parameter())
                        .group(folder_identifier_group()),
                )
                .subcommand(
                    Command::new("set")
                        .about("Set or replace a folder's description")
                        .after_help(crate::commands::examples(&[
                            (
                                "Describe a folder",
                                "pcli2 folder description set --folder-path /Projects/Rail --text \"Rail car parts, 2026 program\"",
                            ),
                            (
                                "Read it back",
                                "pcli2 folder description get --folder-path /Projects/Rail",
                            ),
                        ]))
                        .arg(tenant_parameter())
                        .arg(folder_uuid_parameter())
                        .arg(folder_path_parameter())
                        .group(folder_identifier_group())
                        .arg(
                            clap::Arg::new("text")
                                .long("text")
                                .num_args(1)
                                .required(true)
                                .help("The description: 1 to 255 characters (surrounding spaces are dropped)"),
                        ),
                )
                .subcommand(
                    Command::new("clear")
                        .visible_alias("rm")
                        .about("Remove a folder's description (nothing to do if it has none)")
                        .arg(tenant_parameter())
                        .arg(folder_uuid_parameter())
                        .arg(folder_path_parameter())
                        .group(folder_identifier_group()),
                ),
        )
        .subcommand(
            Command::new("move")
                .about("Move a folder to a new parent folder")
                .visible_alias("mv")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .arg(parent_folder_uuid_parameter().help("UUID of the folder to move it into"))
                .arg(parent_folder_path_parameter().help(
                    "Path of the folder to move it into; / for the root (e.g., /Home/Projects)",
                ))
                .group(folder_identifier_group())
                .group(parent_folder_identifier_group()),
        )
        .subcommand(
            Command::new("resolve")
                .about("Resolve a folder path to its UUID")
                .visible_alias("res")
                .arg(tenant_parameter())
                .arg(folder_path_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_RELOAD)
                        .long(crate::commands::params::PARAMETER_RELOAD)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Force refresh the folder cache from the API before resolving"),
                ),
        )
        .subcommand(
            Command::new("download")
                .about("Download all assets in a folder to a local directory")
                .visible_alias("dl")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(
                    crate::commands::params::output_file_parameter()
                        .help("Output directory path (default: <folder_name> directory in the current directory)"),
                )
                .arg(
                    clap::Arg::new(PARAMETER_PROGRESS)
                        .long(PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during download"),
                )
                .arg(
                    crate::commands::params::concurrent_parameter("4", "Maximum number of concurrent downloads (range: 1-10)"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .long(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Continue downloading other assets if one fails"),
                )
                .arg(crate::commands::params::delay_parameter("Delay in seconds between downloads (range: 0-180)"))
                .arg(
                    crate::commands::params::resume_parameter()
                )
        )
        .subcommand(
            Command::new(crate::commands::params::COMMAND_DEPENDENCIES)
                .about("Get dependencies for all assembly assets in one or more folders")
                .arg(tenant_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .num_args(1..) // Accept one or more values
                        .required(true)
                        .help("Folder path(s) to process (can be provided multiple times or as comma-separated values)")
                        .action(clap::ArgAction::Append), // Allow multiple --path flags
                )
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().value_parser([
                    crate::commands::params::FORMAT_JSON,
                    crate::commands::params::FORMAT_CSV,
                    crate::commands::params::FORMAT_TREE,
                    "table",
                ]))
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_PROGRESS)
                        .long(crate::commands::params::PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during processing"),
                )
        )
        .subcommand(
            Command::new(COMMAND_MATCH)
                .visible_aliases(["geometric-search", "gm"])
                .about("Find geometrically similar assets for all assets in one or more folders")
                .after_help(
                    "Rows are ordered by the unordered asset pair (reference UUID, then candidate UUID) in CSV and JSON output, and by MATCH_PERCENTAGE descending in Excel output. Two runs over unchanged data produce identical output.",
                )
                .arg(tenant_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .num_args(1..) // Accept one or more values
                        .required(true)
                        .help("Folder path(s) to process (can be provided multiple times or as comma-separated values)")
                        .action(clap::ArgAction::Append), // Allow multiple --path flags
                )
                .arg(crate::commands::params::recursive_parameter())
                .arg(
                    crate::commands::params::threshold_parameter("Similarity threshold (0.00 to 100.00)"),
                )
                .arg(
                    clap::Arg::new("exclusive")
                        .long("exclusive")
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Only show matches where both assets belong to the specified paths"),
                )
                .arg(
                    clap::Arg::new("groups")
                        .long("groups")
                        .action(clap::ArgAction::SetTrue)
                        .help("Group assets that match each other (directly or through a chain of matches): adds GROUP_ID and GROUP_SIZE as the last columns (groupId and groupSize in JSON)"),
                )
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV, FORMAT_XLS, "xlsx"]))
                .arg(output_file_parameter().help(
                    "Output file path, used with --format xls (default: match_report.xlsx)",
                ))
                .arg(
                    crate::commands::params::concurrent_parameter("4", "Maximum number of concurrent operations (range: 1-10)"),
                )
                .arg(crate::commands::params::checkpoint_parameter())
                .arg(
                    clap::Arg::new("progress")
                        .long("progress")
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during processing"),
                )
        )
        .subcommand(
            Command::new(COMMAND_PART_MATCH)
                .visible_aliases(["part-search", "pm"])
                .about("Find part matches for all assets in one or more folders")
                .after_help(
                    "Rows are ordered by the unordered asset pair (reference UUID, then candidate UUID). Two runs over unchanged data produce identical output.",
                )
                .arg(tenant_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .num_args(1..) // Accept one or more values
                        .required(true)
                        .help("Folder path(s) to process (can be provided multiple times or as comma-separated values)")
                        .action(clap::ArgAction::Append), // Allow multiple --path flags
                )
                .arg(crate::commands::params::recursive_parameter())
                .arg(
                    crate::commands::params::threshold_parameter("Similarity threshold (0.00 to 100.00)"),
                )
                .arg(
                    clap::Arg::new("exclusive")
                        .long("exclusive")
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Only show matches where both assets belong to the specified paths"),
                )
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().value_parser([crate::commands::params::FORMAT_JSON, crate::commands::params::FORMAT_CSV]))
                .arg(
                    crate::commands::params::concurrent_parameter("4", "Maximum number of concurrent operations (range: 1-10)"),
                )
                .arg(crate::commands::params::checkpoint_parameter())
                .arg(
                    clap::Arg::new("progress")
                        .long("progress")
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during processing"),
                )
        )
        .subcommand(
            Command::new(COMMAND_VISUAL_MATCH)
                .visible_aliases(["visual-search", "vm"])
                .about("Find visually similar assets for all assets in one or more folders")
                .after_help(
                    "Rows are ordered by the unordered asset pair (reference UUID, then candidate UUID). Two runs over unchanged data produce identical output.",
                )
                .arg(tenant_parameter())
                .arg(limit_parameter())
                .arg(
                    crate::commands::params::threshold_parameter("Size threshold (0.00 to 100.00): filters matches by geometric size relative to the reference asset; higher is stricter, 0 disables size filtering"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATH)
                        .num_args(1..) // Accept one or more values
                        .required(true)
                        .help("Folder path(s) to process (can be provided multiple times or as comma-separated values)")
                        .action(clap::ArgAction::Append), // Allow multiple --path flags
                )
                .arg(crate::commands::params::recursive_parameter())
                .arg(
                    clap::Arg::new("exclusive")
                        .long("exclusive")
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Only show matches where both assets belong to the specified paths"),
                )
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().value_parser([crate::commands::params::FORMAT_JSON, crate::commands::params::FORMAT_CSV]))
                .arg(
                    crate::commands::params::concurrent_parameter("4", "Maximum number of concurrent operations (range: 1-10)"),
                )
                .arg(crate::commands::params::checkpoint_parameter())
                .arg(
                    clap::Arg::new("progress")
                        .long("progress")
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during processing"),
                )
        )
        .subcommand(
            Command::new(crate::commands::params::COMMAND_UPLOAD)
                .about("Upload all assets from a local directory to a Physna folder")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(
                    crate::commands::params::input_parameter("Local directory containing the files to upload")
                        .required(true),
                )
                .arg(crate::commands::params::removed_parameter(
                    crate::commands::params::PARAMETER_LOCAL_PATH,
                    "--input",
                ))
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_SKIP_EXISTING)
                        .long(crate::commands::params::PARAMETER_SKIP_EXISTING)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Skip assets that already exist in the target folder instead of failing"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_PROGRESS)
                        .long(crate::commands::params::PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during upload"),
                )
                .arg(
                    crate::commands::params::concurrent_parameter("4", "Maximum number of concurrent uploads (range: 1-10)"),
                )
                .arg(crate::commands::params::delay_parameter("Delay in seconds between uploads (range: 0-180)"))
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .long(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Continue uploading other assets if one fails"),
                )
                .arg(dry_run_parameter())
        )
        .subcommand(
            Command::new(crate::commands::params::COMMAND_THUMBNAIL)
                .about("Download thumbnails for all assets in a folder")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(
                    crate::commands::params::output_file_parameter()
                        .help("Output directory path (default: <folder_name> directory in the current directory)"),
                )
                .arg(
                    clap::Arg::new(PARAMETER_PROGRESS)
                        .long(PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during download"),
                )
                .arg(
                    crate::commands::params::concurrent_parameter("4", "Maximum number of concurrent downloads (range: 1-10)"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .long(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Continue downloading other thumbnails if one fails"),
                )
                .arg(crate::commands::params::delay_parameter("Delay in seconds between downloads (range: 0-180)"))
        )
}

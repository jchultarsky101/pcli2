//! Folder command definitions.
//!
//! This module defines CLI commands related to folder management.

use crate::commands::params::{
    folder_identifier_group, folder_path_parameter, folder_uuid_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    name_parameter, parent_folder_identifier_group, parent_folder_path_parameter,
    parent_folder_uuid_parameter, tenant_parameter, COMMAND_CREATE, COMMAND_DELETE, COMMAND_FOLDER,
    COMMAND_GET, COMMAND_LIST, COMMAND_MATCH, COMMAND_PART_MATCH, COMMAND_VISUAL_MATCH,
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
                .arg(tenant_parameter())
                .arg(name_parameter())
                .arg(parent_folder_path_parameter())
                .arg(parent_folder_uuid_parameter())
                .group(parent_folder_identifier_group()),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get folder details")
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
                        .help("Resource's folder UUID")
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
                        .short('f')
                        .action(clap::ArgAction::SetTrue)
                        .help("Force deletion of non-empty folder by deleting all contents first"),
                )
                .group(folder_identifier_group()),
        )
        .subcommand(
            Command::new("rename")
                .about("Rename a folder")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .arg(name_parameter())
                .group(folder_identifier_group()),
        )
        .subcommand(
            Command::new("move")
                .about("Move a folder to a new parent folder")
                .visible_alias("mv")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .arg(parent_folder_uuid_parameter())
                .arg(parent_folder_path_parameter())
                .group(folder_identifier_group())
                .group(parent_folder_identifier_group()),
        )
        .subcommand(
            Command::new("resolve")
                .about("Resolve a folder path to its UUID")
                .arg(tenant_parameter())
                .arg(folder_path_parameter()),
        )
        .subcommand(
            Command::new("download")
                .about("Download all assets in a folder to a local directory")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_OUTPUT)
                        .long(crate::commands::params::PARAMETER_OUTPUT)
                        .num_args(1)
                        .required(false)
                        .help("Output directory path (default: <folder_name> directory in the current directory)")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
                .arg(
                    clap::Arg::new(PARAMETER_PROGRESS)
                        .long(PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during download"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONCURRENT)
                        .long(crate::commands::params::PARAMETER_CONCURRENT)
                        .num_args(1)
                        .required(false)
                        .default_value("1")
                        .help("Maximum number of concurrent downloads (range: 1-10)")
                        .value_parser(|s: &str| -> Result<usize, String> {
                            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
                            if !(1..=10).contains(&val) {
                                Err("Value must be between 1 and 10".to_string())
                            } else {
                                Ok(val)
                            }
                        }),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .long(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Continue downloading other assets if one fails"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_DELAY)
                        .long(crate::commands::params::PARAMETER_DELAY)
                        .num_args(1)
                        .required(false)
                        .default_value("0")
                        .help("Delay in seconds between downloads (range: 0-180)")
                        .value_parser(|s: &str| -> Result<usize, String> {
                            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
                            if val > 180 {
                                Err("Value must be between 0 and 180".to_string())
                            } else {
                                Ok(val)
                            }
                        }),
                )
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
                .visible_alias("geometric-search") // Add alias for geometric-search
                .about("Find geometrically similar assets for all assets in one or more folders")
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
                .arg(
                    clap::Arg::new("threshold")
                        .short('s')
                        .long("threshold")
                        .num_args(1)
                        .required(false)
                        .default_value("80.0")
                        .help("Similarity threshold (0.00 to 100.00)")
                        .value_parser(clap::value_parser!(f64)),
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
                    clap::Arg::new("concurrent")
                        .long("concurrent")
                        .num_args(1)
                        .required(false)
                        .default_value("1")
                        .help("Maximum number of concurrent operations (range: 1-10)")
                        .value_parser(clap::value_parser!(usize)),
                )
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
                .visible_alias("part-search") // Add alias for part-search
                .about("Find part matches for all assets in one or more folders")
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
                .arg(
                    clap::Arg::new("threshold")
                        .short('s')
                        .long("threshold")
                        .num_args(1)
                        .required(false)
                        .default_value("80.0")
                        .help("Similarity threshold (0.00 to 100.00)")
                        .value_parser(clap::value_parser!(f64)),
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
                    clap::Arg::new("concurrent")
                        .long("concurrent")
                        .num_args(1)
                        .required(false)
                        .default_value("1")
                        .help("Maximum number of concurrent operations (range: 1-10)")
                        .value_parser(clap::value_parser!(usize)),
                )
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
                .visible_alias("visual-search") // Add alias for visual-search
                .about("Find visually similar assets for all assets in one or more folders")
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
                    clap::Arg::new("concurrent")
                        .long("concurrent")
                        .num_args(1)
                        .required(false)
                        .default_value("1")
                        .help("Maximum number of concurrent operations (range: 1-10)")
                        .value_parser(clap::value_parser!(usize)),
                )
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
                    clap::Arg::new(crate::commands::params::PARAMETER_LOCAL_PATH)
                        .long(crate::commands::params::PARAMETER_LOCAL_PATH)
                        .num_args(1)
                        .required(true)
                        .help("Local directory path containing asset files to upload")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
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
                    clap::Arg::new(crate::commands::params::PARAMETER_CONCURRENT)
                        .long(crate::commands::params::PARAMETER_CONCURRENT)
                        .num_args(1)
                        .required(false)
                        .default_value("1")
                        .help("Maximum number of concurrent uploads (range: 1-10)")
                        .value_parser(|s: &str| -> Result<usize, String> {
                            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
                            if !(1..=10).contains(&val) {
                                Err("Value must be between 1 and 10".to_string())
                            } else {
                                Ok(val)
                            }
                        }),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_DELAY)
                        .long(crate::commands::params::PARAMETER_DELAY)
                        .num_args(1)
                        .required(false)
                        .default_value("0")
                        .help("Delay in seconds between uploads (range: 0-180)")
                        .value_parser(|s: &str| -> Result<usize, String> {
                            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
                            if val > 180 {
                                Err("Value must be between 0 and 180".to_string())
                            } else {
                                Ok(val)
                            }
                        }),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .long(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Continue uploading other assets if one fails"),
                )
        )
        .subcommand(
            Command::new(crate::commands::params::COMMAND_THUMBNAIL)
                .about("Download thumbnails for all assets in a folder")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_OUTPUT)
                        .long(crate::commands::params::PARAMETER_OUTPUT)
                        .num_args(1)
                        .required(false)
                        .help("Output directory path (default: <folder_name> directory in the current directory)")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
                .arg(
                    clap::Arg::new(PARAMETER_PROGRESS)
                        .long(PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during download"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONCURRENT)
                        .long(crate::commands::params::PARAMETER_CONCURRENT)
                        .num_args(1)
                        .required(false)
                        .default_value("1")
                        .help("Maximum number of concurrent downloads (range: 1-10)")
                        .value_parser(|s: &str| -> Result<usize, String> {
                            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
                            if !(1..=10).contains(&val) {
                                Err("Value must be between 1 and 10".to_string())
                            } else {
                                Ok(val)
                            }
                        }),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .long(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Continue downloading other thumbnails if one fails"),
                )
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_DELAY)
                        .long(crate::commands::params::PARAMETER_DELAY)
                        .num_args(1)
                        .required(false)
                        .default_value("0")
                        .help("Delay in seconds between downloads (range: 0-180)")
                        .value_parser(|s: &str| -> Result<usize, String> {
                            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
                            if val > 180 {
                                Err("Value must be between 0 and 180".to_string())
                            } else {
                                Ok(val)
                            }
                        }),
                )
        )
}

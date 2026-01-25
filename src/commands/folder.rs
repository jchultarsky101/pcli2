//! Folder command definitions.
//!
//! This module defines CLI commands related to folder management.

use crate::commands::params::{
    folder_identifier_group, folder_path_parameter, folder_uuid_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    name_parameter, parent_folder_identifier_group, parent_folder_path_parameter, parent_folder_uuid_parameter, tenant_parameter,
    COMMAND_CREATE, COMMAND_DELETE, COMMAND_FOLDER, COMMAND_GET,
    COMMAND_LIST, PARAMETER_PROGRESS, COMMAND_MATCH_FOLDER, COMMAND_PART_MATCH_FOLDER,
    COMMAND_VISUAL_MATCH_FOLDER, PARAMETER_FOLDER_PATHS,
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
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group()),
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
                .about("Download all assets in a folder as a ZIP archive")
                .arg(tenant_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_OUTPUT)
                        .long(crate::commands::params::PARAMETER_OUTPUT)
                        .num_args(1)
                        .required(false)
                        .help("Output file path (default: <folder_name>.zip in the current directory)")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
                .arg(
                    clap::Arg::new(PARAMETER_PROGRESS)
                        .long(PARAMETER_PROGRESS)
                        .action(clap::ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during download"),
                )
        )
        .subcommand(
            Command::new(COMMAND_MATCH_FOLDER)
                .visible_alias("geometric-search-folder") // Add alias for geometric-search-folder
                .about("Find geometrically similar assets for all assets in one or more folders")
                .arg(tenant_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATHS)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATHS)
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
            Command::new(COMMAND_PART_MATCH_FOLDER)
                .visible_alias("part-search-folder") // Add alias for part-search-folder
                .about("Find part matches for all assets in one or more folders")
                .arg(tenant_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATHS)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATHS)
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
            Command::new(COMMAND_VISUAL_MATCH_FOLDER)
                .visible_alias("visual-search-folder") // Add alias for visual-search-folder
                .about("Find visually similar assets for all assets in one or more folders")
                .arg(tenant_parameter())
                .arg(
                    clap::Arg::new(crate::commands::params::PARAMETER_FOLDER_PATHS)
                        .short('p')
                        .long(crate::commands::params::PARAMETER_FOLDER_PATHS)
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
}

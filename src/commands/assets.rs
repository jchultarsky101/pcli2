//! Asset command definitions.
//!
//! This module defines CLI commands related to asset management, including
//! upload, download, geometric matching, and metadata operations.

use crate::commands::metadata::metadata_command;
use crate::commands::params::{
    asset_identifier_group, asset_identifier_multiple_group, file_parameter,
    folder_identifier_group, folder_path_parameter, folder_uuid_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    multiple_files_parameter, path_parameter, recursive_parameter,
    tenant_parameter, uuid_parameter, COMMAND_ASSET, COMMAND_CREATE, COMMAND_CREATE_BATCH,
    COMMAND_DELETE, COMMAND_DEPENDENCIES, COMMAND_DOWNLOAD, COMMAND_DOWNLOAD_FOLDER, COMMAND_GET,
    COMMAND_LIST, COMMAND_MATCH, COMMAND_MATCH_FOLDER, FORMAT_CSV, FORMAT_JSON, FORMAT_TREE, PARAMETER_CONCURRENT,
    PARAMETER_FILE, PARAMETER_FOLDER_PATH, PARAMETER_PROGRESS,
};
use clap::{Arg, ArgAction, ArgGroup, Command};

/// Create the asset command with all its subcommands.
pub fn asset_command() -> Command {
    Command::new(COMMAND_ASSET)
        .about("Manage assets")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get asset details")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV]))
                .group(asset_identifier_multiple_group()),
        )
        .subcommand(
            Command::new(COMMAND_CREATE)
                .about("Create a new asset by uploading a file")
                .arg(tenant_parameter())
                .arg(file_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(format_with_metadata_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV])),
        )
        .subcommand(
            Command::new(COMMAND_CREATE_BATCH)
                .about("Create multiple assets by uploading files matching a glob pattern")
                .arg(tenant_parameter())
                .arg(multiple_files_parameter())
                .arg(folder_uuid_parameter())
                .arg(folder_path_parameter())
                .group(folder_identifier_group())
                .arg(format_with_metadata_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV]))
                .arg(
                    Arg::new(PARAMETER_CONCURRENT)
                        .long(PARAMETER_CONCURRENT)
                        .num_args(1)
                        .required(false)
                        .default_value("5")
                        .help("Maximum number of concurrent uploads")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new(PARAMETER_PROGRESS)
                        .long(PARAMETER_PROGRESS)
                        .action(ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during upload"),
                ),
        )
        .subcommand(
            Command::new(COMMAND_DELETE)
                .about("Delete an asset")
                .visible_alias("rm")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .group(asset_identifier_group()),
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all assets in a folder")
                .visible_alias("ls")
                .arg(tenant_parameter())
                .arg(folder_path_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV])),
        )
        .subcommand(metadata_command()) // Add the metadata subcommands
        .subcommand(
            Command::new(COMMAND_DEPENDENCIES)
                .about("Get dependencies for an asset")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV, FORMAT_TREE]))
                .arg(recursive_parameter())
                .group(asset_identifier_group()),
        )
        .subcommand(
            Command::new(COMMAND_DOWNLOAD)
                .about("Download asset file")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(
                    Arg::new(PARAMETER_FILE)
                        .num_args(1)
                        .required(false)
                        .help("Output file path (default: asset filename in current directory)")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
                .group(asset_identifier_group()),
        )
        .subcommand(
            Command::new(COMMAND_DOWNLOAD_FOLDER)
                .about("Download all assets in a folder as a ZIP archive")
                .arg(tenant_parameter())
                .arg(
                    Arg::new(PARAMETER_FOLDER_PATH)
                        .num_args(1..) // Accept one or more values
                        .required(true)
                        .help("Folder path(s) to download (can be provided multiple times or as comma-separated values)")
                        .action(clap::ArgAction::Append), // Allow multiple --path flags
                )
                .arg(
                    Arg::new(PARAMETER_FILE)
                        .num_args(1)
                        .required(false)
                        .help("Output file path (default: <folder_name>.zip in the current directory)")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
        )
    .subcommand(
        Command::new(COMMAND_MATCH)
            .about("Find geometrically similar assets")
            .arg(tenant_parameter())
            .arg(uuid_parameter())
            .arg(path_parameter())
            .arg(
                Arg::new("threshold")
                    .short('s')
                    .long("threshold")
                    .num_args(1)
                    .required(false)
                    .default_value("80.0")
                    .help("Similarity threshold (0.00 to 100.00)")
                    .value_parser(clap::value_parser!(f64)),
            )
            .arg(format_with_headers_parameter())
            .arg(format_with_metadata_parameter())
            .arg(format_pretty_parameter())
            .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV]))
            .group(
                ArgGroup::new("reference_asset")
                    .args(["uuid", "path"])
                    .multiple(false)
                    .required(true)
            ),
    )
    .subcommand(
        Command::new("part-match")
            .about("Find geometrically similar assets using part search algorithm")
            .arg(tenant_parameter())
            .arg(uuid_parameter())
            .arg(path_parameter())
            .arg(
                Arg::new("threshold")
                    .short('s')
                    .long("threshold")
                    .num_args(1)
                    .required(false)
                    .default_value("80.0")
                    .help("Similarity threshold (0.00 to 100.00)")
                    .value_parser(clap::value_parser!(f64)),
            )
            .arg(format_with_headers_parameter())
            .arg(format_with_metadata_parameter())
            .arg(format_pretty_parameter())
            .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV]))
            .group(
                ArgGroup::new("reference_asset")
                    .args(["uuid", "path"])
                    .multiple(false)
                    .required(true)
            ),
    )
    .subcommand(
        Command::new(COMMAND_MATCH_FOLDER)
            .about("Find geometrically similar assets for all assets in one or more folders")
            .arg(tenant_parameter())
            .arg(
                Arg::new(PARAMETER_FOLDER_PATH)
                    .short('p')
                    .long(PARAMETER_FOLDER_PATH)
                    .num_args(1..) // Accept one or more values
                    .required(true)
                    .help("Folder path(s) to process (can be provided multiple times or as comma-separated values)")
                    .action(clap::ArgAction::Append), // Allow multiple --path flags
            )
            .arg(
                Arg::new("threshold")
                    .short('s')
                    .long("threshold")
                    .num_args(1)
                    .required(false)
                    .default_value("80.0")
                    .help("Similarity threshold (0.00 to 100.00)")
                    .value_parser(clap::value_parser!(f64)),
            )
            .arg(
                Arg::new("exclusive")
                    .long("exclusive")
                    .action(ArgAction::SetTrue)
                    .required(false)
                    .help("Only show matches where both assets belong to the specified paths"),
            )
            .arg(format_with_headers_parameter())
            .arg(format_with_metadata_parameter())
            .arg(format_pretty_parameter())
            .arg(format_parameter().default_value(FORMAT_JSON).value_parser([FORMAT_JSON, FORMAT_CSV]))
            .arg(
                Arg::new("concurrent")
                    .long("concurrent")
                    .num_args(1)
                    .required(false)
                    .default_value("1")
                    .help("Maximum number of concurrent operations (range: 1-10)")
                    .value_parser(clap::value_parser!(usize)),
            )
            .arg(
                Arg::new("progress")
                    .long("progress")
                    .action(ArgAction::SetTrue)
                    .required(false)
                    .help("Display progress bar during processing"),
            )
    )
}

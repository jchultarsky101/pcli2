//! Asset command definitions.
//!
//! This module defines CLI commands related to asset management, including
//! upload, download, geometric matching, and metadata operations.

use crate::commands::metadata::metadata_command;
use crate::commands::params::{
    asset_identifier_group, asset_identifier_multiple_group, file_parameter,
    folder_identifier_group, folder_path_parameter, folder_uuid_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    multiple_files_parameter, path_parameter, tenant_parameter, uuid_parameter, COMMAND_ASSET,
    COMMAND_CREATE, COMMAND_CREATE_BATCH, COMMAND_DELETE, COMMAND_DEPENDENCIES, COMMAND_DOWNLOAD,
    COMMAND_GET, COMMAND_LIST, COMMAND_MATCH, COMMAND_PART_MATCH, COMMAND_REPROCESS,
    COMMAND_TEXT_MATCH, COMMAND_THUMBNAIL, COMMAND_VISUAL_MATCH, FORMAT_CSV, FORMAT_JSON,
    FORMAT_TREE, PARAMETER_CONCURRENT, PARAMETER_FILE, PARAMETER_FUZZY, PARAMETER_PROGRESS,
};
use clap::{Arg, ArgAction, Command};

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
                .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV]))
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
                .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV])),
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
                .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV]))
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
                .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV])),
        )
        .subcommand(metadata_command()) // Add the metadata subcommands
        .subcommand(
            Command::new(COMMAND_DEPENDENCIES)
                .about("Get dependencies for an asset")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(format_with_metadata_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV, FORMAT_TREE]))
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
        Command::new(COMMAND_MATCH)
            .visible_alias("geometric-search") // Add alias for geometric-search
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
            .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV]))
            .group(asset_identifier_group()),
    )
    .subcommand(
        Command::new(COMMAND_PART_MATCH)
            .visible_alias("part-search") // Add alias for part-search
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
            .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV]))
            .group(asset_identifier_group()),
    )
    .subcommand(
        Command::new(COMMAND_VISUAL_MATCH)
            .visible_alias("visual-search") // Add alias for visual-search
            .about("Find visually similar assets for a specific reference asset")
            .arg(tenant_parameter())
            .arg(uuid_parameter())
            .arg(path_parameter())
            .arg(format_with_headers_parameter())
            .arg(format_with_metadata_parameter())
            .arg(format_pretty_parameter())
            .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV]))
            .group(asset_identifier_group())
    )
    .subcommand(
        Command::new(COMMAND_TEXT_MATCH)
            .visible_alias("text-search") // Add alias for text-search
            .about("Find assets using text search")
            .arg(tenant_parameter())
            .arg(
                Arg::new("text")
                    .short('q')  // Changed from 't' to 'q' to avoid conflict with tenant parameter
                    .long("text")
                    .num_args(1)
                    .required(true)
                    .help("Text query to search for in assets")
                    .value_parser(clap::value_parser!(String)),
            )
            .arg(
                Arg::new(PARAMETER_FUZZY)
                    .long(PARAMETER_FUZZY)
                    .action(clap::ArgAction::SetTrue)
                    .help("Perform fuzzy search instead of exact search (default: false, which means exact search with quoted text)"),
            )
            .arg(format_with_headers_parameter())
            .arg(format_with_metadata_parameter())  // Add metadata flag to be consistent with other match commands
            .arg(format_pretty_parameter())
            .arg(format_parameter().value_parser([FORMAT_JSON, FORMAT_CSV])) // Only support JSON and CSV as requested
    )
    .subcommand(
        Command::new(COMMAND_REPROCESS)
            .about("Reprocess an asset to refresh its analysis")
            .arg(tenant_parameter())
            .arg(uuid_parameter())
            .arg(path_parameter())
            .group(asset_identifier_group()), // Use the standard asset identifier group to ensure either UUID or path is provided, but not both
    )
    .subcommand(
        Command::new(COMMAND_THUMBNAIL)
            .about("Download asset thumbnail")
            .arg(tenant_parameter())
            .arg(uuid_parameter())
            .arg(path_parameter())
            .arg(
                clap::Arg::new(PARAMETER_FILE)
                    .num_args(1)
                    .required(false)
                    .help("Output file path (default: asset name with .png extension in current directory)")
                    .value_parser(clap::value_parser!(std::path::PathBuf)),
            )
            .group(asset_identifier_group()),
    )
}

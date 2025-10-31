//! Asset command definitions.
//!
//! This module defines CLI commands related to asset management, including 
//! upload, download, geometric matching, and metadata operations.

use crate::commands::metadata::metadata_command;
use crate::commands::params::{
    format_parameter, name_parameter, path_parameter, refresh_parameter,
    tenant_parameter, uuid_parameter, 
    COMMAND_CREATE, COMMAND_CREATE_BATCH, COMMAND_DELETE, COMMAND_GET, 
    COMMAND_LIST, COMMAND_UPDATE
};
use clap::{Arg, ArgAction, Command, ArgGroup};

/// Create the asset command with all its subcommands.
pub fn asset_command() -> Command {
    Command::new("asset")
        .about("Manage assets")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_CREATE)
                .about("Create a new asset by uploading a file")
                .arg(tenant_parameter())
                .arg(
                    Arg::new("file")
                        .long("file")
                        .num_args(1)
                        .required(true)
                        .help("Path to the file to upload")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
                .arg(path_parameter().required(true))
                .arg(format_parameter().value_parser(["json", "csv"])),
        )
        .subcommand(
            Command::new(COMMAND_CREATE_BATCH)
                .about("Create multiple assets by uploading files matching a glob pattern")
                .arg(tenant_parameter())
                .arg(
                    Arg::new("files")
                        .long("files")
                        .num_args(1)
                        .required(true)
                        .help("Glob pattern to match files to upload (e.g., \"data/puzzle/*.STL\")")
                        .value_parser(clap::value_parser!(String)),
                )
                .arg(path_parameter().required(true)) // Make path required for batch operations
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(
                    Arg::new("concurrent")
                        .long("concurrent")
                        .num_args(1)
                        .required(false)
                        .default_value("5")
                        .help("Maximum number of concurrent uploads")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("progress")
                        .long("progress")
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
                .group(
                    ArgGroup::new("asset_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new("geometric-match")
                .about("Find geometrically similar assets")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(
                    Arg::new("threshold")
                        .long("threshold")
                        .num_args(1)
                        .required(false)
                        .default_value("80.0")
                        .help("Similarity threshold (0.00 to 100.00)")
                        .value_parser(clap::value_parser!(f64)),
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .group(
                    ArgGroup::new("reference_asset")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new("geometric-match-folder")
                .about("Find geometrically similar assets for all assets in a folder")
                .arg(tenant_parameter())
                .arg(path_parameter().required(true))
                .arg(
                    Arg::new("threshold")
                        .long("threshold")
                        .num_args(1)
                        .required(false)
                        .default_value("80.0")
                        .help("Similarity threshold (0.00 to 100.00)")
                        .value_parser(clap::value_parser!(f64)),
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(
                    Arg::new("concurrent")
                        .long("concurrent")
                        .num_args(1)
                        .required(false)
                        .default_value("5")
                        .help("Maximum number of concurrent operations")
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
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get asset details")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(format_parameter().value_parser(["json", "csv"]))
                .group(
                    ArgGroup::new("asset_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all assets in a folder")
                .visible_alias("ls")
                .arg(tenant_parameter())
                .arg(path_parameter())
                .arg(refresh_parameter())
                .arg(
                    Arg::new("metadata")
                        .short('m')
                        .long("metadata")
                        .action(ArgAction::SetTrue)
                        .required(false)
                        .help("Include metadata fields in the output (adds metadata columns for CSV, metadata object for JSON)"),
                )
                .arg(format_parameter().value_parser(["json", "csv"])),
        )
        .subcommand(metadata_command()) // Add the metadata subcommands
        .subcommand(
            Command::new("dependencies")
                .about("Get dependencies for an asset")
                .visible_alias("dep")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(format_parameter().value_parser(["json", "csv", "tree"]))
                .arg(crate::commands::params::recursive_parameter())
                .group(
                    ArgGroup::new("asset_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new(COMMAND_UPDATE)
                .about("Update an asset's metadata")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(name_parameter())
                .arg(format_parameter().value_parser(["json", "csv"]))
                .group(
                    ArgGroup::new("asset_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
}
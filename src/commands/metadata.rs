//! Metadata command definitions.
//!
//! This module defines CLI commands related to asset metadata management.

use crate::commands::params::{
    format_parameter, format_pretty_parameter, format_with_headers_parameter, path_parameter, tenant_parameter, uuid_parameter, COMMAND_CREATE,
    COMMAND_DELETE, COMMAND_GET, COMMAND_METADATA, PARAMETER_REFRESH,
};
use clap::{Arg, ArgAction, ArgGroup, Command};

/// Create the metadata command with all its subcommands.
pub fn metadata_command() -> Command {
    Command::new(COMMAND_METADATA)
        .about("Manage asset metadata")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get metadata for an asset")
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
            Command::new(COMMAND_CREATE)
                .about("Add metadata to an asset")
                .visible_alias("update")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(
                    Arg::new("metadata_name")
                        .long("name")
                        .num_args(1)
                        .required(true)
                        .value_parser(clap::value_parser!(String))
                        .help("Metadata property name")
                )
                .arg(
                    Arg::new("metadata_value")
                        .long("value")
                        .num_args(1)
                        .required(true)
                        .value_parser(clap::value_parser!(String))
                        .help("Metadata property value")
                )
                .arg(
                    Arg::new("metadata_type")
                        .long("type")
                        .num_args(1)
                        .required(false)
                        .value_parser(["text", "number", "boolean"])
                        .default_value("text")
                        .help("Metadata field type (text, number, boolean) - default: text")
                )
                .arg(
                    Arg::new(PARAMETER_REFRESH)
                        .long(PARAMETER_REFRESH)
                        .required(false)
                        .action(ArgAction::SetTrue)
                        .help("Force refresh metadata field cache from API")
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .group(
                    ArgGroup::new("asset_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new(COMMAND_DELETE)
                .about("Delete specific metadata fields from an asset")
                .visible_alias("rm")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(
                    Arg::new("metadata_name")
                        .short('n')
                        .long("name")
                        .num_args(1)
                        .required(true)
                        .value_parser(clap::value_parser!(String))
                        .help("Metadata property name (can be provided multiple times or as comma-separated list)")
                        .action(ArgAction::Append)
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .group(
                    ArgGroup::new("asset_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new("create-batch")
                .about("Create metadata for multiple assets from a CSV file")
                .visible_alias("update-batch")
                .long_about(
                    "Create metadata for multiple assets from a CSV file.\n\n\
                    The CSV file must have the following columns in the specified order:\n\
                    - ASSET_PATH: The full path of the asset in Physna\n\
                    - NAME: The name of the metadata field to set\n\
                    - VALUE: The value to set for the metadata field\n\n\
                    CSV File Requirements:\n\
                    - The first row must contain the headers ASSET_PATH,NAME,VALUE\n\
                    - The file must be UTF-8 encoded\n\
                    - Values containing commas, quotes, or newlines must be enclosed in double quotes\n\
                    - Empty rows will be ignored\n\
                    - Each row represents a single metadata field assignment for an asset\n\n\
                    If an asset has multiple metadata fields to update, include multiple rows \n\
                    with the same ASSET_PATH but different NAME and VALUE combinations.\n\n\
                    Example CSV format:\n\
                    ASSET_PATH,NAME,VALUE\n\
                    folder/subfolder/asset1.stl,Material,Steel\n\
                    folder/subfolder/asset1.stl,Weight,\"15.5 kg\"\n\
                    folder/subfolder/asset2.ipt,Material,Aluminum\n\n\
                    The command will group metadata by asset path and update all metadata \
                    for each asset in a single API call."
                )
                .arg(tenant_parameter())
                .arg(
                    Arg::new("csv-file")
                        .long("csv-file")
                        .num_args(1)
                        .required(true)
                        .help("Path to the CSV file containing metadata entries")
                        .value_parser(clap::value_parser!(std::path::PathBuf)),
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(
                    Arg::new("progress")
                        .long("progress")
                        .action(ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during processing"),
                ),
        )
        .subcommand(
            Command::new("inference")
                .about("Apply metadata from a reference asset to geometrically similar assets")
                .arg(path_parameter().required(true))
                .arg(
                    Arg::new("inference_name")
                        .long("name")
                        .num_args(1..)
                        .action(ArgAction::Append)
                        .required(true)
                        .help("Metadata field name(s) to copy (can be specified multiple times or comma-separated)")
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
                    Arg::new("recursive")
                        .long("recursive")
                        .action(ArgAction::SetTrue)
                        .required(false)
                        .help("Apply inference recursively to all found similar assets"),
                )
                .arg(
                    Arg::new("exclusive")
                        .long("exclusive")
                        .action(ArgAction::SetTrue)
                        .required(false)
                        .help("Only apply inference to assets in the same parent folder as the reference asset"),
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_with_headers_parameter())
                .arg(format_pretty_parameter())
                .arg(tenant_parameter())
        )
}

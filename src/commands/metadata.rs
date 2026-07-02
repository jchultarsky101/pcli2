//! Metadata command definitions.
//!
//! This module defines CLI commands related to asset metadata management.

use crate::commands::params::{
    continue_on_error_parameter, delete_if_empty_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    path_parameter, tenant_parameter, uuid_parameter, COMMAND_CREATE, COMMAND_DELETE, COMMAND_GET,
    COMMAND_METADATA,
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
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
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
                    Arg::new("name")
                        .long("name")
                        .num_args(1)
                        .required(true)
                        .value_parser(clap::value_parser!(String))
                        .help("Metadata property name")
                )
                .arg(
                    Arg::new("value")
                        .long("value")
                        .num_args(1)
                        .required(true)
                        .value_parser(clap::value_parser!(String))
                        .help("Metadata property value")
                )
                .arg(
                    Arg::new("type")
                        .long("type")
                        .num_args(1)
                        .required(false)
                        .value_parser(["text", "number", "boolean"])
                        .default_value("text")
                        .help("Metadata field type (text, number, boolean) - default: text")
                )
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
                    Arg::new("field_name")
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
                    Two CSV layouts are supported. The layout is detected automatically \
                    from the header row, or can be forced with --csv-format.\n\n\
                    CLASSIC (vertical) format — one row per asset+field combination:\n\
                    - ASSET_PATH: The full path of the asset in Physna\n\
                    - NAME: The name of the metadata field to set\n\
                    - VALUE: The value to set for the metadata field\n\n\
                    If an asset has multiple metadata fields to update, include multiple rows \n\
                    with the same ASSET_PATH but different NAME and VALUE combinations.\n\n\
                    Example:\n\
                    ASSET_PATH,NAME,VALUE\n\
                    folder/subfolder/asset1.stl,Material,Steel\n\
                    folder/subfolder/asset1.stl,Weight,\"15.5 kg\"\n\
                    folder/subfolder/asset2.ipt,Material,Aluminum\n\n\
                    UI (horizontal) format — one row per asset, as exported by the Physna \
                    web UI's bulk metadata upload:\n\
                    - path: The full path of the asset in Physna\n\
                    - id: Optional asset UUID; when present it takes precedence over the path\n\
                    - metadata:<field name>: One column per metadata field to set\n\n\
                    Columns other than path, id, and metadata:* are ignored with a warning.\n\n\
                    In both formats, empty values are skipped by default (the existing \
                    metadata field, if any, is left untouched), so the file can be used to \
                    incrementally add or update fields. Pass --delete-if-empty to instead \
                    delete the metadata field from the asset when its value is empty, e.g. \
                    to replace an asset's metadata wholesale.\n\n\
                    Example:\n\
                    path,id,metadata:Material,metadata:Color\n\
                    /folder/part1.sldprt,,Steel,Blue\n\
                    /folder/part2.step,123e4567-e89b-12d3-a456-426614174000,Aluminum,Red\n\n\
                    General CSV requirements:\n\
                    - The first row must contain the column headers\n\
                    - The file must be UTF-8 encoded\n\
                    - Values containing commas, quotes, or newlines must be enclosed in double quotes\n\n\
                    The command groups metadata by asset and updates all metadata \
                    for each asset in a single API call.\n\n\
                    By default, any error (such as an asset that cannot be resolved \
                    or a failed metadata API call) terminates the batch operation. \
                    Pass --continue-on-error to skip assets that cannot be resolved \
                    and continue with the remaining rows. Metadata API errors always \
                    terminate execution regardless of this flag, because the API already \
                    retries transient failures internally."
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
                .arg(
                    Arg::new("csv-format")
                        .long("csv-format")
                        .num_args(1)
                        .required(false)
                        .value_parser(["auto", "classic", "ui"])
                        .default_value("auto")
                        .help(
                            "CSV layout: 'classic' (ASSET_PATH,NAME,VALUE rows), 'ui' (one row \
                            per asset with 'metadata:' columns, as exported by the Physna UI), \
                            or 'auto' to detect from the header row",
                        ),
                )
                .arg(
                    Arg::new("progress")
                        .long("progress")
                        .action(ArgAction::SetTrue)
                        .required(false)
                        .help("Display progress bar during processing"),
                )
                .arg(delete_if_empty_parameter())
                .arg(continue_on_error_parameter()),
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

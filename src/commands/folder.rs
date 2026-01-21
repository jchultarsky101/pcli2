//! Folder command definitions.
//!
//! This module defines CLI commands related to folder management.

use crate::commands::params::{
    folder_identifier_group, folder_path_parameter, folder_uuid_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    name_parameter, parent_folder_identifier_group, parent_folder_path_parameter, parent_folder_uuid_parameter, tenant_parameter,
    COMMAND_CREATE, COMMAND_DELETE, COMMAND_FOLDER, COMMAND_GET,
    COMMAND_LIST, PARAMETER_FILE, PARAMETER_PROGRESS,
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
                    clap::Arg::new(PARAMETER_FILE)
                        .long(PARAMETER_FILE)
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
}

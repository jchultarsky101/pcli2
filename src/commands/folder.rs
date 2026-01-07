//! Folder command definitions.
//!
//! This module defines CLI commands related to folder management.

use crate::commands::params::{
    folder_identifier_group, folder_path_parameter, folder_uuid_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    name_parameter, parent_folder_identifier_group, parent_folder_path_parameter, parent_folder_uuid_parameter, tenant_parameter,
    COMMAND_CREATE, COMMAND_DELETE, COMMAND_FOLDER, COMMAND_GET,
    COMMAND_LIST,
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
}

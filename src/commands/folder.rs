//! Folder command definitions.
//!
//! This module defines CLI commands related to folder management.

use crate::commands::params::{
    format_parameter, name_parameter, parent_folder_id_parameter, 
    path_parameter, refresh_parameter, recursive_parameter, tenant_parameter,
    uuid_parameter, 
    COMMAND_CREATE, COMMAND_DELETE, COMMAND_GET, COMMAND_LIST, COMMAND_FOLDER
};
use clap::{Command, ArgGroup};

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
                .arg(parent_folder_id_parameter())
                .arg(path_parameter())
                .arg(format_parameter())
                .group(
                    ArgGroup::new("parent")
                        .args(["parent-folder-id", "path"])
                        .multiple(false)
                ),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get folder details")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter())
                .arg(format_parameter())
                .group(
                    ArgGroup::new("folder_identifier")
                        .args(["uuid", "path"])
                        .multiple(false)
                        .required(true)
                ),
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all folders")
                .visible_alias("ls")
                .arg(tenant_parameter())
                .arg(format_parameter())
                .arg(path_parameter())
                .arg(refresh_parameter())
                .arg(recursive_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_DELETE)
                .about("Delete a folder")
                .visible_alias("rm")
                .arg(tenant_parameter())
                .arg(uuid_parameter())
                .arg(path_parameter()),
        )
}
//! Configuration command definitions.
//!
//! This module defines CLI commands related to configuration management.

use crate::commands::params::{
    file_parameter, format_parameter, output_file_parameter, COMMAND_CONFIG, COMMAND_EXPORT,
    COMMAND_GET, COMMAND_IMPORT, COMMAND_LIST,
};
use clap::Command;

/// Create the config command with all its subcommands.
pub fn config_command() -> Command {
    Command::new(COMMAND_CONFIG)
        .about("Configuration management")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get configuration details")
                .arg(format_parameter())
                .subcommand(Command::new("path").about("Show configuration file path")),
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List configuration")
                .visible_alias("ls")
                .arg(format_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_EXPORT)
                .about("Export configuration to file")
                .arg(output_file_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_IMPORT)
                .about("Import configuration from file")
                .arg(file_parameter()),
        )
}

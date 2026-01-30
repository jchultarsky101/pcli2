//! Configuration command definitions.
//!
//! This module defines CLI commands related to configuration management.

use crate::commands::params::{
    api_url_parameter, auth_url_parameter, file_parameter, format_parameter,
    format_pretty_parameter, format_with_headers_parameter, output_file_parameter,
    ui_url_parameter, COMMAND_ADD, COMMAND_CONFIG, COMMAND_ENVIRONMENT, COMMAND_ENVIRONMENT_GET,
    COMMAND_ENVIRONMENT_LIST, COMMAND_EXPORT, COMMAND_GET, COMMAND_IMPORT, COMMAND_REMOVE,
    COMMAND_RESET, COMMAND_USE,
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
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter())
                .subcommand(Command::new("path").about("Show configuration file path")),
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
        .subcommand(
            Command::new(COMMAND_ENVIRONMENT)
                .about("Manage environment configurations")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_ADD)
                        .about("Add a new environment configuration")
                        .arg(crate::commands::params::name_parameter().required(true)
                            .help("Name of the environment"))
                        .arg(api_url_parameter())
                        .arg(ui_url_parameter())
                        .arg(auth_url_parameter()),
                )
                .subcommand(
                    Command::new(COMMAND_USE)
                        .about("Switch to an environment")
                        .arg(crate::commands::params::name_parameter().required(false)
                            .help("Name of the environment to switch to")),
                )
                .subcommand(
                    Command::new(COMMAND_REMOVE)
                        .about("Remove an environment")
                        .arg(crate::commands::params::name_parameter().required(true)
                            .help("Name of the environment to remove")),
                )
                .subcommand(
                    Command::new(COMMAND_ENVIRONMENT_LIST)
                        .about("List all environments")
                        .arg(crate::commands::params::format_parameter().value_parser([crate::commands::params::FORMAT_JSON, crate::commands::params::FORMAT_CSV]))
                        .arg(crate::commands::params::format_pretty_parameter())
                        .arg(crate::commands::params::format_with_headers_parameter()),
                )
                .subcommand(
                    Command::new(COMMAND_RESET)
                        .about("Reset all environment configurations to blank state"),
                )
                .subcommand(
                    Command::new(COMMAND_ENVIRONMENT_GET)
                        .about("Get environment details")
                        .arg(crate::commands::params::name_parameter().required(false)
                            .help("Name of the environment to get details for (defaults to active environment)"))
                        .arg(crate::commands::params::format_parameter().value_parser([crate::commands::params::FORMAT_JSON, crate::commands::params::FORMAT_CSV]))
                        .arg(crate::commands::params::format_pretty_parameter())
                        .arg(crate::commands::params::format_with_headers_parameter()),
                ),
        )
}

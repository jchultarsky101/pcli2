//! Context command definitions.
//!
//! This module defines CLI commands related to context management.

use crate::commands::params::{
    format_parameter, id_parameter, name_parameter, 
    COMMAND_CLEAR, COMMAND_CONTEXT, COMMAND_GET, COMMAND_SET
};
use clap::Command;

/// Create the context command with all its subcommands.
pub fn context_command() -> Command {
    Command::new(COMMAND_CONTEXT)
        .about("Context management")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_SET)
                .about("Set context")
                .subcommand(
                    Command::new("tenant")
                        .about("Set active tenant")
                        .arg(name_parameter().required(false))
                        .arg(id_parameter()),
                ),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get current context")
                .arg(format_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_CLEAR)
                .about("Clear context")
                .subcommand(
                    Command::new("tenant").about("Clear active tenant"),
                ),
        )
}
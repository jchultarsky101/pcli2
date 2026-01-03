//! Context command definitions.
//!
//! This module defines CLI commands related to context management.

use crate::commands::params::{
    tenant_id_parameter, tenant_name_parameter, COMMAND_CLEAR, COMMAND_CONTEXT, COMMAND_GET,
    COMMAND_SET, COMMAND_TENANT,
};
use clap::Command;

/// Create the context command with all its subcommands.
pub fn context_command() -> Command {
    Command::new(COMMAND_CONTEXT)
        .about("Context management")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_SET).about("Set context").subcommand(
                Command::new(COMMAND_TENANT)
                    .about("Set or select an active tenant")
                    .arg(tenant_name_parameter().required(false))
                    .arg(tenant_id_parameter()),
            ),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get current context")
                .subcommand_required(false) // Allow no subcommand for context get
                .subcommand(
                    Command::new(COMMAND_TENANT)
                        .about("Print the active tenant")
                        .arg(crate::commands::params::format_with_headers_parameter())
                        .arg(crate::commands::params::format_with_metadata_parameter())
                        .arg(crate::commands::params::format_pretty_parameter())
                        .arg(crate::commands::params::format_parameter().value_parser([crate::commands::params::FORMAT_JSON, crate::commands::params::FORMAT_CSV]))
                )
                .arg(crate::commands::params::format_with_headers_parameter())
                .arg(crate::commands::params::format_with_metadata_parameter())
                .arg(crate::commands::params::format_pretty_parameter())
                .arg(crate::commands::params::format_parameter().value_parser([crate::commands::params::FORMAT_JSON, crate::commands::params::FORMAT_CSV]))
        )
        .subcommand(
            Command::new(COMMAND_CLEAR)
                .about("Clear context")
                .subcommand(Command::new(COMMAND_TENANT).about("Clear the active tenant")),
        )
}

//! Tenant command definitions.
//!
//! This module defines CLI commands related to tenant management.

use crate::commands::params::{
    format_parameter, id_parameter, 
    COMMAND_GET, COMMAND_LIST, COMMAND_TENANT
};
use clap::Command;

/// Create the tenant command with all its subcommands.
pub fn tenant_command() -> Command {
    Command::new(COMMAND_TENANT)
        .about("Manage tenants")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get tenant details")
                .arg(id_parameter())
                .arg(format_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all tenants")
                .visible_alias("ls")
                .arg(format_parameter().value_parser(["json", "csv"])),
        )
}
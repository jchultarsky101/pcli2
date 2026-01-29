//! Tenant command definitions.
//!
//! This module defines CLI commands related to tenant management.

use crate::commands::params::{
    format_parameter, format_pretty_parameter, format_with_headers_parameter,
    tenant_identifier_group, tenant_name_parameter, tenant_uuid_parameter, COMMAND_CLEAR,
    COMMAND_GET, COMMAND_LIST, COMMAND_TENANT, COMMAND_USE,
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
                .arg(tenant_uuid_parameter()) // --tenant-uuid (tenant UUID)
                .arg(tenant_name_parameter()) // --name (tenant short name, using existing PARAMETER_TENANT_NAME)
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter())
                .group(tenant_identifier_group()), // Group to ensure only one of --tenant-uuid or --name is provided
        )
        .subcommand(
            Command::new(COMMAND_LIST)
                .about("List all tenants")
                .visible_alias("ls")
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new("state")
                .about("Get asset state counts for the current tenant")
                .arg(crate::commands::params::tenant_parameter())
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_USE)
                .about("Set the active tenant")
                .arg(tenant_name_parameter()) // --name (tenant short name)
                .arg(crate::commands::params::refresh_parameter()) // --refresh flag to force refresh tenant list
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new("current")
                .about("Get the active tenant")
                .arg(format_parameter().value_parser(["json", "csv", "tree"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(Command::new(COMMAND_CLEAR).about("Clear the active tenant"))
}

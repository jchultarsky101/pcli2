//! Tenant command definitions.
//!
//! This module defines CLI commands related to tenant management.

use crate::commands::params::{
    format_parameter, format_pretty_parameter, format_with_headers_parameter, format_with_metadata_parameter,
    tenant_name_parameter, tenant_uuid_parameter, tenant_identifier_group, COMMAND_GET, COMMAND_LIST, COMMAND_TENANT,
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
                .arg(tenant_uuid_parameter())    // --tenant-uuid (tenant UUID)
                .arg(tenant_name_parameter())    // --name (tenant short name, using existing PARAMETER_TENANT_NAME)
                .arg(format_parameter())
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter())
                .arg(format_with_metadata_parameter())
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
}

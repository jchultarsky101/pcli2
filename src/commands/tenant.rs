//! Tenant command definitions.
//!
//! This module defines CLI commands related to tenant management.

use crate::commands::params::{
    format_parameter, format_pretty_parameter, format_with_headers_parameter,
    tenant_name_parameter, tenant_parameter, COMMAND_CLEAR, COMMAND_GET, COMMAND_LIST,
    COMMAND_METADATA, COMMAND_TENANT, COMMAND_USE,
};
use clap::Command;

/// Create the tenant command with all its subcommands.
pub fn tenant_command() -> Command {
    Command::new(COMMAND_TENANT)
        .about("Manage tenants")
        .subcommand_required(true)
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
                .arg(
                    clap::Arg::new("type")
                        .long("type")
                        .num_args(1)
                        .required(false)
                        .value_parser(["indexing", "finished", "failed", "unsupported", "no-3d-data", "missing-dependencies"])
                        .help("Filter assets by state: indexing, finished, failed, unsupported, no-3d-data, or missing-dependencies"),
                )
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_USE)
                .about("Set the active tenant")
                .visible_alias("select")
                .arg(tenant_name_parameter()) // --name (tenant short name)
                .arg(crate::commands::params::refresh_parameter()) // --refresh flag to force refresh tenant list
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .visible_alias("current")
                .about("Get the active tenant")
                .arg(format_parameter().value_parser(["json", "csv", "tree"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_CLEAR)
                .about("Clear the active tenant")
                .visible_alias("unset"),
        )
        .subcommand(
            Command::new(COMMAND_METADATA)
                .about("Inspect the tenant's metadata-field registry")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_LIST)
                        .about("List all metadata fields registered in the tenant with their types")
                        .visible_alias("ls")
                        .long_about(
                            "List every metadata field registered in the tenant along with its \
                            data type (text, number, boolean, url, ...).\n\n\
                            The CSV output uses the same column headers as the classic \
                            'asset metadata create-batch' input (ASSET_PATH,NAME,VALUE,TYPE), \
                            with the NAME and TYPE columns filled from the registry and the \
                            ASSET_PATH and VALUE columns left empty. This makes it easy to save \
                            the listing and turn it into a batch-upload file: replicate each row \
                            per asset, then fill in ASSET_PATH and VALUE.\n\n\
                            Example:\n\
                            pcli2 tenant metadata list --format csv --headers > fields.csv",
                        )
                        .arg(tenant_parameter())
                        .arg(format_parameter().value_parser(["json", "csv", "tree"]))
                        .arg(format_pretty_parameter())
                        .arg(format_with_headers_parameter()),
                ),
        )
}

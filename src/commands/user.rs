//! User command definitions and argument parsing.
//!
//! This module defines the user-related CLI commands and their arguments using the clap crate.
//! It provides a structured way to define the user command-line interface for the Physna CLI.

use clap::{Arg, ArgMatches, Command};

use crate::commands::params::{
    format_parameter, format_pretty_parameter, format_with_headers_parameter,
};

/// Define the user command and its subcommands
pub fn user_command() -> Command {
    Command::new("user")
        .about("Manage users in the Physna system")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("list")
                .about("List users in the current tenant")
                .alias("ls")
                .arg(format_parameter())
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new("get")
                .about("Get details for a specific user")
                .arg(
                    Arg::new("user_id")
                        .help("The ID of the user to retrieve")
                        .required(true)
                        .num_args(1)
                        .value_parser(clap::value_parser!(String)),
                )
                .arg(format_parameter())
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
}

/// Execute user-related subcommands based on the provided arguments
pub async fn execute_user_command(matches: &ArgMatches) -> Result<(), crate::error::CliError> {
    match matches.subcommand() {
        Some(("list", sub_matches)) => {
            crate::actions::users::list_users(sub_matches).await?;
            Ok(())
        }
        Some(("get", sub_matches)) => {
            crate::actions::users::get_user(sub_matches).await?;
            Ok(())
        }
        _ => Err(crate::error::CliError::UnsupportedSubcommand(
            matches
                .subcommand()
                .map(|(name, _)| name.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        )),
    }
}

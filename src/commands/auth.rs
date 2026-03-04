//! Authentication command definitions.
//!
//! This module defines CLI commands related to authentication and session management.

use crate::commands::params::{
    client_id_parameter, client_secret_parameter, format_parameter, format_pretty_parameter,
    format_with_headers_parameter, COMMAND_AUTH, COMMAND_CLEAR_TOKEN, COMMAND_EXPIRATION,
    COMMAND_GET, COMMAND_LOGIN, COMMAND_LOGOUT,
};
use clap::Command;

/// Create the authentication command with all its subcommands.
pub fn auth_command() -> Command {
    Command::new(COMMAND_AUTH)
        .about("Authentication operations")
        .visible_alias("a")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_LOGIN)
                .about("Login using client credentials")
                .visible_alias("in")
                .arg(client_id_parameter())
                .arg(client_secret_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_LOGOUT)
                .about("Logout and clear session")
                .visible_alias("out"),
        )
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get current access token")
                .visible_alias("token")
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_CLEAR_TOKEN)
                .about("Clear the cached access token")
                .visible_alias("clear"),
        )
        .subcommand(
            Command::new(COMMAND_EXPIRATION)
                .about("Show the expiration time of the current access token")
                .visible_alias("exp"),
        )
}

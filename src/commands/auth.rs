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
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_LOGIN)
                .about("Login using client credentials")
                .arg(client_id_parameter())
                .arg(client_secret_parameter()),
        )
        .subcommand(Command::new(COMMAND_LOGOUT).about("Logout and clear session"))
        .subcommand(
            Command::new(COMMAND_GET)
                .about("Get current access token")
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(Command::new(COMMAND_CLEAR_TOKEN).about("Clear the cached access token"))
        .subcommand(
            Command::new(COMMAND_EXPIRATION)
                .about("Show the expiration time of the current access token"),
        )
}

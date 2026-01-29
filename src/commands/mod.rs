//! CLI command definitions and argument parsing.
//!
//! This module defines all the CLI commands and their arguments using the clap crate.
//! It provides a structured way to define the command-line interface for the Physna CLI.
//! The implementation has been modularized into separate files for better maintainability.

use clap::{ArgMatches, Command};

// Import all submodules
pub mod assets;
pub mod auth;
pub mod completions;
pub mod config;
pub mod folder;
pub mod metadata;
pub mod params;
pub mod tenant;

/// Create and configure all CLI commands and their arguments.
///
/// This function defines the entire command-line interface for the Physna CLI,
/// including all subcommands, arguments, and their relationships by combining
/// the modularized command definitions.
///
/// # Returns
///
/// An `ArgMatches` instance containing the parsed command-line arguments.
pub fn create_cli_commands() -> ArgMatches {
    create_full_command().get_matches()
}

/// Create the full CLI command structure without parsing arguments.
///
/// This function creates the complete command structure for use with completion generation.
///
/// # Returns
///
/// A `Command` instance containing the full CLI structure.
pub fn create_full_command() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        // Add all the modularized command groups
        .subcommand(tenant::tenant_command())
        .subcommand(folder::folder_command())
        .subcommand(auth::auth_command())
        .subcommand(assets::asset_command())
        .subcommand(config::config_command())
        .subcommand(completions::completions_command())
}

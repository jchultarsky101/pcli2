//! CLI command definitions and argument parsing.
//!
//! This module defines all the CLI commands and their arguments using the clap crate.
//! It provides a structured way to define the command-line interface for the Physna CLI.
//! The implementation has been modularized into separate files for better maintainability.

use clap::{Arg, ArgAction, ArgMatches, Command};

// Import all submodules
pub mod params;
pub mod auth;
pub mod tenant;
pub mod folder;
pub mod asset;
pub mod metadata;
pub mod context;
pub mod cache;
pub mod config;

// Re-export constants for backward compatibility
pub use params::{
    COMMAND_TENANT, COMMAND_FOLDER, COMMAND_ASSET, COMMAND_FILE,
    COMMAND_CREATE, COMMAND_CREATE_BATCH, COMMAND_GET, COMMAND_LIST,
    COMMAND_UPDATE, COMMAND_DELETE, COMMAND_MATCH, COMMAND_METADATA,
    COMMAND_AUTH, COMMAND_LOGIN, COMMAND_LOGOUT,
    COMMAND_CACHE, COMMAND_PURGE,
    COMMAND_CONTEXT, COMMAND_SET, COMMAND_CLEAR,
    COMMAND_CONFIG, COMMAND_EXPORT, COMMAND_IMPORT,
    PARAMETER_FORMAT, PARAMETER_OUTPUT, PARAMETER_INPUT,
    PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_ID,
    PARAMETER_UUID, PARAMETER_ASSET_UUID, PARAMETER_NAME,
    PARAMETER_TENANT, PARAMETER_PARENT_FOLDER_ID, PARAMETER_PATH,
    PARAMETER_REFRESH, PARAMETER_FILE, PARAMETER_RECURSIVE,
};

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
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Enable verbose output for debugging"),
        )
        // Add all the modularized command groups
        .subcommand(tenant::tenant_command())
        .subcommand(folder::folder_command())
        .subcommand(auth::auth_command())
        .subcommand(asset::asset_command())
        .subcommand(context::context_command())
        .subcommand(cache::cache_command())
        .subcommand(config::config_command())
        .get_matches()
}
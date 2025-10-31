//! Cache command definitions.
//!
//! This module defines CLI commands related to cache management.

use crate::commands::params::{
    COMMAND_CACHE, COMMAND_PURGE
};
use clap::Command;

/// Create the cache command with all its subcommands.
pub fn cache_command() -> Command {
    Command::new(COMMAND_CACHE)
        .about("Cache management")
        .subcommand_required(true)
        .subcommand(
            Command::new(COMMAND_PURGE)
                .about("Purge all cached data")
                .long_about("Delete all cached data including folder hierarchies and asset lists. \
                            This is useful for clearing stale cache data or preparing for uninstallation."),
        )
}
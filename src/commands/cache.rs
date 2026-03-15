//! Cache command definitions.
//!
//! This module defines CLI commands for cache management.

use crate::commands::params::COMMAND_CACHE;
use clap::{Arg, ArgAction, Command};

/// Create the cache command with all its subcommands.
pub fn cache_command() -> Command {
    Command::new(COMMAND_CACHE)
        .about("Cache management")
        .subcommand_required(true)
        .subcommand(
            Command::new("clear")
                .about("Clear all cached data (folder hierarchy, metadata, tenants)")
                .visible_alias("clean")
                .arg(
                    Arg::new("folder")
                        .long("folder")
                        .action(ArgAction::SetTrue)
                        .help("Clear only folder cache"),
                )
                .arg(
                    Arg::new("metadata")
                        .long("metadata")
                        .action(ArgAction::SetTrue)
                        .help("Clear only metadata cache"),
                )
                .arg(
                    Arg::new("tenant")
                        .long("tenant")
                        .action(ArgAction::SetTrue)
                        .help("Clear only tenant cache"),
                )
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                ),
        )
}

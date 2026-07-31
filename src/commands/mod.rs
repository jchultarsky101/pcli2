//! CLI command definitions and argument parsing.
//!
//! This module defines all the CLI commands and their arguments using the clap crate.
//! It provides a structured way to define the command-line interface for the Physna CLI.
//! The implementation has been modularized into separate files for better maintainability.

use clap::{ArgMatches, Command};

// Import all submodules
pub mod assets;
pub mod auth;
pub mod cache;
pub mod completions;
pub mod config;
pub mod environment;
pub mod folder;
pub mod man;
pub mod metadata;
pub mod params;
pub mod tenant;
pub mod user;

/// Usage examples appended to the top-level help output, with ANSI colors.
const EXAMPLES_COLORED: &str = color_print::cstr!(
    "<bold>Examples:</bold>
  <cyan># Authenticate with your Physna tenant</cyan>
  <green>pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET</green>

  <cyan># List folders in tree format</cyan>
  <green>pcli2 folder list --format tree</green>

  <cyan># Upload an asset to a folder</cyan>
  <green>pcli2 asset create --file model.stl --folder-path /Root/Models/</green>

  <cyan># Find geometrically similar assets</cyan>
  <green>pcli2 asset geometric-match --path /Root/Models/part.stl --threshold 85.0</green>

  <cyan># Download all assets from a folder</cyan>
  <green>pcli2 folder download --folder-path /Root/Models/ --output ./downloads --progress</green>

  <cyan># Use short aliases for common commands</cyan>
  <green>pcli2 folder ls          # List folders</green>
  <green>pcli2 asset ls           # List assets</green>
  <green>pcli2 auth in            # Login</green>
  <green>pcli2 env list           # List environments</green>"
);

/// Usage examples appended to the top-level help output, without ANSI colors.
const EXAMPLES_PLAIN: &str = "Examples:
  # Authenticate with your Physna tenant
  pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

  # List folders in tree format
  pcli2 folder list --format tree

  # Upload an asset to a folder
  pcli2 asset create --file model.stl --folder-path /Root/Models/

  # Find geometrically similar assets
  pcli2 asset geometric-match --path /Root/Models/part.stl --threshold 85.0

  # Download all assets from a folder
  pcli2 folder download --folder-path /Root/Models/ --output ./downloads --progress

  # Use short aliases for common commands
  pcli2 folder ls          # List folders
  pcli2 asset ls           # List assets
  pcli2 auth in            # Login
  pcli2 env list           # List environments";

/// Select the examples text for the top-level help based on terminal capabilities.
fn examples_after_help() -> &'static str {
    if crate::terminal::colors_enabled() {
        EXAMPLES_COLORED
    } else {
        EXAMPLES_PLAIN
    }
}

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
        .color(if crate::terminal::colors_enabled() {
            clap::ColorChoice::Auto
        } else {
            clap::ColorChoice::Never
        })
        // Add global arguments
        .arg(
            clap::Arg::new("no-color")
                .long("no-color")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .env("PCLI2_NO_COLOR")
                .help("Disable color output"),
        )
        .arg(
            clap::Arg::new("yes")
                .long("yes")
                .short('y')
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Automatically answer yes to confirmation prompts"),
        )
        .arg(
            clap::Arg::new("verbose")
                .long("verbose")
                .short('v')
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .conflicts_with("quiet")
                .help("Enable verbose output (debug-level logging)"),
        )
        .arg(
            clap::Arg::new("quiet")
                .long("quiet")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Suppress diagnostic output (error-level logging only)"),
        )
        // Add examples
        .after_help(examples_after_help())
        // Add all the modularized command groups
        .subcommand(tenant::tenant_command())
        .subcommand(folder::folder_command())
        .subcommand(auth::auth_command())
        .subcommand(assets::asset_command())
        .subcommand(config::config_command())
        .subcommand(environment::environment_command())
        .subcommand(user::user_command())
        .subcommand(completions::completions_command())
        .subcommand(man::man_command())
        .subcommand(cache::cache_command())
}

#[cfg(test)]
mod recursive_flag_tests {
    use super::*;

    /// Parse an argument vector exactly as the binary would.
    fn parse(args: &[&str]) -> Result<ArgMatches, clap::Error> {
        create_full_command().try_get_matches_from(args)
    }

    #[test]
    fn recursive_is_recognized_when_it_follows_the_folder_path() {
        // `--folder-path` takes one-or-more values, so the question is whether a flag
        // written immediately after it gets swallowed as another path. Reported from
        // the field as "--recursive is not recognized".
        let matches = parse(&[
            "pcli2",
            "folder",
            "geometric-match",
            "--threshold",
            "100.00",
            "--metadata",
            "--pretty",
            "--concurrent",
            "10",
            "--progress",
            "--folder-path",
            "/Creo Files",
            "--recursive",
        ])
        .expect("the documented invocation must parse");

        let sub = matches
            .subcommand_matches("folder")
            .and_then(|m| m.subcommand_matches("geometric-match"))
            .expect("geometric-match");

        let paths: Vec<&String> = sub
            .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
            .expect("folder-path")
            .collect();
        assert_eq!(
            paths,
            vec!["/Creo Files"],
            "the flag must not become a path"
        );
        assert!(
            sub.get_flag(crate::commands::params::PARAMETER_RECURSIVE),
            "--recursive must be set"
        );
    }

    #[test]
    fn recursive_is_recognized_before_the_folder_path_too() {
        let matches = parse(&[
            "pcli2",
            "folder",
            "geometric-match",
            "--recursive",
            "--folder-path",
            "/Creo Files",
        ])
        .expect("must parse");
        let sub = matches
            .subcommand_matches("folder")
            .and_then(|m| m.subcommand_matches("geometric-match"))
            .unwrap();
        assert!(sub.get_flag(crate::commands::params::PARAMETER_RECURSIVE));
    }

    #[test]
    fn the_short_form_works_too() {
        let matches = parse(&[
            "pcli2",
            "folder",
            "geometric-match",
            "--folder-path",
            "/Creo Files",
            "-R",
        ])
        .expect("must parse");
        let sub = matches
            .subcommand_matches("folder")
            .and_then(|m| m.subcommand_matches("geometric-match"))
            .unwrap();
        assert!(sub.get_flag(crate::commands::params::PARAMETER_RECURSIVE));
    }

    #[test]
    fn part_and_visual_match_accept_it_as_well() {
        for command in ["part-match", "visual-match"] {
            let matches = parse(&[
                "pcli2",
                "folder",
                command,
                "--folder-path",
                "/Creo Files",
                "--recursive",
            ])
            .unwrap_or_else(|e| panic!("{} must accept --recursive: {}", command, e));
            let sub = matches
                .subcommand_matches("folder")
                .and_then(|m| m.subcommand_matches(command))
                .unwrap();
            assert!(
                sub.get_flag(crate::commands::params::PARAMETER_RECURSIVE),
                "{}",
                command
            );
        }
    }
}

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
  <green>pcli2 asset create --input model.stl --folder-path /Root/Models/</green>

  <cyan># Find geometrically similar assets</cyan>
  <green>pcli2 asset geometric-match --path /Root/Models/part.stl --threshold 85.0</green>

  <cyan># Download all assets from a folder</cyan>
  <green>pcli2 folder download --folder-path /Root/Models/ --output ./downloads --progress</green>

  <cyan># Use short aliases for common commands</cyan>
  <green>pcli2 folder ls          # List folders</green>
  <green>pcli2 asset ls           # List assets</green>
  <green>pcli2 auth in            # Login</green>
  <green>pcli2 env list           # List environments</green>

<bold>Environment variables:</bold>
  PCLI2_CONFIG_DIR         Directory holding config.yml and the credentials file
  PCLI2_CACHE_DIR          Directory for cache files
  PCLI2_FORMAT             Default --format when the flag is not given
  PCLI2_HEADERS            Default --headers (1/0, yes/no)
  PCLI2_LOG_LEVEL          error, warn (default), info, debug, trace (RUST_LOG wins when set)
  PCLI2_TIMEOUT            Total request timeout in seconds (default 1800)
  PCLI2_MAX_RETRIES        Retries for transient failures (default 2, 0 disables)
  PCLI2_NO_COLOR, NO_COLOR Disable colored output (PCLI2_NO_COLOR=0/false/no/off keeps it on)
  PCLI2_SAFE_CSV           Guard CSV cells against spreadsheet formula injection
  PCLI2_NO_INPUT           Never prompt; fail with exit 64 instead
  PCLI2_ERROR_FORMAT       text (default) or json for errors on stderr
  PCLI2_NO_UPDATE_CHECK    Disable the new-version hint (CI is respected too)"
);

/// Usage examples appended to the top-level help output, without ANSI colors.
const EXAMPLES_PLAIN: &str = "Examples:
  # Authenticate with your Physna tenant
  pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

  # List folders in tree format
  pcli2 folder list --format tree

  # Upload an asset to a folder
  pcli2 asset create --input model.stl --folder-path /Root/Models/

  # Find geometrically similar assets
  pcli2 asset geometric-match --path /Root/Models/part.stl --threshold 85.0

  # Download all assets from a folder
  pcli2 folder download --folder-path /Root/Models/ --output ./downloads --progress

  # Use short aliases for common commands
  pcli2 folder ls          # List folders
  pcli2 asset ls           # List assets
  pcli2 auth in            # Login
  pcli2 env list           # List environments

Environment variables:
  PCLI2_CONFIG_DIR         Directory holding config.yml and the credentials file
  PCLI2_CACHE_DIR          Directory for cache files
  PCLI2_FORMAT             Default --format when the flag is not given
  PCLI2_HEADERS            Default --headers (1/0, yes/no)
  PCLI2_LOG_LEVEL          error, warn (default), info, debug, trace (RUST_LOG wins when set)
  PCLI2_TIMEOUT            Total request timeout in seconds (default 1800)
  PCLI2_MAX_RETRIES        Retries for transient failures (default 2, 0 disables)
  PCLI2_NO_COLOR, NO_COLOR Disable colored output (PCLI2_NO_COLOR=0/false/no/off keeps it on)
  PCLI2_SAFE_CSV           Guard CSV cells against spreadsheet formula injection
  PCLI2_NO_INPUT           Never prompt; fail with exit 64 instead
  PCLI2_ERROR_FORMAT       text (default) or json for errors on stderr
  PCLI2_NO_UPDATE_CHECK    Disable the new-version hint (CI is respected too)";

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

/// Parse the command line, handing back clap's error instead of exiting.
///
/// [`create_cli_commands`] uses `get_matches`, which prints and exits inside clap. That
/// leaves no opportunity to say anything afterwards - including that the binary being
/// run is several versions old, which is exactly the thing worth knowing when an
/// argument was rejected or `--version` was asked for.
pub fn try_create_cli_commands() -> Result<ArgMatches, clap::Error> {
    create_full_command().try_get_matches()
}

/// Whether an update hint is worth appending to this parse outcome.
///
/// Yes for `--version`, which is what someone checks when they suspect their install,
/// and yes for a rejected argument, where "unexpected argument" on an old build reads
/// as a broken feature rather than a stale binary. No for help output, which is long
/// enough that a trailing line would be lost anyway.
pub fn should_hint_after_parse_error(error: &clap::Error) -> bool {
    !matches!(
        error.kind(),
        clap::error::ErrorKind::DisplayHelp
            | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    )
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
                .value_parser(clap::builder::FalseyValueParser::new())
                .help("Disable color output (PCLI2_NO_COLOR: empty, 0, false, no, off mean enabled; anything else disables)"),
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
                .short('q')
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Suppress diagnostic output (error-level logging only)"),
        )
        .arg(
            clap::Arg::new("stats")
                .long("stats")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Print request statistics (API requests, retries, token renewals, elapsed time) on stderr at exit"),
        )
        .arg(
            clap::Arg::new("safe-csv")
                .long("safe-csv")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .env("PCLI2_SAFE_CSV")
                .value_parser(clap::builder::FalseyValueParser::new())
                .help("Guard CSV output against spreadsheet formula injection: a text cell starting with =, +, -, @, tab or carriage return is prefixed with a single quote (numbers are left alone)"),
        )
        .arg(
            clap::Arg::new("no-input")
                .long("no-input")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .env("PCLI2_NO_INPUT")
                .value_parser(clap::builder::FalseyValueParser::new())
                .help("Never prompt: a command that would need an answer fails with exit 64 instead (pass --yes to confirm, or name the tenant or environment)"),
        )
        .arg(
            clap::Arg::new("error-format")
                .long("error-format")
                .value_name("FORMAT")
                .value_parser(["text", "json"])
                .default_value("text")
                .global(true)
                .env("PCLI2_ERROR_FORMAT")
                .help("How errors are written to stderr: text, or json (one object per line; the last one carries the exit code)"),
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
        .subcommand(
            Command::new("doctor")
                .about("Check the local setup: binary, configuration, credentials, token, tenant, caches, and connectivity")
                .arg(
                    clap::Arg::new("format")
                        .short('f')
                        .long("format")
                        .num_args(1)
                        .value_parser(["text", "json"])
                        .ignore_case(true)
                        .default_value("text")
                        .help("Output format"),
                ),
        )
}

#[cfg(test)]
mod update_hint_gating_tests {
    use super::*;
    use clap::error::ErrorKind;

    fn parse_err(args: &[&str]) -> clap::Error {
        try_create_cli_commands_from(args).expect_err("expected a parse error")
    }

    fn try_create_cli_commands_from(args: &[&str]) -> Result<ArgMatches, clap::Error> {
        create_full_command().try_get_matches_from(args)
    }

    #[test]
    fn a_rejected_argument_gets_the_hint() {
        // The case that cost a support round trip: a user on a build predating the flag
        // sees "unexpected argument" and concludes the feature is broken, with nothing
        // pointing at their own install.
        let e = parse_err(&[
            "pcli2",
            "folder",
            "geometric-match",
            "--folder-path",
            "/x",
            "-recursive",
        ]);
        assert_eq!(e.kind(), ErrorKind::UnknownArgument);
        assert!(should_hint_after_parse_error(&e));
    }

    #[test]
    fn version_gets_the_hint() {
        // The single most valuable place for it: --version is what someone checks when
        // they suspect their install is wrong.
        let e = parse_err(&["pcli2", "--version"]);
        assert_eq!(e.kind(), ErrorKind::DisplayVersion);
        assert!(should_hint_after_parse_error(&e));
    }

    #[test]
    fn help_does_not_get_the_hint() {
        // Help is long; a trailing line would scroll past unread.
        let e = parse_err(&["pcli2", "--help"]);
        assert!(!should_hint_after_parse_error(&e));
    }

    #[test]
    fn a_missing_required_argument_gets_the_hint() {
        let e = parse_err(&["pcli2", "folder", "geometric-match"]);
        assert!(should_hint_after_parse_error(&e));
    }
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

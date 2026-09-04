//! Main entry point for the Physna CLI client.
//!
//! This module contains the main function that serves as the entry point
//! for the CLI application. It handles initialization, configuration loading,
//! command parsing, and error handling.
//!
//! The application follows a layered architecture pattern:
//! - main.rs: Entry point and application initialization
//! - cli.rs: Command execution logic
//! - commands.rs: Command definitions and parsing
//! - physna_v3.rs: API client and communication layer
//! - model.rs: Data models and structures
//! - auth.rs: Authentication handling
//! - configuration.rs: Configuration management

use configuration::ConfigurationError;
use pcli2::error::CliError;
use pcli2::{configuration, error_utils};
use std::env;
use std::process;
use thiserror::Error;
use tracing_subscriber::EnvFilter;

mod banner;
mod cli;
use cli::execute_command;
use pcli2::exit_codes::PcliExitCode;

/// Error types that can occur in the main application
#[derive(Error, Debug)]
#[allow(clippy::large_enum_variant)]
enum MainError {
    /// Error related to configuration loading or management
    #[error(transparent)]
    ConfigurationError(#[from] ConfigurationError),
    /// Error related to CLI command execution
    #[error(transparent)]
    CliError(#[from] CliError),
}

impl MainError {
    /// Get the appropriate exit code for this error
    ///
    /// Returns:
    /// - `PcliExitCode::ConfigError` for configuration errors
    /// - The CLI error's specific exit code for command execution errors
    fn exit_code(&self) -> i32 {
        match self {
            MainError::ConfigurationError(_) => PcliExitCode::ConfigError.code(),
            MainError::CliError(cli_error) => cli_error.exit_code().code(),
        }
    }
}

/// Initialize the logging subsystem with the specified log level
///
/// Supports log levels: error, warn, info, debug, trace
/// Precedence: --quiet/--verbose flags, then RUST_LOG, then PCLI2_LOG_LEVEL,
/// then the default of "warn".
fn init_logging(matches: &clap::ArgMatches) {
    let env_filter = if matches.get_flag("quiet") {
        EnvFilter::new("error")
    } else if matches.get_flag("verbose") {
        EnvFilter::new("debug")
    } else {
        // Check for PCLI2_LOG_LEVEL environment variable first
        let log_level = env::var("PCLI2_LOG_LEVEL")
            .or_else(|_| env::var("RUST_LOG"))
            .unwrap_or_else(|_| "warn".to_string());

        // Parse the log level and create filter
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&log_level))
    };

    // Diagnostics go to stderr so stdout stays clean for command output
    // (pipes, --format json/csv, shell completions). ANSI colors are
    // disabled when stderr is redirected or the user opted out, since
    // tracing-subscriber would otherwise emit escape codes unconditionally.
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .with_ansi(pcli2::terminal::stderr_colors_enabled())
        .init();
}

/// Main entry point for the Physna CLI client application.
///
/// This function performs the following steps:
/// 1. Initializes the logging subsystem using tracing with environment-filtered configuration
/// 2. Loads the application configuration from persistent storage
/// 3. Parses command-line arguments using the pre-defined command structure
/// 4. Routes execution to the appropriate command handler based on user input
/// 5. Handles any errors and exits with appropriate exit codes based on error types
///
/// The function uses structured error handling with the `MainError` enum to provide
/// clear error categorization and appropriate exit codes based on error types.
///
/// # Returns
///
/// * `Ok(())` - If the command executed successfully (exit code 0)
/// * `Err(i32)` - If an error occurred, with the appropriate exit code for the error type
#[tokio::main]
async fn main() {
    // Parse without letting clap exit on our behalf, so a rejected argument or a
    // --version check can still be told that this binary is out of date. A user on a
    // build that predates the flag they are passing sees only "unexpected argument",
    // concludes the feature is broken, and has no reason to suspect their own install.
    let matches = match pcli2::commands::try_create_cli_commands() {
        Ok(matches) => matches,
        Err(e) => {
            // The banner goes above help output only. It used to be printed whenever
            // any argument equalled "help", which put ASCII art on stdout ahead of the
            // JSON of `env list --name help` or `asset text-match --text help`.
            if e.kind() == clap::error::ErrorKind::DisplayHelp {
                banner::print_banner();
            }
            let _ = e.print();
            if pcli2::commands::should_hint_after_parse_error(&e) {
                pcli2::update_check::maybe_print_update_hint().await;
            }
            // clap exits 2 for usage errors and 0 for help/version. The documented
            // contract (and sysexits) says usage errors are 64.
            process::exit(if e.use_stderr() {
                PcliExitCode::UsageError.code()
            } else {
                0
            });
        }
    };

    // Initialize the logging subsystem
    // Log level can be set via --verbose/--quiet flags or the
    // PCLI2_LOG_LEVEL / RUST_LOG environment variables (default "warn")
    init_logging(&matches);

    // Commands whose stdout is consumed by other tools (shell init,
    // man page generation) must not trigger the update hint
    let machine_output_command = matches!(matches.subcommand_name(), Some("completions" | "man"));

    // Execute the CLI command
    match execute_command(matches).await {
        Ok(()) => {
            // Check for a newer release (cached, terminal sessions only)
            if !machine_output_command {
                pcli2::update_check::maybe_print_update_hint().await;
            }
            // Success - exit with code 0
            process::exit(0);
        }
        Err(e) => {
            if !e.is_already_reported() {
                error_utils::report_cli_error(&e);
            }

            // Also hint on failure, and after the error so it is the last thing read.
            // A command that just failed is when being several versions behind matters
            // most: a user running a build that predates the flag they are passing sees
            // only "unexpected argument", concludes the feature is broken, and has no
            // reason to suspect their own install. That is not hypothetical - it cost a
            // user a support round trip and cost us a release chasing a bug they never
            // had.
            if !machine_output_command {
                pcli2::update_check::maybe_print_update_hint().await;
            }

            let main_error = MainError::CliError(e);
            process::exit(main_error.exit_code());
        }
    }
}

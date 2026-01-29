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
mod exit_codes;
use exit_codes::PcliExitCode;

/// Error types that can occur in the main application
#[derive(Error, Debug)]
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
    // Check if help is requested to show banner
    let args: Vec<String> = env::args().collect();
    if banner::has_help_flag(&args) {
        banner::print_banner();
    }

    // Initialize the logging subsystem
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Parse and execute the CLI command
    match execute_command().await {
        Ok(()) => {
            // Success - exit with code 0
            process::exit(0);
        }
        Err(e) => {
            error_utils::report_detailed_error(&e, None); // Remove generic context
            let main_error = MainError::CliError(e);
            process::exit(main_error.exit_code());
        }
    }
}

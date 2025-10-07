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

use configuration::{Configuration, ConfigurationError};
use pcli2::{
    configuration,
};
use std::env;
use thiserror::Error;
use tracing_subscriber::EnvFilter;

mod banner;
mod cli;
use cli::{execute_command, CliError};
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
async fn main() -> Result<(), i32> {
    // Check if help is requested to show banner
    let args: Vec<String> = env::args().collect();
    if banner::has_help_flag(&args) {
        banner::print_banner();
    }

    // Initialize the logging subsystem
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Get the configuration
    let configuration = match Configuration::load_default() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("ERROR: Configuration error: {}", e);
            return Err(PcliExitCode::ConfigError.code());
        }
    };

    // Create an API client (placeholder for now)
    let api = (); // We're using Physna V3 API directly in CLI commands

    // Parse and execute the CLI command
    match execute_command(configuration, api).await {
        Ok(()) => {
            // Success - exit with code 0
            Ok(())
        },
        Err(e) => {
            eprintln!("ERROR: {}", e);
            let main_error = MainError::CliError(e);
            Err(main_error.exit_code())
        }
    }
}
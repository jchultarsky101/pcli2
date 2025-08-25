use configuration::{Configuration, ConfigurationError};
use pcli2::{
    configuration,
};
use thiserror::Error;
use tracing_subscriber::EnvFilter;

mod cli;
use cli::{execute_command, CliError};
mod exit_codes;
use exit_codes::PcliExitCode;

#[derive(Error, Debug)]
enum MainError {
    #[error(transparent)]
    ConfigurationError(#[from] ConfigurationError),
    #[error(transparent)]
    CliError(#[from] CliError),
}

impl MainError {
    /// Get the appropriate exit code for this error
    fn exit_code(&self) -> i32 {
        match self {
            MainError::ConfigurationError(_) => PcliExitCode::ConfigError.code(),
            MainError::CliError(cli_error) => cli_error.exit_code().code(),
        }
    }
}

/// Main entry point for the program
#[tokio::main]
async fn main() -> Result<(), i32> {
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
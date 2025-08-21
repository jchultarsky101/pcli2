use configuration::{Configuration, ConfigurationError};
use pcli2::{
    configuration,
};
use thiserror::Error;
use tracing_subscriber::EnvFilter;

mod cli;
use cli::{execute_command, CliError};

#[derive(Error, Debug)]
enum PcliError {
    #[error(transparent)]
    ConfigurationError(#[from] ConfigurationError),
    #[error(transparent)]
    CliError(#[from] CliError),
}

/// Main entry point for the program
#[tokio::main]
async fn main() -> Result<(), PcliError> {
    // Intialize the logging subsystem
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Get the configuration
    let configuration = Configuration::load_default()?;

    // Create an API client (placeholder for now)
    let api = (); // We're using Physna V3 API directly in CLI commands

    // Parse and execute the CLI command
    match execute_command(configuration, api).await {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("ERROR: {}", e.to_string());
            ::std::process::exit(exitcode::DATAERR);
        }
    }
}
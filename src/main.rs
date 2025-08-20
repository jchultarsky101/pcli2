use api::Api;
use configuration::{Configuration, ConfigurationError};
use pcli2::{
    api::{self, ApiError},
    commands, configuration, format,
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
    ApiError(#[from] ApiError),
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

    // Create an API client
    let configuration_ref = std::cell::RefCell::new(configuration.clone());
    let api = Api::initialize(&configuration_ref);

    // Parse and execute the CLI command
    match execute_command(configuration, api).await {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("ERROR: {}", e.to_string());
            ::std::process::exit(exitcode::DATAERR);
        }
    }
}

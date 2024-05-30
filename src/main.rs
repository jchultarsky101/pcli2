use api::Api;
use configuration::{Configuration, ConfigurationError};
use pcli2::{
    api::{self, ApiError},
    commands, configuration, format,
};
use std::cell::RefCell;
use thiserror::Error;
use tracing_subscriber::EnvFilter;

mod cli;
use cli::{execute_command, CliError};

#[derive(Error, Debug)]
enum PcliError {
    #[error("configuration error")]
    ConfigurationError { message: String },
    #[error("API error")]
    ApiError(#[from] ApiError),
    #[error("CLI Error")]
    CliError(#[from] CliError),
}

impl From<ConfigurationError> for PcliError {
    fn from(error: ConfigurationError) -> PcliError {
        PcliError::ConfigurationError {
            message: format!("{}", error.to_string()),
        }
    }
}

/// Main entry point for the program
#[tokio::main]
async fn main() -> Result<(), PcliError> {
    // Intialize the logging subsystem
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Get the configuration
    let configuration = RefCell::new(Configuration::load_default().unwrap_or_default());

    // Create an API client
    let api = RefCell::new(Api::initialize(&configuration));

    // Parse and execute the CLI command
    match execute_command(configuration, api).await {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("ERROR[]: {}", e.to_string());
            ::std::process::exit(exitcode::DATAERR);
        }
    }
}

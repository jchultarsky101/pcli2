mod browser;
mod commands;
mod configuration;
mod security;

use commands::{
    create_cli_commands, COMMAND_NAME_ALL, COMMAND_NAME_CONFIG, COMMAND_NAME_EXPORT,
    PARAMETER_NAME_OUTPUT,
};
use configuration::{Configuration, ConfigurationError};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
enum PcliError {
    #[error("configuration error")]
    ConfigurationError { message: String },
}

impl From<ConfigurationError> for PcliError {
    fn from(error: ConfigurationError) -> PcliError {
        PcliError::ConfigurationError {
            message: format!("{}", error.to_string()),
        }
    }
}

fn main() -> Result<(), PcliError> {
    // initialize the log
    let _log_init_result = pretty_env_logger::try_init_timed();
    let configuration = Configuration::load_default().unwrap_or_default();
    let matches = create_cli_commands();

    match matches.subcommand() {
        // Configuration
        Some((COMMAND_NAME_CONFIG, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_NAME_EXPORT, sub_matches)) => {
                let path = sub_matches
                    .get_one::<PathBuf>(PARAMETER_NAME_OUTPUT)
                    .unwrap(); // it is save vefause the argument is mandatory
                configuration.save_to_file(path)?;
            }
            Some((COMMAND_NAME_SHOW, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_NAME_PATH, _)) => {
                    let path = Configuration::get_default_configuration_file_path()?;
                    let path = path.into_os_string().into_string().unwrap();
                    println!("{}", path);
                }
                Some((COMMAND_NAME_ALL, _)) => {
                    let names = Configuration::get_all_valid_property_names();
                    println!("{:?}", names);
                }
                _ => unreachable!("Invalid sub-command for 'config show'"),
            },
            _ => unreachable!("Invalid sub command for 'config'"),
        },
        _ => unreachable!("Invalid command"),
    }

    // exit normally with status code of zero
    Ok(())
}

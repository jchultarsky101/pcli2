use commands::{
    create_cli_commands, COMMAND_CONFIG, COMMAND_EXPORT, COMMAND_PATH, COMMAND_SET, COMMAND_SHOW,
    COMMAND_TENANT, PARAMETER_OUTPUT,
};
use configuration::{Configuration, ConfigurationError};
use std::{
    io::{stdout, Write},
    path::PathBuf,
};
use thiserror::Error;

mod browser;
mod commands;
mod configuration;
mod security;

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

/*
fn exit_with_error(message: &str) {
    eprintln!("ERROR: {}", message);
    ::std::process::exit(exitcode::USAGE);
}
*/

fn main() -> Result<(), PcliError> {
    // initialize the log
    let _log_init_result = pretty_env_logger::try_init_timed();
    let configuration = Configuration::load_default().unwrap_or_default();
    let commands = create_cli_commands();

    match commands.subcommand() {
        // Configuration
        Some((COMMAND_CONFIG, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_SET, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_TENANT, _)) => {
                    println!("config set tenant");
                }
                _ => unreachable!("Invalid subcommand for 'config set"),
            },
            Some((COMMAND_EXPORT, sub_matches)) => {
                let path = sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT).unwrap(); // it is save vefause the argument is mandatory
                configuration.save(path)?;
            }
            Some((COMMAND_SHOW, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_PATH, _)) => {
                    let path = Configuration::get_default_configuration_file_path()?;
                    let path = path.into_os_string().into_string().unwrap();
                    println!("{}", path);
                }
                Some((COMMAND_TENANT, _)) => {
                    println!("config show tenant");
                }
                _ => {
                    let out: Box<dyn Write> = Box::new(stdout());
                    configuration.write(out)?;
                }
            },
            _ => unreachable!("Invalid subcommand for 'config'"),
        },
        _ => unreachable!("Invalid command"),
    }

    // exit normally with status code of zero
    Ok(())
}

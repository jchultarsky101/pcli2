use crate::format::{OutputFormat, OutputFormatter};
use api::Api;
use commands::{
    create_cli_commands, COMMAND_CONFIG, COMMAND_DELETE, COMMAND_EXPORT, COMMAND_FOLDER,
    COMMAND_FOLDERS, COMMAND_GET, COMMAND_LOGIN, COMMAND_PATH, COMMAND_SET, COMMAND_TENANT,
    PARAMETER_API_URL, PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_FOLDER_ID,
    PARAMETER_FORMAT, PARAMETER_ID, PARAMETER_OIDC_URL, PARAMETER_OUTPUT, PARAMETER_TENANT,
    PARAMETER_TENANT_ALIAS,
};
use configuration::{Configuration, ConfigurationError, TenantConfiguration};
use pcli2::api::ApiError;
use pcli2::commands::COMMAND_LOGOFF;
use std::cell::RefCell;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use url::Url;

use pcli2::{api, commands, configuration, format};

#[derive(Error, Debug)]
enum PcliError {
    #[error("configuration error")]
    ConfigurationError { message: String },
    #[error("API error")]
    ApiError(#[from] ApiError),
}

impl From<ConfigurationError> for PcliError {
    fn from(error: ConfigurationError) -> PcliError {
        PcliError::ConfigurationError {
            message: format!("{}", error.to_string()),
        }
    }
}

/// Display errors in a human-readable format before exiting the application with an error code
///
/// # Arguments
///
/// * `message` - A string slice that holds the human-readable error message
/// * `exitCode` - An exit code of type exitcode::ExitCode to be returned by the process
fn exit_with_error(message: &str, code: exitcode::ExitCode) {
    eprintln!("ERROR: {}", message);
    ::std::process::exit(code);
}

/// Main entry point for the program
fn main() -> Result<(), PcliError> {
    // initialize the log
    let _log_init_result = pretty_env_logger::try_init_timed();
    let configuration = RefCell::new(Configuration::load_default().unwrap_or_default());
    let api = Api::new(&configuration);
    let commands = create_cli_commands();

    match commands.subcommand() {
        // Configuration
        Some((COMMAND_CONFIG, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_SET, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_TENANT, sub_matches)) => {
                    let id = sub_matches.get_one::<String>(PARAMETER_ID).unwrap(); // unwraps here are safe, because the arguments is mandatory and it will caught by Clap before this point
                    let alias = sub_matches.get_one::<String>(PARAMETER_TENANT_ALIAS);
                    let api_url = sub_matches.get_one::<Url>(PARAMETER_API_URL).unwrap();
                    let oidc_url = sub_matches.get_one::<Url>(PARAMETER_OIDC_URL).unwrap();
                    let client_id = sub_matches.get_one::<String>(PARAMETER_CLIENT_ID).unwrap();
                    let client_secret = sub_matches
                        .get_one::<String>(PARAMETER_CLIENT_SECRET)
                        .unwrap();

                    let tenant = TenantConfiguration::builder()
                        .tenant_id(id.to_owned())
                        .api_url(api_url.to_owned())
                        .oidc_url(oidc_url.to_owned())
                        .client_id(client_id.to_owned())
                        .client_secret(client_secret.to_owned())
                        .build()?;

                    configuration.borrow_mut().add_tenant(alias, &tenant)?;
                    configuration.borrow().save_to_default()?;
                }
                _ => unreachable!("Invalid subcommand for 'config set"),
            },
            Some((COMMAND_EXPORT, sub_matches)) => {
                let path = sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT).unwrap(); // it is save vefause the argument is mandatory
                configuration.borrow().save(path)?;
            }
            Some((COMMAND_GET, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_PATH, _)) => {
                    let path = Configuration::get_default_configuration_file_path()?;
                    let path = path.into_os_string().into_string().unwrap();
                    println!("{}", path);
                }
                Some((COMMAND_TENANT, sub_matches)) => {
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();

                    let id = sub_matches.get_one::<String>(PARAMETER_ID).unwrap();
                    match configuration.borrow().tenant(id) {
                        Some(tenant) => match tenant.format(format) {
                            Ok(output) => println!("{}", output),
                            Err(e) => exit_with_error(e.to_string().as_str(), exitcode::CONFIG),
                        },
                        None => (),
                    }
                }
                _ => {
                    // print all tenants
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();

                    match configuration.borrow().format(format) {
                        Ok(output) => println!("{}", output),
                        Err(e) => exit_with_error(e.to_string().as_str(), exitcode::CONFIG),
                    }
                }
            },
            Some((COMMAND_DELETE, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_TENANT, sub_matches)) => {
                    let alias = sub_matches.get_one::<String>(PARAMETER_ID).unwrap();
                    configuration.borrow_mut().delete_tenant(alias);
                    match configuration.borrow().save_to_default() {
                        Ok(()) => (),
                        Err(e) => exit_with_error(e.to_string().as_str(), exitcode::IOERR),
                    }
                }
                _ => unreachable!("Invalid subcommand for 'delete'"),
            },
            _ => unreachable!("Invalid subcommand for 'config'"),
        },
        // Folder
        Some((COMMAND_FOLDER, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_GET, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
                let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let format = OutputFormat::from_str(format).unwrap();
                let folder_id = sub_matches.get_one::<u32>(PARAMETER_FOLDER_ID).unwrap();

                let folder = api.get_folder(&tenant, folder_id, true);

                match folder {
                    Ok(folder) => match folder.format(format) {
                        Ok(output) => println!("{}", output),
                        Err(e) => exit_with_error(e.to_string().as_str(), exitcode::DATAERR),
                    },
                    Err(e) => exit_with_error(&e.to_string(), exitcode::DATAERR),
                }
            }
            _ => unreachable!("Invalid subcommand for 'folder'"),
        },
        // Folders
        Some((COMMAND_FOLDERS, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
            let format = OutputFormat::from_str(format).unwrap();
            let folders = api.get_list_of_folders(&tenant, true);

            match folders {
                Ok(folders) => match folders.format(format) {
                    Ok(output) => println!("{}", output),
                    Err(e) => exit_with_error(e.to_string().as_str(), exitcode::CONFIG),
                },
                Err(e) => exit_with_error(&e.to_string(), exitcode::DATAERR),
            }
        }
        // Login
        Some((COMMAND_LOGIN, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            let _ = api.login(tenant)?;
        }
        // Logoff
        Some((COMMAND_LOGOFF, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            api.logoff(tenant)?;
        }
        _ => unreachable!("Invalid command"),
    }

    // exit normally with status code of zero
    Ok(())
}

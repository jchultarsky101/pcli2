use crate::commands::{
    create_cli_commands, COMMAND_CONFIG, COMMAND_DELETE, COMMAND_EXPORT, COMMAND_FOLDER,
    COMMAND_FOLDERS, COMMAND_GET, COMMAND_LOGIN, COMMAND_LOGOFF, COMMAND_PATH, COMMAND_SET,
    COMMAND_TENANT, PARAMETER_API_URL, PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET,
    PARAMETER_FOLDER_ID, PARAMETER_FORMAT, PARAMETER_ID, PARAMETER_OIDC_URL, PARAMETER_OUTPUT,
    PARAMETER_TENANT, PARAMETER_TENANT_ALIAS,
};
use crate::configuration::TenantConfiguration;
use crate::format::{OutputFormat, OutputFormatter};
use clap::ArgMatches;
use pcli2::api::{Api, ApiInitialized};
use pcli2::configuration::Configuration;
use std::cell::RefCell;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Undefined or unsupported subcommand")]
    UnsupportedSubcommand(String),
    #[error("Configuration error")]
    ConfigurationError(#[from] crate::configuration::ConfigurationError),
    #[error("Processing error")]
    ProcessingError(String),
    #[error("Input/Output error")]
    InputOutputError(String),
    #[error("Formatting error")]
    FormattingError(#[from] crate::format::FormattingError),
    #[error("Security error")]
    SecurityError(String),
}

fn extract_subcommand_name(sub_matches: &ArgMatches) -> String {
    let message = match sub_matches.subcommand() {
        Some(m) => m.0,
        None => "unknown",
    };

    message.to_string()
}

pub async fn execute_command(
    configuration: RefCell<Configuration>,
    api: RefCell<Api<ApiInitialized>>,
) -> Result<(), CliError> {
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

                    Ok(())
                }
                None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
                _ => unreachable!(),
            },
            Some((COMMAND_EXPORT, sub_matches)) => {
                let path = sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT).unwrap(); // it is save vefause the argument is mandatory
                configuration.borrow().save(path)?;

                Ok(())
            }
            Some((COMMAND_GET, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_PATH, _)) => {
                    let path = Configuration::get_default_configuration_file_path()?;
                    let path = path.into_os_string().into_string().unwrap();
                    println!("{}", path);

                    Ok(())
                }
                Some((COMMAND_TENANT, sub_matches)) => {
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();

                    let id = sub_matches.get_one::<String>(PARAMETER_ID).unwrap();
                    match configuration.borrow().tenant(id) {
                        Some(tenant) => match tenant.format(format) {
                            Ok(output) => {
                                println!("{}", output);
                                Ok(())
                            }
                            Err(e) => Err(CliError::FormattingError(e)),
                        },
                        None => Ok(()),
                    }
                }
                None => {
                    // print all tenants
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();

                    match configuration.borrow().format(format) {
                        Ok(output) => {
                            println!("{}", output);
                            Ok(())
                        }
                        Err(e) => Err(CliError::FormattingError(e)),
                    }
                }
                _ => unreachable!(),
            },
            Some((COMMAND_DELETE, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_TENANT, sub_matches)) => {
                    let alias = sub_matches.get_one::<String>(PARAMETER_ID).unwrap();
                    configuration.borrow_mut().delete_tenant(alias);
                    match configuration.borrow().save_to_default() {
                        Ok(()) => Ok(()),
                        Err(e) => Err(CliError::InputOutputError(e.to_string())),
                    }
                }
                None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
                _ => unreachable!(),
            },
            None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                sub_matches,
            ))),
            _ => unreachable!(),
        },
        // Folder
        Some((COMMAND_FOLDER, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_GET, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
                let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let format = OutputFormat::from_str(format).unwrap();
                let folder_id = sub_matches.get_one::<u32>(PARAMETER_FOLDER_ID).unwrap();

                let folder = api.borrow().get_folder(&tenant, folder_id, true).await;

                match folder {
                    Ok(folder) => match folder.format(format) {
                        Ok(output) => {
                            println!("{}", output);
                            Ok(())
                        }
                        Err(e) => Err(CliError::ProcessingError(e.to_string())),
                    },
                    Err(e) => Err(CliError::ProcessingError(e.to_string())),
                }
            }
            None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                sub_matches,
            ))),
            _ => unreachable!(),
        },
        // Folders
        Some((COMMAND_FOLDERS, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
            let format = OutputFormat::from_str(format).unwrap();
            let folders = api.borrow().get_list_of_folders(&tenant, true).await;

            match folders {
                Ok(folders) => match folders.format(format) {
                    Ok(output) => {
                        println!("{}", output);
                        Ok(())
                    }
                    Err(e) => Err(CliError::FormattingError(e)),
                },
                Err(e) => Err(CliError::ProcessingError(e.to_string())),
            }
        }
        // Login
        Some((COMMAND_LOGIN, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            match api.borrow().login(tenant).await {
                Ok(_session) => Ok(()),
                Err(_) => Err(CliError::SecurityError(String::from("Failed to login"))),
            }
        }
        // Logoff
        Some((COMMAND_LOGOFF, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            match api.borrow().logoff(tenant) {
                Ok(()) => Ok(()),
                Err(_) => Err(CliError::SecurityError(String::from("Failed to logoff"))),
            }
        }
        None => Err(CliError::UnsupportedSubcommand(String::from("unknown"))),
        _ => unreachable!(),
    }
}

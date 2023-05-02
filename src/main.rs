use crate::format::{OutputFormat, OutputFormatter};
use api::Api;
use commands::{
    create_cli_commands, COMMAND_CONFIG, COMMAND_DELETE, COMMAND_EXPORT, COMMAND_FOLDERS,
    COMMAND_LOGIN, COMMAND_PATH, COMMAND_SET, COMMAND_SHOW, COMMAND_TENANT, PARAMETER_API_URL,
    PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_FORMAT, PARAMETER_ID,
    PARAMETER_OIDC_URL, PARAMETER_OUTPUT, PARAMETER_TENANT, PARAMETER_TENANT_ALIAS,
};
use configuration::{Configuration, ConfigurationError, TenantConfiguration};
use pcli2::api::ApiError;
use pcli2::commands::COMMAND_LOGOFF;
use std::cell::RefCell;
use std::str::FromStr;
use std::{
    io::{stdout, Write},
    path::PathBuf,
};
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

fn exit_with_error(message: &str, code: exitcode::ExitCode) {
    eprintln!("ERROR: {}", message);
    ::std::process::exit(code);
}

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
                    let id = sub_matches.get_one::<String>(PARAMETER_ID).unwrap();
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
            Some((COMMAND_SHOW, sub_matches)) => match sub_matches.subcommand() {
                Some((COMMAND_PATH, _)) => {
                    let path = Configuration::get_default_configuration_file_path()?;
                    let path = path.into_os_string().into_string().unwrap();
                    println!("{}", path);
                }
                Some((COMMAND_TENANT, sub_matches)) => {
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();

                    let id = sub_matches.get_one::<String>(PARAMETER_ID).unwrap();
                    let tenant = configuration.borrow().tenant(id).unwrap();
                    match tenant.format(format) {
                        Ok(output) => println!("{}", output),
                        Err(e) => exit_with_error(e.to_string().as_str(), exitcode::CONFIG),
                    };
                }
                _ => {
                    let out: Box<dyn Write> = Box::new(stdout());
                    configuration.borrow().write(out)?;
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
        // Folders
        Some((COMMAND_FOLDERS, sub_matches)) => {
            let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT).unwrap();
            let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
            let format = OutputFormat::from_str(format).unwrap();
            let folders = api.list_folders(&tenant);

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

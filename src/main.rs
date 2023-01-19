mod commands;
mod configuration;

use commands::create_cli_commands;
use configuration::{Configuration, ConfigurationError, TenantConfiguration};
use std::path::PathBuf;
use thiserror::Error;
use url::Url;

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

    //let configuration = Configuration::load_from_file(configuration_file_path).unwrap_or_default();

    let matches = create_cli_commands();

    match matches.subcommand() {
        // working with configuration
        Some(("config", sub_matches)) => match sub_matches.subcommand() {
            Some(("show-default-location", _)) => {
                let path = Configuration::get_default_configuration_file_path()?;
                let path = path.into_os_string().into_string().unwrap();

                println!("{}", path);
            }
            Some(("init", sub_matches)) => {
                let file = sub_matches.get_one::<String>("file");
                let file = PathBuf::from(file.unwrap());
                let configuration = Configuration::default();

                configuration.save_to_file(file)?
            }
            Some(("import", sub_matches)) => {
                let file = sub_matches.get_one::<String>("file");
                let file = PathBuf::from(file.unwrap());
                let default_file = Configuration::get_default_configuration_file_path()?;
                let configuration = Configuration::load_from_file(file)?;

                configuration.save_to_file(default_file)?
            }
            Some(("export", sub_matches)) => {
                let file = sub_matches.get_one::<String>("file");
                let file = PathBuf::from(file.unwrap());
                let configuration = Configuration::load_default()?;
                configuration.save_to_file(file)?
            }
            Some(("show-names", _)) => {
                let property_names = Configuration::get_all_valid_property_names();
                property_names.iter().for_each(|name| println!("{}", name));
            }
            Some(("get", sub_matches)) => {
                let name = sub_matches.get_one::<String>("name");
                match name {
                    Some(name) => {
                        let name = name.to_string();
                        let configuration = Configuration::load_default()?;
                        println!("{}", configuration.get(name).unwrap_or_default())
                    }
                    None => unreachable!("Invalid option for config subcommand!"),
                }
            }
            Some(("set", sub_matches)) => {
                let name = sub_matches.get_one::<String>("name");
                let value = sub_matches.get_one::<String>("value");
                match name {
                    Some(name) => {
                        let configuration = Configuration::load_default();

                        let value = match value {
                            Some(value) => Some(value.to_owned()),
                            None => None,
                        };

                        match configuration {
                            Ok(mut configuration) => {
                                configuration.set(name.to_string(), value)?;
                                configuration.save_to_default()?;
                            }
                            Err(e) => return Err(PcliError::from(e)),
                        }
                    }
                    None => unreachable!("\"name\" is a mandatory argument"),
                }
            }
            Some(("tenant", sub_matches)) => match sub_matches.subcommand() {
                Some(("add", sub_matches)) => {
                    let alias = sub_matches.get_one::<String>("alias");
                    let id = sub_matches.get_one::<String>("id").unwrap().to_owned();
                    let alias = match alias {
                        Some(alias) => Some(alias.to_owned()),
                        None => Some(id.to_owned()),
                    };
                    let api_url = sub_matches.get_one::<String>("api-url").unwrap();
                    let api_url = Url::parse(api_url.as_str()).unwrap();
                    let oidc_url = sub_matches
                        .get_one::<String>("oidc-url")
                        .unwrap()
                        .to_owned();
                    let oidc_url = Url::parse(oidc_url.as_str()).unwrap();
                    let client_id = sub_matches
                        .get_one::<String>("client-id")
                        .unwrap()
                        .to_owned();
                    let client_secret = sub_matches
                        .get_one::<String>("client-secret")
                        .unwrap()
                        .to_owned();

                    let tenant =
                        TenantConfiguration::new(id, api_url, oidc_url, client_id, client_secret);

                    let mut configuration = Configuration::load_default().unwrap();
                    configuration.add_tenant(alias, tenant).unwrap();
                    configuration.save_to_default().unwrap();
                }
                Some(("delete", sub_matches)) => {
                    let id = sub_matches.get_one::<String>("id").unwrap();

                    let mut configuration = Configuration::load_default().unwrap();
                    configuration.delete_tenant(id);
                    configuration.save_to_default().unwrap();
                }
                Some(("show-all-aliases", _sub_matches)) => {
                    let configuration = Configuration::load_default().unwrap();
                    let aliases = configuration.get_all_tenant_aliases();
                    aliases.iter().for_each(|a| println!("{}", a));
                }
                _ => unreachable!("Invalid sub command for 'tenant'"),
            },
            _ => unreachable!("Invalid sub command for 'config'"),
        },

        // working with folders
        Some(("folder", sub_matches)) => match sub_matches.subcommand() {
            Some(("get", sub_matches)) => {
                let search = sub_matches.get_one::<String>("search");

                match search {
                    Some(search) => {
                        println!("executing \"folder get {search}\"...", search = search)
                    }
                    None => print!("executing \"folder get *\"..."),
                }
            }
            Some(("add", sub_matches)) => {
                let name = sub_matches.get_one::<String>("name").unwrap();

                println!("executing \"folder add {name}\"...", name = name);
            }
            _ => unreachable!("Invalid sub command for 'folder'"),
        },
        _ => unreachable!("Invalid command"),
    }

    // exit normally with status code of zero
    Ok(())
}

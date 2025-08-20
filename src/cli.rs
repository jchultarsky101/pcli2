use crate::commands::{
    create_cli_commands, COMMAND_AUTH, COMMAND_CLEAR, COMMAND_CONFIG, COMMAND_CONTEXT, COMMAND_CREATE, 
    COMMAND_DELETE, COMMAND_EXPORT, COMMAND_FOLDER, COMMAND_GET, COMMAND_IMPORT, COMMAND_LIST, 
    COMMAND_LOGIN, COMMAND_LOGOUT, COMMAND_SET, COMMAND_TENANT, COMMAND_UPDATE, 
    PARAMETER_API_KEY, PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, 
    PARAMETER_FORMAT, PARAMETER_ID, PARAMETER_INPUT, PARAMETER_NAME, 
    PARAMETER_OUTPUT, PARAMETER_TENANT,
};
use crate::format::{OutputFormat, OutputFormatter};
use clap::ArgMatches;
use pcli2::api::{Api, ApiInitialized};
use pcli2::api_key;
use pcli2::auth::AuthClient;
use pcli2::configuration::Configuration;
use pcli2::keyring::Keyring;
use pcli2::physna_v3::PhysnaApiClient;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Undefined or unsupported subcommand")]
    UnsupportedSubcommand(String),
    #[error("Configuration error")]
    ConfigurationError(#[from] crate::configuration::ConfigurationError),
    #[error("Formatting error")]
    FormattingError(#[from] crate::format::FormattingError),
    #[error("Security error")]
    SecurityError(String),
    #[error("Missing required argument: {0}")]
    MissingRequiredArgument(String),
}

fn extract_subcommand_name(sub_matches: &ArgMatches) -> String {
    let message = match sub_matches.subcommand() {
        Some(m) => m.0,
        None => "unknown",
    };

    message.to_string()
}

pub async fn execute_command(
    mut configuration: Configuration,
    _api: Api<ApiInitialized>,
) -> Result<(), CliError> {
    let commands = create_cli_commands();

    match commands.subcommand() {
        // Tenant resource commands
        Some((COMMAND_TENANT, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_CREATE, sub_matches)) => {
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                // Implementation would create tenant
                println!("Would create tenant: {} (but using Physna V3 API approach)", id);
                Ok(())
            }
            Some((COMMAND_GET, sub_matches)) => {
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                let _format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let _format = OutputFormat::from_str(_format).unwrap();

                // For now, we'll just print a message since we're moving to Physna V3 API
                println!("Would get tenant: {} (but using Physna V3 API approach)", id);
                Ok(())
            }
            Some((COMMAND_LIST, sub_matches)) => {
                // Try to get access token and list tenants from Physna V3 API
                let keyring = Keyring::default();
                match keyring.get(&"default".to_string(), "access-token".to_string()) {
                    Ok(Some(token)) => {
                        let client = PhysnaApiClient::new().with_access_token(token);
                        match client.list_tenants().await {
                            Ok(tenants) => {
                                let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                                let format = OutputFormat::from_str(format).unwrap();
                                
                                // Display the tenants
                                println!("Available tenants:");
                                for tenant in tenants {
                                    match format {
                                        OutputFormat::Json => {
                                            println!("  {{\"id\": \"{}\", \"name\": \"{}\"}}", 
                                                tenant.tenant_id, tenant.tenant_display_name);
                                        }
                                        OutputFormat::Csv => {
                                            println!("  {},{}", tenant.tenant_id, tenant.tenant_display_name);
                                        }
                                    }
                                }
                                Ok(())
                            }
                            Err(e) => {
                                eprintln!("Error fetching tenants: {}", e);
                                Ok(())
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'");
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error retrieving access token: {}", e);
                        Ok(())
                    }
                }
            }
            Some((COMMAND_UPDATE, sub_matches)) => {
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                // Implementation would update existing tenant
                println!("Updating tenant: {}", id);
                Ok(())
            }
            Some((COMMAND_DELETE, sub_matches)) => {
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                // For now, we'll just print a message since we're moving to Physna V3 API
                println!("Would delete tenant: {} (but using Physna V3 API approach)", id);
                Ok(())
            }
            None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                sub_matches,
            ))),
            _ => unreachable!(),
        },
        // Folder resource commands
        Some((COMMAND_FOLDER, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_CREATE, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()))?;
                let name = sub_matches.get_one::<String>(PARAMETER_NAME)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_NAME.to_string()))?;
                // Implementation would create folder
                println!("Creating folder '{}' for tenant '{}'", name, tenant);
                Ok(())
            }
            Some((COMMAND_GET, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()))?;
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let format = OutputFormat::from_str(format).unwrap();
                // Implementation would get folder
                println!("Getting folder '{}' for tenant '{}' in format {:?}", id, tenant, format);
                Ok(())
            }
            Some((COMMAND_LIST, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()))?;
                let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let format = OutputFormat::from_str(format).unwrap();
                
                // Try to get access token and list folders from Physna V3 API
                let keyring = Keyring::default();
                match keyring.get(&"default".to_string(), "access-token".to_string()) {
                    Ok(Some(token)) => {
                        let client = PhysnaApiClient::new().with_access_token(token);
                        match client.list_folders(tenant).await {
                            Ok(folder_list_response) => {
                                let folder_list = folder_list_response.to_folder_list();
                                match folder_list.format(format) {
                                    Ok(output) => {
                                        println!("{}", output);
                                        Ok(())
                                    }
                                    Err(e) => Err(CliError::FormattingError(e)),
                                }
                            }
                            Err(e) => {
                                eprintln!("Error fetching folders: {}", e);
                                Ok(())
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'");
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error retrieving access token: {}", e);
                        Ok(())
                    }
                }
            }
            Some((COMMAND_UPDATE, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()))?;
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                // Implementation would update folder
                println!("Updating folder '{}' for tenant '{}'", id, tenant);
                Ok(())
            }
            Some((COMMAND_DELETE, sub_matches)) => {
                let tenant = sub_matches.get_one::<String>(PARAMETER_TENANT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()))?;
                let id = sub_matches.get_one::<String>(PARAMETER_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_ID.to_string()))?;
                // Implementation would delete folder
                println!("Deleting folder '{}' for tenant '{}'", id, tenant);
                Ok(())
            }
            None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                sub_matches,
            ))),
            _ => unreachable!(),
        },
        // Authentication commands
        Some((COMMAND_AUTH, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_SET, sub_matches)) => match sub_matches.subcommand() {
                Some(("api-key", sub_matches)) => {
                    let api_key = sub_matches.get_one::<String>(PARAMETER_API_KEY)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_API_KEY.to_string()))?;
                    
                    match api_key::store_api_key(api_key) {
                        Ok(()) => {
                            println!("API key stored successfully");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error storing API key: {}", e);
                            Err(CliError::SecurityError(String::from("Failed to store API key")))
                        }
                    }
                }
                None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
                _ => unreachable!(),
            },
            Some((COMMAND_GET, sub_matches)) => match sub_matches.subcommand() {
                Some(("api-key", _)) => {
                    match api_key::get_api_key() {
                        Ok(_) => {
                            println!("API key is configured");
                            Ok(())
                        }
                        Err(api_key::ApiKeyError::ApiKeyNotFound) => {
                            println!("API key is not configured");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error retrieving API key: {}", e);
                            Err(CliError::SecurityError(String::from("Failed to retrieve API key")))
                        }
                    }
                }
                None => {
                    match api_key::get_api_key() {
                        Ok(_) => {
                            println!("API key is configured");
                            Ok(())
                        }
                        Err(api_key::ApiKeyError::ApiKeyNotFound) => {
                            println!("API key is not configured");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error retrieving API key: {}", e);
                            Err(CliError::SecurityError(String::from("Failed to retrieve API key")))
                        }
                    }
                }
                _ => unreachable!(),
            },
            Some((COMMAND_DELETE, sub_matches)) => match sub_matches.subcommand() {
                Some(("api-key", _)) => {
                    match api_key::delete_api_key() {
                        Ok(()) => {
                            println!("API key deleted successfully");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error deleting API key: {}", e);
                            Err(CliError::SecurityError(String::from("Failed to delete API key")))
                        }
                    }
                }
                None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
                _ => unreachable!(),
            },
            Some((COMMAND_LOGIN, sub_matches)) => {
                let client_id = sub_matches.get_one::<String>(PARAMETER_CLIENT_ID)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_CLIENT_ID.to_string()))?;
                let client_secret = sub_matches.get_one::<String>(PARAMETER_CLIENT_SECRET)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_CLIENT_SECRET.to_string()))?;
                
                let auth_client = AuthClient::new(client_id.clone(), client_secret.clone());
                
                match auth_client.get_access_token().await {
                    Ok(token) => {
                        // Store the token in the keyring
                        let keyring = Keyring::default();
                        match keyring.put(&"default".to_string(), "access-token".to_string(), token) {
                            Ok(()) => {
                                println!("Login successful");
                                Ok(())
                            }
                            Err(e) => {
                                eprintln!("Error storing access token: {}", e);
                                Err(CliError::SecurityError(String::from("Failed to store access token")))
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Login failed: {}", e);
                        Err(CliError::SecurityError(String::from("Login failed")))
                    }
                }
            }
            Some((COMMAND_LOGOUT, _)) => {
                let keyring = Keyring::default();
                match keyring.delete(&"default".to_string(), "access-token".to_string()) {
                    Ok(()) => {
                        println!("Logout successful");
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error deleting access token: {}", e);
                        Err(CliError::SecurityError(String::from("Failed to delete access token")))
                    }
                }
            }
            None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                sub_matches,
            ))),
            _ => unreachable!(),
        },
        // Context commands
        Some((COMMAND_CONTEXT, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_SET, sub_matches)) => match sub_matches.subcommand() {
                Some(("tenant", sub_matches)) => {
                    let name = sub_matches.get_one::<String>(PARAMETER_NAME)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_NAME.to_string()))?;
                    
                    // Try to get access token and fetch tenant info from Physna V3 API
                    let keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let client = PhysnaApiClient::new().with_access_token(token);
                            match client.list_tenants().await {
                                Ok(tenants) => {
                                    // Find tenant by name
                                    if let Some(tenant) = tenants.iter().find(|t| 
                                        t.tenant_display_name == *name || t.tenant_short_name == *name) {
                                        // Set the active tenant in configuration
                                        configuration.set_active_tenant(
                                            tenant.tenant_id.clone(), 
                                            tenant.tenant_display_name.clone()
                                        );
                                        
                                        // Save configuration
                                        match configuration.save_to_default() {
                                            Ok(()) => {
                                                println!("Active tenant set to: {} ({})", 
                                                    tenant.tenant_display_name, tenant.tenant_id);
                                                Ok(())
                                            }
                                            Err(e) => {
                                                eprintln!("Error saving configuration: {}", e);
                                                Err(CliError::ConfigurationError(e))
                                            }
                                        }
                                    } else {
                                        eprintln!("Tenant '{}' not found", name);
                                        Ok(())
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error fetching tenants: {}", e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            eprintln!("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error retrieving access token: {}", e);
                            Ok(())
                        }
                    }
                }
                None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
                _ => unreachable!(),
            },
            Some((COMMAND_GET, sub_matches)) => {
                let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let format = OutputFormat::from_str(format).unwrap();
                
                if let Some(tenant_id) = configuration.get_active_tenant_id() {
                    if let Some(tenant_name) = configuration.get_active_tenant_name() {
                        match format {
                            OutputFormat::Json => {
                                println!("{{\"active_tenant\": {{\"id\": \"{}\", \"name\": \"{}\"}}}}", 
                                    tenant_id, tenant_name);
                            }
                            OutputFormat::Csv => {
                                println!("ACTIVE_TENANT_ID,ACTIVE_TENANT_NAME\n{},{}", 
                                    tenant_id, tenant_name);
                            }
                        }
                    } else {
                        println!("Active tenant ID: {}", tenant_id);
                    }
                } else {
                    println!("No active tenant selected");
                }
                Ok(())
            }
            Some((COMMAND_CLEAR, sub_matches)) => match sub_matches.subcommand() {
                Some(("tenant", _)) => {
                    configuration.clear_active_tenant();
                    match configuration.save_to_default() {
                        Ok(()) => {
                            println!("Active tenant cleared");
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error saving configuration: {}", e);
                            Err(CliError::ConfigurationError(e))
                        }
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
        // Configuration commands
        Some((COMMAND_CONFIG, sub_matches)) => match sub_matches.subcommand() {
            Some((COMMAND_GET, sub_matches)) => match sub_matches.subcommand() {
                Some(("path", _)) => {
                    let path = Configuration::get_default_configuration_file_path()?;
                    let path = path.into_os_string().into_string()
                        .map_err(|_| CliError::ConfigurationError(
                            crate::configuration::ConfigurationError::FailedToFindConfigurationDirectory))?;
                    println!("{}", path);
                    Ok(())
                }
                None => {
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();
                    match configuration.format(format) {
                        Ok(output) => {
                            println!("{}", output);
                            Ok(())
                        }
                        Err(e) => Err(CliError::FormattingError(e)),
                    }
                }
                _ => unreachable!(),
            },
            Some((COMMAND_LIST, sub_matches)) => {
                let _format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                let _format = OutputFormat::from_str(_format).unwrap();

                match configuration.format(_format) {
                    Ok(output) => {
                        println!("{}", output);
                        Ok(())
                    }
                    Err(e) => Err(CliError::FormattingError(e)),
                }
            }
            Some((COMMAND_EXPORT, sub_matches)) => {
                let path = sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_OUTPUT.to_string()))?;
                configuration.save(path)?;
                Ok(())
            }
            Some((COMMAND_IMPORT, sub_matches)) => {
                let path = sub_matches.get_one::<PathBuf>(PARAMETER_INPUT)
                    .ok_or(CliError::MissingRequiredArgument(PARAMETER_INPUT.to_string()))?;
                // Implementation would import configuration
                println!("Importing configuration from: {:?}", path);
                Ok(())
            }
            None => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                sub_matches,
            ))),
            _ => unreachable!(),
        },
        None => Err(CliError::UnsupportedSubcommand(String::from("unknown"))),
        _ => unreachable!(),
    }
}

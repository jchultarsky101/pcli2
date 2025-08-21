use pcli2::commands::{
    create_cli_commands, COMMAND_AUTH, COMMAND_CLEAR, COMMAND_CONFIG, COMMAND_CONTEXT, 
    COMMAND_FOLDER, COMMAND_GET, COMMAND_EXPORT, COMMAND_IMPORT, COMMAND_LIST, 
    COMMAND_LOGIN, COMMAND_LOGOUT, COMMAND_SET, COMMAND_TENANT,
    PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, 
    PARAMETER_FORMAT, PARAMETER_INPUT, PARAMETER_NAME, 
    PARAMETER_OUTPUT, PARAMETER_TENANT,
};
use pcli2::format::{OutputFormat, OutputFormatter};
use clap::ArgMatches;
use inquire::Select;
use pcli2::auth::AuthClient;
use pcli2::configuration::Configuration;
use pcli2::keyring::Keyring;
use pcli2::physna_v3::PhysnaApiClient;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use tracing::{debug, trace, error};

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Undefined or unsupported subcommand")]
    UnsupportedSubcommand(String),
    #[error("Configuration error")]
    ConfigurationError(#[from] pcli2::configuration::ConfigurationError),
    #[error("Formatting error")]
    FormattingError(#[from] pcli2::format::FormattingError),
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
    _api: (), // We're using Physna V3 API directly
) -> Result<(), CliError> {
    trace!("Executing CLI command");
    let commands = create_cli_commands();

    match commands.subcommand() {
        // Tenant resource commands
        Some((COMMAND_TENANT, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing tenant list command");
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
                                    trace!("Displaying list of available tenants");
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
                                    error!("Error fetching tenants: {}", e);
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
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        // Folder resource commands
        Some((COMMAND_FOLDER, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing folder list command");
                    // Get tenant from explicit parameter or fall back to active tenant from configuration
                    let tenant = match sub_matches.get_one::<String>(PARAMETER_TENANT) {
                        Some(tenant_id) => tenant_id.clone(),
                        None => {
                            // Try to get active tenant from configuration
                            if let Some(active_tenant_id) = configuration.get_active_tenant_id() {
                                active_tenant_id
                            } else {
                                return Err(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()));
                            }
                        }
                    };
                    
                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                    let format = OutputFormat::from_str(format).unwrap();
                    
                    // Try to get access token and list folders from Physna V3 API
                    let keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let client = PhysnaApiClient::new().with_access_token(token);
                            match client.list_folders(&tenant).await {
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
                                    error!("Error fetching folders: {}", e);
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
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        // Authentication commands
        Some((COMMAND_AUTH, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LOGIN, sub_matches)) => {
                    trace!("Executing login command");
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
                    trace!("Executing logout command");
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
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        // Context commands
        Some((COMMAND_CONTEXT, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_SET, sub_matches)) => {
                    match sub_matches.subcommand() {
                        Some(("tenant", sub_matches)) => {
                            trace!("Executing context set tenant command");
                            let name = sub_matches.get_one::<String>(PARAMETER_NAME);
                            
                            // Try to get access token and fetch tenant info from Physna V3 API
                            let keyring = Keyring::default();
                            match keyring.get(&"default".to_string(), "access-token".to_string()) {
                                Ok(Some(token)) => {
                                    let client = PhysnaApiClient::new().with_access_token(token);
                                    match client.list_tenants().await {
                                        Ok(tenants) => {
                                            // If no name was provided, show interactive selection
                                            let selected_tenant = if let Some(name) = name {
                                                // Find tenant by name (existing logic)
                                                tenants.iter().find(|t| 
                                                    t.tenant_display_name == *name || t.tenant_short_name == *name).cloned()
                                            } else {
                                                // Interactive selection using TUI
                                                if tenants.is_empty() {
                                                    eprintln!("No tenants available");
                                                    return Ok(());
                                                }
                                                
                                                // Create options for the select menu
                                                let options: Vec<String> = tenants.iter()
                                                    .map(|tenant| format!("{} ({})", tenant.tenant_display_name, tenant.tenant_id))
                                                    .collect();
                                                
                                                // Use inquire to create an interactive selection
                                                let ans = Select::new("Select a tenant:", options)
                                                    .with_help_message("Choose the tenant you want to set as active")
                                                    .prompt();
                                                    
                                                match ans {
                                                    Ok(choice) => {
                                                        // Find the tenant that matches the selection
                                                        tenants.iter().find(|tenant| {
                                                            choice == format!("{} ({})", tenant.tenant_display_name, tenant.tenant_id)
                                                        }).cloned()
                                                    }
                                                    Err(_) => {
                                                        eprintln!("No tenant selected");
                                                        return Ok(());
                                                    }
                                                }
                                            };
                                            
                                            // Set the active tenant in configuration
                                            if let Some(tenant) = selected_tenant {
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
                                                eprintln!("Tenant '{}' not found", name.unwrap()); // Safe to unwrap since we checked above
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
                        _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                            sub_matches,
                        ))),
                    }
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing context get command");
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
                Some((COMMAND_CLEAR, sub_matches)) => {
                    trace!("Executing context clear command");
                    match sub_matches.subcommand() {
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
                        _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                            sub_matches,
                        ))),
                    }
                }
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        // Configuration commands
        Some((COMMAND_CONFIG, sub_matches)) => {
            trace!("Executing config command");
            match sub_matches.subcommand() {
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing config get command");
                    match sub_matches.subcommand() {
                        Some(("path", _)) => {
                            let path = Configuration::get_default_configuration_file_path()?;
                            let path = path.into_os_string().into_string()
                                .map_err(|_| CliError::ConfigurationError(
                                    pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory))?;
                            println!("{}", path);
                            Ok(())
                        }
                        _ => {
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
                    }
                }
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing config list command");
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
                    trace!("Executing config export command");
                    let path = sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_OUTPUT.to_string()))?;
                    configuration.save(path)?;
                    Ok(())
                }
                Some((COMMAND_IMPORT, sub_matches)) => {
                    trace!("Executing config import command");
                    let path = sub_matches.get_one::<PathBuf>(PARAMETER_INPUT)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_INPUT.to_string()))?;
                    // Implementation would import configuration
                    debug!("Importing configuration from: {:?}", path);
                    Ok(())
                }
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        _ => Err(CliError::UnsupportedSubcommand(String::from("unknown"))),
    }
}
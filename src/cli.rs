//! CLI command execution logic.
//!
//! This module contains the core logic for executing CLI commands parsed by the
//! command definition module. It handles the execution of all supported commands
//! including tenant, folder, asset, authentication, context, and configuration operations.

use clap::ArgMatches;
use pcli2::{
    actions::{
        assets::{
            create_asset, create_asset_batch, delete_asset, list_assets, print_asset, download_asset
        },
        folders::{
            create_folder,
            delete_folder,
            list_folders,
            print_folder_details
        },
        tenants::{
            clear_active_tenant,
            get_tenant_details,
            list_all_tenants,
            print_active_tenant_name,
            print_current_context,
            set_active_tenant
        }
    },
    commands::{
        create_cli_commands,
        params::{
            COMMAND_ASSET, COMMAND_AUTH, COMMAND_CLEAR, COMMAND_CONFIG, COMMAND_CONTEXT, COMMAND_CREATE, COMMAND_CREATE_BATCH, COMMAND_DELETE, COMMAND_DOWNLOAD, COMMAND_EXPORT, COMMAND_FOLDER, COMMAND_GET, COMMAND_IMPORT, COMMAND_LIST, COMMAND_LOGIN, COMMAND_LOGOUT, COMMAND_SET, COMMAND_TENANT, PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_FILE, PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_OUTPUT, PARAMETER_PRETTY
        }
    },
    format::{Formattable, OutputFormat, OutputFormatOptions}};
use pcli2::error_utils;
use pcli2::auth::AuthClient;
use pcli2::configuration::Configuration;
use pcli2::keyring::Keyring;
use pcli2::error::CliError;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{debug, trace};

fn extract_subcommand_name(sub_matches: &ArgMatches) -> String {
    let message = match sub_matches.subcommand() {
        Some(m) => m.0,
        None => "unknown",
    };

    message.to_string()
}

pub async fn execute_command() -> Result<(), CliError> {
    let commands = create_cli_commands();
    
    // Check for verbose flag and set up tracing level accordingly
    if commands.get_flag("verbose") {
        // Set tracing level to debug if verbose flag is present
        std::env::set_var("RUST_LOG", "debug");
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .ok(); // Silently handle if tracing is already initialized
    }
    
    trace!("Executing CLI command");

    match commands.subcommand() {
        // Tenant resource commands
        Some((COMMAND_TENANT, sub_matches)) => {
            trace!("Command: {}", COMMAND_TENANT);
            
            match sub_matches.subcommand() {
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_TENANT, COMMAND_LIST);

                    list_all_tenants(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_TENANT, COMMAND_GET);

                    get_tenant_details(sub_matches).await?;
                    Ok(())
                }
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        // Folder resource commands
        Some((COMMAND_FOLDER, sub_matches)) => {
            trace!("Command: {}", COMMAND_FOLDER);
            
            match sub_matches.subcommand() {
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_LIST);

                    list_folders(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_GET);

                    print_folder_details(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CREATE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_CREATE);

                    create_folder(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_DELETE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_DELETE);

                    delete_folder(sub_matches).await?;
                    Ok(())
                }
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
            }
        }
        // Asset commands
        Some((COMMAND_ASSET, sub_matches)) => {
            trace!("Command: {}", COMMAND_ASSET);
            
            match sub_matches.subcommand() {
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_GET);

                    print_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CREATE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_CREATE);
                    trace!("Routing to asset create...");

                    create_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_LIST);
                    trace!("Routing to asset list...");

                    list_assets(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CREATE_BATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_CREATE_BATCH);
                    trace!("Routing to asset batch create...");

                    create_asset_batch(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_DOWNLOAD, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_DOWNLOAD);
                    trace!("Routing to asset download...");

                    download_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_DELETE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_DELETE);
                    trace!("Routing to asset delete...");

                    delete_asset(sub_matches).await?;
                    Ok(())
                }
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                )))
            }
        }
        // Authentication commands
        Some((COMMAND_AUTH, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LOGIN, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_AUTH, COMMAND_LOGIN);
                    
                    // Try to get client credentials from command line or stored values
                    let mut keyring = Keyring::default();
                    let client_id = match sub_matches.get_one::<String>(PARAMETER_CLIENT_ID) {
                        Some(id) => id.clone(),
                        None => {
                            // Try to get stored client ID
                            match keyring.get("default", "client-id".to_string()) {
                                Ok(Some(stored_id)) => stored_id,
                                _ => {
                                    return Err(CliError::MissingRequiredArgument(PARAMETER_CLIENT_ID.to_string()));
                                }
                            }
                        }
                    };
                    
                    let client_secret = match sub_matches.get_one::<String>(PARAMETER_CLIENT_SECRET) {
                        Some(secret) => secret.clone(),
                        None => {
                            // Try to get stored client secret
                            match keyring.get("default", "client-secret".to_string()) {
                                Ok(Some(stored_secret)) => stored_secret,
                                _ => {
                                    return Err(CliError::MissingRequiredArgument(PARAMETER_CLIENT_SECRET.to_string()));
                                }
                            }
                        }
                    };
                    
                    let auth_client = AuthClient::new(client_id.clone(), client_secret.clone());
                    
                    // Store the client credentials so they're available for token refresh
                    let client_id_result = keyring.put("default", "client-id".to_string(), client_id.clone());
                    let client_secret_result = keyring.put("default", "client-secret".to_string(), client_secret.clone());
                    
                    if client_id_result.is_err() || client_secret_result.is_err() {
                        eprintln!("Error storing client credentials");
                        return Err(CliError::SecurityError(String::from("Failed to store client credentials")));
                    }
                    
                    match auth_client.get_access_token().await {
                        Ok(token) => {
                            // Store the access token
                            let token_result = keyring.put("default", "access-token".to_string(), token);
                            
                            if token_result.is_ok() {
                                Ok(())
                            } else {
                                eprintln!("Error storing access token");
                                Err(CliError::SecurityError(String::from("Failed to store access token")))
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
                    let mut keyring = Keyring::default();
                    match keyring.delete("default", "access-token".to_string()) {
                        Ok(()) => {
                            Ok(())
                        }
                        Err(e) => {
                            eprintln!("Error deleting access token: {}", e);
                            Err(CliError::SecurityError(String::from("Failed to delete access token")))
                        }
                    }
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing auth token get command");
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                    let format = OutputFormat::from_str(&format_str).unwrap();
                    
                    // Try to get access token from keyring
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            // Output the token based on the requested format
                            match format {
                                _ => {
                                    println!("{{\"access_token\": \"{}\"}}", token);
                                }
                            }
                            Ok(())
                        }
                        Ok(None) => {
                            eprintln!("No access token found. Please login first.");
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
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
            trace!("Command: context");

            match sub_matches.subcommand() {
                Some((COMMAND_SET, sub_matches)) => {
                    trace!("Command: context set");

                    match sub_matches.subcommand() {
                        Some((COMMAND_TENANT, sub_matches)) => {
                            trace!("Command: context set tenant");

                            set_active_tenant(sub_matches).await?;

                            Ok(())
                        }
                        _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                            sub_matches,
                        ))),
                    }
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Command: context get");

                    match sub_matches.subcommand() {
                        Some((COMMAND_TENANT, _)) => {
                            trace!("Command: context get tenant");
                            print_active_tenant_name().await?;
                            Ok(())
                        }
                        None => {
                            // Handle context get without subcommand
                            trace!("Command: context get (no subcommand)");
                            print_current_context(sub_matches).await?;
                            Ok(())
                        }
                        _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                            sub_matches,
                        ))),
                    }
                }
                Some((COMMAND_CLEAR, sub_matches)) => {
                    trace!("Command: context clear");

                    match sub_matches.subcommand() {
                        Some((COMMAND_TENANT, _)) => {
                            trace!("Command: context clear tenant");
                            clear_active_tenant().await?;
                            Ok(())
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
                            // Get format parameters directly from sub_matches since config commands don't have all format flags
                            let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();

                            let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
                            let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
                            // Note: config commands don't have metadata flag

                            let format_options = OutputFormatOptions {
                                with_metadata: false,  // No metadata for config
                                with_headers,
                                pretty,
                            };

                            let format = OutputFormat::from_string_with_options(format_str, format_options).unwrap();

                            let configuration = Configuration::load_or_create_default()?;
                            match configuration.format(&format) {
                                Ok(output) => {
                                    println!("{}", output);
                                    Ok(())
                                }
                                Err(e) => Err(CliError::FormattingError(e)),
                            }
                        }
                    }
                }
                Some((COMMAND_EXPORT, sub_matches)) => {
                    trace!("Executing config export command");
                    let path = sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_OUTPUT.to_string()))?;
                    let configuration = Configuration::load_or_create_default()?;
                    configuration.save(path)?;
                    Ok(())
                }
                Some((COMMAND_IMPORT, sub_matches)) => {
                    trace!("Executing config import command");
                    let path = sub_matches.get_one::<PathBuf>(PARAMETER_FILE)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_FILE.to_string()))?;
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


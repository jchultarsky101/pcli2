//! CLI command execution logic.
//!
//! This module contains the core logic for executing CLI commands parsed by the
//! command definition module. It handles the execution of all supported commands
//! including tenant, folder, asset, authentication, context, and configuration operations.

use clap::ArgMatches;
use pcli2::{
    actions::{
        assets::{
            create_asset, create_asset_batch, create_asset_metadata_batch, delete_asset, delete_asset_metadata, download_asset, geometric_match_asset, geometric_match_folder, list_assets, metadata_inference, part_match_asset, part_match_folder, print_asset, print_asset_dependencies, print_asset_metadata, print_folder_dependencies, text_match, update_asset_metadata, visual_match_asset, visual_match_folder
        },
        folders::{
            create_folder, delete_folder, download_folder, list_folders, move_folder, print_folder_details, rename_folder, resolve_folder, upload_folder
        },
        tenants::{
            clear_active_tenant,
            get_tenant_details,
            list_all_tenants,
            print_active_tenant_name_with_format,
            set_active_tenant
        }
    },
    commands::{
        create_cli_commands,
        params::{
            COMMAND_ASSET, COMMAND_AUTH, COMMAND_CLEAR, COMMAND_CLEAR_TOKEN, COMMAND_CONFIG, COMMAND_CREATE, COMMAND_CREATE_BATCH, COMMAND_CURRENT, COMMAND_DELETE, COMMAND_DEPENDENCIES, COMMAND_DOWNLOAD, COMMAND_EXPORT, COMMAND_FOLDER, COMMAND_GET, COMMAND_IMPORT, COMMAND_INFERENCE, COMMAND_LIST, COMMAND_LOGIN, COMMAND_LOGOUT, COMMAND_MATCH, COMMAND_METADATA, COMMAND_PART_MATCH, COMMAND_STATE, COMMAND_TENANT, COMMAND_TEXT_MATCH, COMMAND_UPLOAD, COMMAND_USE, COMMAND_VISUAL_MATCH, PARAMETER_API_URL, PARAMETER_AUTH_URL, PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_FILE, PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_OUTPUT, PARAMETER_PRETTY, PARAMETER_UI_URL
        }
    },
    format::{Formattable, FormattingError, OutputFormat, OutputFormatOptions}};
use pcli2::error_utils;
use pcli2::auth::AuthClient;
use pcli2::configuration::Configuration;
use pcli2::keyring::Keyring;
use pcli2::error::CliError;
use std::path::PathBuf;
use tracing::{debug, trace};
use base64::Engine;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Local, Utc};
use pcli2::commands::params::COMMAND_EXPIRATION;

#[derive(Debug)]
struct TokenExpirationInfo {
    expiration_datetime: String,
    time_remaining_formatted: String,
}

fn decode_jwt_expiration(token: &str) -> Result<TokenExpirationInfo, Box<dyn std::error::Error>> {
    // Split the JWT token into its three parts: header.payload.signature
    let parts: Vec<&str> = token.split('.').collect();

    if parts.len() != 3 {
        return Err("Invalid JWT format: token must have exactly 3 parts separated by dots".into());
    }

    // Decode the payload (the middle part)
    let payload = parts[1];

    // Add padding if necessary (JWTs use base64url encoding without padding)
    let mut padded_payload = payload.to_string();
    match payload.len() % 4 {
        2 => padded_payload.push_str("=="),
        3 => padded_payload.push_str("="),
        _ => {} // 0 remainder means no padding needed, 1 remainder is invalid
    }

    // Decode the base64url-encoded payload
    let decoded_bytes = base64::engine::general_purpose::URL_SAFE.decode(&padded_payload)?;
    let payload_str = String::from_utf8(decoded_bytes)?;

    // Parse the JSON payload
    let payload_json: serde_json::Value = serde_json::from_str(&payload_str)?;

    // Extract the 'exp' claim (expiration time)
    let exp = payload_json.get("exp")
        .and_then(|v| v.as_i64())
        .ok_or("Token does not contain an 'exp' (expiration) claim")?;

    // Calculate time remaining
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;

    let time_remaining = exp - current_time;

    // Format the expiration time as a readable datetime in local timezone
    let expiration_datetime = DateTime::<Utc>::from_timestamp(exp, 0)
        .map(|utc_dt| {
            let local_dt = utc_dt.with_timezone(&Local);
            local_dt.format("%Y-%m-%d %H:%M:%S %Z").to_string()
        })
        .unwrap_or_else(|| "Invalid timestamp".to_string());

    // Format the time remaining in a human-readable format
    let time_remaining_formatted = if time_remaining > 0 {
        let days = time_remaining / (24 * 3600);
        let hours = (time_remaining % (24 * 3600)) / 3600;
        let minutes = (time_remaining % 3600) / 60;
        let seconds = time_remaining % 60;

        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
        } else if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    } else {
        "Expired".to_string()
    };

    Ok(TokenExpirationInfo {
        expiration_datetime,
        time_remaining_formatted,
    })
}

fn extract_subcommand_name(sub_matches: &ArgMatches) -> String {
    let message = match sub_matches.subcommand() {
        Some(m) => m.0,
        None => "unknown",
    };

    message.to_string()
}

pub async fn execute_command() -> Result<(), CliError> {
    let commands = create_cli_commands();

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
                Some((COMMAND_USE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_TENANT, COMMAND_USE);

                    set_active_tenant(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CURRENT, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_TENANT, COMMAND_CURRENT);

                    print_active_tenant_name_with_format(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CLEAR, _sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_TENANT, COMMAND_CLEAR);

                    clear_active_tenant().await?;
                    Ok(())
                }
                Some((COMMAND_STATE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_TENANT, COMMAND_STATE);

                    pcli2::actions::tenants::get_tenant_state_counts(sub_matches).await?;
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
                Some(("rename", sub_matches)) => {
                    trace!("Command: {} rename", COMMAND_FOLDER);

                    rename_folder(sub_matches).await?;
                    Ok(())
                }
                Some(("move", sub_matches)) => {
                    trace!("Command: {} move", COMMAND_FOLDER);

                    move_folder(sub_matches).await?;
                    Ok(())
                }
                Some(("resolve", sub_matches)) => {
                    trace!("Command: {} resolve", COMMAND_FOLDER);

                    resolve_folder(sub_matches).await?;
                    Ok(())
                }
                Some(("download", sub_matches)) => {
                    trace!("Command: {} download", COMMAND_FOLDER);

                    download_folder(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_MATCH);
                    geometric_match_folder(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_PART_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_PART_MATCH);
                    part_match_folder(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_VISUAL_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_VISUAL_MATCH);
                    visual_match_folder(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_DEPENDENCIES, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_DEPENDENCIES);
                    print_folder_dependencies(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_UPLOAD, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_FOLDER, COMMAND_UPLOAD);
                    upload_folder(sub_matches).await?;
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
                Some((COMMAND_DEPENDENCIES, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_DEPENDENCIES);

                    print_asset_dependencies(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CREATE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_CREATE);
                    create_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_LIST);
                    list_assets(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_CREATE_BATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_CREATE_BATCH);
                    create_asset_batch(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_DOWNLOAD, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_DOWNLOAD);
                    download_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_DELETE, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_DELETE);
                    delete_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_MATCH);
                    geometric_match_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_PART_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_PART_MATCH);
                    part_match_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_VISUAL_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_VISUAL_MATCH);
                    visual_match_asset(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_TEXT_MATCH, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_TEXT_MATCH);
                    text_match(sub_matches).await?;
                    Ok(())
                }
                Some((COMMAND_METADATA, sub_matches)) => {
                    trace!("Command: {} {}", COMMAND_ASSET, COMMAND_METADATA);

                    match sub_matches.subcommand() {
                        Some((COMMAND_CREATE, sub_matches)) => {
                            trace!("Command: {} {} {}", COMMAND_ASSET, COMMAND_METADATA, COMMAND_CREATE);
                            update_asset_metadata(sub_matches).await?;
                            Ok(())
                        }
                        Some((COMMAND_GET, sub_matches)) => {
                            trace!("Command: {} {} {}", COMMAND_ASSET, COMMAND_METADATA, COMMAND_GET);
                            print_asset_metadata(sub_matches).await?;
                            Ok(())
                        }
                        Some((COMMAND_DELETE, sub_matches)) => {
                            trace!("Command: {} {} {}", COMMAND_ASSET, COMMAND_METADATA, COMMAND_DELETE);
                            delete_asset_metadata(sub_matches).await?;
                            Ok(())
                        }
                        Some((COMMAND_INFERENCE, sub_matches)) => {
                            trace!("Command: {} {} {}", COMMAND_ASSET, COMMAND_METADATA, COMMAND_INFERENCE);
                            metadata_inference(sub_matches).await?;
                            Ok(())
                        }
                        Some(("create-batch", sub_matches)) => {
                            trace!("Command: {} {} create-batch", COMMAND_ASSET, COMMAND_METADATA);
                            create_asset_metadata_batch(sub_matches).await?;
                            Ok(())
                        }
                        _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                            sub_matches,
                        )))
                    }
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

                    let configuration = Configuration::load_or_create_default()?;

                    // Use the active environment name for keyring storage, fallback to "default" if no environment is set
                    let environment_name = configuration.get_active_environment().unwrap_or_else(|| "default".to_string());

                    // Try to get client credentials from command line or stored values
                    #[allow(unused_mut)]
                    let mut keyring = Keyring::default();
                    let client_id = match sub_matches.get_one::<String>(PARAMETER_CLIENT_ID) {
                        Some(id) => id.clone(),
                        None => {
                            // Try to get stored client ID
                            match keyring.get(&environment_name, "client-id".to_string()) {
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
                            match keyring.get(&environment_name, "client-secret".to_string()) {
                                Ok(Some(stored_secret)) => stored_secret,
                                _ => {
                                    return Err(CliError::MissingRequiredArgument(PARAMETER_CLIENT_SECRET.to_string()));
                                }
                            }
                        }
                    };

                    let auth_client = AuthClient::new_with_configuration(client_id.clone(), client_secret.clone(), &configuration);

                    // Store the client credentials so they're available for token refresh
                    let client_id_result = keyring.put(&environment_name, "client-id".to_string(), client_id.clone());
                    let client_secret_result = keyring.put(&environment_name, "client-secret".to_string(), client_secret.clone());

                    if client_id_result.is_err() || client_secret_result.is_err() {
                        error_utils::report_error_with_remediation(
                            &CliError::SecurityError(String::from("Failed to store client credentials")),
                            &[
                                "Check that your system's keyring service is running",
                                "Try logging in again with 'pcli2 auth login'"
                            ]
                        );
                        return Err(CliError::SecurityError(String::from("Failed to store client credentials")));
                    }

                    match auth_client.get_access_token().await {
                        Ok(token) => {
                            // Store the access token
                            let token_result = keyring.put(&environment_name, "access-token".to_string(), token);

                            if token_result.is_ok() {
                                Ok(())
                            } else {
                                error_utils::report_error_with_remediation(
                                    &CliError::SecurityError(String::from("Failed to store access token")),
                                    &[
                                        "Check that your system's keyring service is running",
                                        "Try logging in again with 'pcli2 auth login'"
                                    ]
                                );
                                Err(CliError::SecurityError(String::from("Failed to store access token")))
                            }
                        }
                        Err(e) => {
                            error_utils::report_error_with_remediation(
                                &format!("Login failed: {}", e),
                                &[
                                    "Verify your client ID and client secret are correct",
                                    "Check your internet connection",
                                    "Ensure your credentials have not expired"
                                ]
                            );
                            Err(CliError::SecurityError(String::from("Login failed")))
                        }
                    }
                }
                Some((COMMAND_LOGOUT, _)) => {
                    trace!("Executing logout command");

                    let configuration = Configuration::load_or_create_default()?;
                    // Use the active environment name for keyring storage, fallback to "default" if no environment is set
                    let environment_name = configuration.get_active_environment().unwrap_or_else(|| "default".to_string());

                    #[allow(unused_mut)]
                    let mut keyring = Keyring::default();
                    match keyring.delete(&environment_name, "access-token".to_string()) {
                        Ok(()) => {
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error_with_remediation(
                                &format!("Error deleting access token: {}", e),
                                &[
                                    "Check that your system's keyring service is running",
                                    "Try logging in again with 'pcli2 auth login'"
                                ]
                            );
                            Err(CliError::SecurityError(String::from("Failed to delete access token")))
                        }
                    }
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing auth token get command");

                    // Get format parameters directly from sub_matches since auth commands don't have metadata flag
                    let format_str_owned = if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
                        format_val.clone()
                    } else {
                        "json".to_string()
                    };
                    let format_str = &format_str_owned;

                    let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
                    let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
                    // Note: auth commands don't have metadata flag

                    let format_options = OutputFormatOptions {
                        with_metadata: false,  // No metadata for auth
                        with_headers,
                        pretty,
                    };

                    let format = OutputFormat::from_string_with_options(format_str, format_options).unwrap();

                    let configuration = Configuration::load_or_create_default()?;
                    // Use the active environment name for keyring storage, fallback to "default" if no environment is set
                    let environment_name = configuration.get_active_environment().unwrap_or_else(|| "default".to_string());

                    // Try to get access token from keyring
                    #[allow(unused_mut)]
                    let mut keyring = Keyring::default();
                    match keyring.get(&environment_name, "access-token".to_string()) {
                        Ok(Some(token)) => {
                            // Create a simple struct to hold the token for formatting
                            #[derive(serde::Serialize)]
                            struct TokenResponse {
                                access_token: String,
                            }

                            let token_response = TokenResponse {
                                access_token: token,
                            };

                            // Format the response based on the requested format
                            match format {
                                OutputFormat::Json(options) => {
                                    let json_output = if options.pretty {
                                        serde_json::to_string_pretty(&token_response)
                                    } else {
                                        serde_json::to_string(&token_response)
                                    };
                                    match json_output {
                                        Ok(json) => println!("{}", json),
                                        Err(e) => return Err(CliError::FormattingError(FormattingError::JsonSerializationError(e))),
                                    }
                                },
                                OutputFormat::Csv(options) => {
                                    let mut wtr = csv::Writer::from_writer(vec![]);

                                    if options.with_headers {
                                        if let Err(e) = wtr.serialize(("ACCESS_TOKEN",)) {
                                            return Err(CliError::FormattingError(FormattingError::CsvError(e)));
                                        }
                                    }

                                    if let Err(e) = wtr.serialize((&token_response.access_token,)) {
                                        return Err(CliError::FormattingError(FormattingError::CsvError(e)));
                                    }

                                    let data = match wtr.into_inner() {
                                        Ok(data) => data,
                                        Err(e) => return Err(CliError::FormattingError(FormattingError::CsvIntoInnerError(e))),
                                    };
                                    let csv_output = match String::from_utf8(data) {
                                        Ok(csv_str) => csv_str,
                                        Err(e) => return Err(CliError::FormattingError(FormattingError::Utf8Error(e))),
                                    };
                                    print!("{}", csv_output);
                                },
                                OutputFormat::Tree(_) => {
                                    // For tree format, just print the token
                                    println!("{}", token_response.access_token);
                                }
                            }
                            Ok(())
                        }
                        Ok(None) => {
                            error_utils::report_error_with_remediation(
                                &"No access token found. Please login first.",
                                &[
                                    "Log in with 'pcli2 auth login --client-id <id> --client-secret <secret>'",
                                    "Verify your credentials are correct"
                                ]
                            );
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
                Some((COMMAND_CLEAR_TOKEN, _)) => {
                    trace!("Executing auth clear-token command");

                    let configuration = Configuration::load_or_create_default()?;
                    // Use the active environment name for keyring storage, fallback to "default" if no environment is set
                    let environment_name = configuration.get_active_environment().unwrap_or_else(|| "default".to_string());

                    #[allow(unused_mut)]
                    let mut keyring = Keyring::default();

                    // Try to get the current token to see if one exists
                    match keyring.get(&environment_name, "access-token".to_string()) {
                        Ok(Some(_token)) => {
                            // Token exists, try to delete it
                            match keyring.delete(&environment_name, "access-token".to_string()) {
                                Ok(()) => {
                                    println!("Access token cleared successfully");
                                    Ok(())
                                }
                                Err(e) => {
                                    error_utils::report_error_with_remediation(
                                        &format!("Error deleting access token: {}", e),
                                        &[
                                            "Check that your system's keyring service is running",
                                            "Try logging in again with 'pcli2 auth login'"
                                        ]
                                    );
                                    Err(CliError::SecurityError(String::from("Failed to delete access token")))
                                }
                            }
                        }
                        Ok(None) => {
                            // No token exists, which is fine
                            println!("No access token found to clear");
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error_with_remediation(
                                &format!("Error checking for access token: {}", e),
                                &[
                                    "Check that your system's keyring service is running",
                                    "Try logging in again with 'pcli2 auth login'"
                                ]
                            );
                            Err(CliError::SecurityError(String::from("Failed to check access token")))
                        }
                    }
                }
                Some((command_name, sub_matches)) if command_name == COMMAND_EXPIRATION => {
                    trace!("Executing auth expiration command");

                    let configuration = Configuration::load_or_create_default()?;
                    // Use the active environment name for keyring storage, fallback to "default" if no environment is set
                    let environment_name = configuration.get_active_environment().unwrap_or_else(|| "default".to_string());

                    // Try to get access token from keyring
                    #[allow(unused_mut)]
                    let mut keyring = Keyring::default();
                    match keyring.get(&environment_name, "access-token".to_string()) {
                        Ok(Some(token)) => {
                            // Decode the JWT token to get expiration information
                            match decode_jwt_expiration(&token) {
                                Ok(expiration_info) => {
                                    // Print plain text output
                                    println!("Expiration Time: {}", expiration_info.expiration_datetime);
                                    println!("Time Remaining: {}", expiration_info.time_remaining_formatted);
                                    Ok(())
                                }
                                Err(e) => {
                                    error_utils::report_error_with_remediation(
                                        &format!("Error decoding token expiration: {}", e),
                                        &[
                                            "The token might not be a valid JWT",
                                            "Check that your token is properly formatted",
                                            "Try logging in again with 'pcli2 auth login'"
                                        ]
                                    );
                                    Err(CliError::SecurityError(String::from("Failed to decode token expiration")))
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error_with_remediation(
                                &"No access token found. Please login first.",
                                &[
                                    "Log in with 'pcli2 auth login --client-id <id> --client-secret <secret>'",
                                    "Verify your credentials are correct"
                                ]
                            );
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
                            // Get format parameters with precedence: 1) explicit --format, 2) PCLI2_FORMAT env var, 3) default "json"
                            let format_str_owned = if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
                                // User explicitly provided --format argument
                                format_val.clone()
                            } else {
                                // Format was not explicitly provided by user, check environment variable first
                                if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
                                    env_format
                                } else {
                                    // Use default value
                                    "json".to_string()
                                }
                            };
                            let format_str = &format_str_owned;

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
                Some(("environment", sub_matches)) => {
                    trace!("Executing config environment command");
                    match sub_matches.subcommand() {
                        Some(("add", sub_matches)) => {
                            trace!("Executing config environment add command");

                            let env_name = sub_matches.get_one::<String>("name")
                                .ok_or(CliError::MissingRequiredArgument("name".to_string()))?;

                            let api_url = sub_matches.get_one::<String>(PARAMETER_API_URL).cloned();
                            let ui_url = sub_matches.get_one::<String>(PARAMETER_UI_URL).cloned();
                            let auth_url = sub_matches.get_one::<String>(PARAMETER_AUTH_URL).cloned();

                            let mut configuration = Configuration::load_or_create_default()?;

                            let env_config = pcli2::configuration::EnvironmentConfig {
                                api_base_url: api_url.unwrap_or_else(||
                                    pcli2::configuration::default_api_base_url()),
                                ui_base_url: ui_url.unwrap_or_else(||
                                    pcli2::configuration::default_ui_base_url()),
                                auth_base_url: auth_url.unwrap_or_else(||
                                    pcli2::configuration::default_auth_base_url()),
                            };

                            configuration.add_environment(env_name.clone(), env_config);
                            configuration.save_to_default()?;

                            println!("Environment '{}' added successfully", env_name);
                            Ok(())
                        }
                        Some(("use", sub_matches)) => {
                            trace!("Executing config environment use command");

                            let env_name = sub_matches.get_one::<String>("name");

                            let mut configuration = Configuration::load_or_create_default()?;

                            let selected_env_name = if let Some(name) = env_name {
                                // Use the provided name
                                name.clone()
                            } else {
                                // Interactive selection using TUI
                                let available_envs = configuration.list_environments();

                                if available_envs.is_empty() {
                                    error_utils::report_error_with_remediation(
                                        &"No environments available",
                                        &[
                                            "Add an environment with 'pcli2 config environment add'",
                                            "Check that your configuration file is properly set up"
                                        ]
                                    );
                                    return Ok(());
                                }

                                // Create options for the select menu
                                let options: Vec<String> = available_envs.iter()
                                    .map(|env_name| {
                                        let is_active = if let Some(ref active) = configuration.get_active_environment() {
                                            env_name == active
                                        } else {
                                            false
                                        };
                                        let active_status = if is_active { " (active)" } else { "" };
                                        format!("{}{}", env_name, active_status)
                                    })
                                    .collect();

                                // Use inquire to create an interactive selection
                                let ans = inquire::Select::new("Select an environment:", options)
                                    .with_help_message("Choose the environment you want to set as active")
                                    .prompt();

                                match ans {
                                    Ok(choice) => {
                                        // Extract the environment name from the choice (removing " (active)" if present)
                                        let env_name = choice.split_once(" (active)").map(|(before, _)| before.trim()).unwrap_or(&choice);
                                        env_name.to_string()
                                    }
                                    Err(_) => {
                                        error_utils::report_error_with_remediation(
                                            &"No environment selected",
                                            &[
                                                "Run 'pcli2 config environment use' again to select an environment",
                                                "Add an environment with 'pcli2 config environment add' if none exist"
                                            ]
                                        );
                                        return Ok(());
                                    }
                                }
                            };

                            configuration.set_active_environment(&selected_env_name)?;
                            // Clear the active tenant when switching environments to avoid confusion
                            configuration.clear_active_tenant();
                            configuration.save_to_default()?;

                            println!("Switched to environment '{}'. Select a tenant with 'pcli2 tenant use' before running commands.", selected_env_name);
                            Ok(())
                        }
                        Some(("remove", sub_matches)) => {
                            trace!("Executing config environment remove command");

                            let env_name = sub_matches.get_one::<String>("name")
                                .ok_or(CliError::MissingRequiredArgument("name".to_string()))?;

                            let mut configuration = Configuration::load_or_create_default()?;

                            configuration.remove_environment(env_name)?;
                            configuration.save_to_default()?;

                            println!("Environment '{}' removed successfully", env_name);
                            Ok(())
                        }
                        Some(("list", sub_matches)) => {
                            trace!("Executing config environment list command");

                            // Get format parameters with precedence: 1) explicit --format, 2) PCLI2_FORMAT env var, 3) default "json"
                            let format_str = if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
                                // User explicitly provided --format argument
                                format_val.clone()
                            } else {
                                // Format was not explicitly provided by user, check environment variable first
                                if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
                                    env_format
                                } else {
                                    // Use default value
                                    "json".to_string()
                                }
                            };

                            let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
                            let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
                            // Note: environment list commands don't have metadata flag

                            let format_options = OutputFormatOptions {
                                with_metadata: false,  // No metadata for environment list
                                with_headers,
                                pretty,
                            };

                            let format = OutputFormat::from_string_with_options(&format_str, format_options).unwrap();

                            let configuration = Configuration::load_or_create_default()?;
                            let active_env = configuration.get_active_environment();

                            // Create a detailed representation for display
                            #[derive(serde::Serialize)]
                            struct EnvironmentDetails {
                                name: String,
                                is_active: bool,
                                api_base_url: String,
                                ui_base_url: String,
                                auth_base_url: String,
                            }

                            let env_details: Vec<EnvironmentDetails> = configuration.list_environments()
                                .into_iter()
                                .map(|name| {
                                    let is_active = if let Some(ref active) = active_env {
                                        name == *active
                                    } else {
                                        false
                                    };

                                    let env_config = configuration.get_environment_config(&name).unwrap();

                                    EnvironmentDetails {
                                        name: name.clone(),
                                        is_active,
                                        api_base_url: env_config.api_base_url.clone(),
                                        ui_base_url: env_config.ui_base_url.clone(),
                                        auth_base_url: env_config.auth_base_url.clone(),
                                    }
                                })
                                .collect();

                            // Format the response based on the requested format
                            match format {
                                OutputFormat::Json(options) => {
                                    let json_output = if options.pretty {
                                        serde_json::to_string_pretty(&env_details)
                                    } else {
                                        serde_json::to_string(&env_details)
                                    };
                                    match json_output {
                                        Ok(json) => println!("{}", json),
                                        Err(e) => return Err(CliError::FormattingError(FormattingError::JsonSerializationError(e))),
                                    }
                                },
                                OutputFormat::Csv(options) => {
                                    let mut wtr = csv::Writer::from_writer(vec![]);

                                    if options.with_headers {
                                        if let Err(e) = wtr.serialize(("ENVIRONMENT_NAME", "IS_ACTIVE", "API_BASE_URL", "UI_BASE_URL", "AUTH_BASE_URL")) {
                                            return Err(CliError::FormattingError(FormattingError::CsvError(e)));
                                        }
                                    }

                                    for env_detail in &env_details {
                                        if let Err(e) = wtr.serialize((
                                            &env_detail.name,
                                            &env_detail.is_active,
                                            &env_detail.api_base_url,
                                            &env_detail.ui_base_url,
                                            &env_detail.auth_base_url,
                                        )) {
                                            return Err(CliError::FormattingError(FormattingError::CsvError(e)));
                                        }
                                    }

                                    let data = match wtr.into_inner() {
                                        Ok(data) => data,
                                        Err(e) => return Err(CliError::FormattingError(FormattingError::CsvIntoInnerError(e))),
                                    };
                                    let csv_output = match String::from_utf8(data) {
                                        Ok(csv_str) => csv_str,
                                        Err(e) => return Err(CliError::FormattingError(FormattingError::Utf8Error(e))),
                                    };
                                    print!("{}", csv_output);
                                },
                                OutputFormat::Tree(_) => {
                                    // For tree format, show detailed information
                                    for env_detail in &env_details {
                                        let active_status = if env_detail.is_active { " (active)" } else { "" };
                                        println!("{}{}:", env_detail.name, active_status);
                                        println!("  API Base URL: {}", env_detail.api_base_url);
                                        println!("  UI Base URL: {}", env_detail.ui_base_url);
                                        println!("  Auth Base URL: {}", env_detail.auth_base_url);
                                        println!(); // Empty line between environments
                                    }
                                }
                            }
                            Ok(())
                        }
                        Some(("reset", _sub_matches)) => {
                            trace!("Executing config environment reset command");

                            let mut configuration = Configuration::load_or_create_default()?;

                            configuration.reset_environments();
                            configuration.save_to_default()?;

                            println!("Environment configurations reset successfully");
                            Ok(())
                        }
                        Some(("get", sub_matches)) => {
                            trace!("Executing config environment get command");

                            // Get format parameters with precedence: 1) explicit --format, 2) PCLI2_FORMAT env var, 3) default "json"
                            let format_str = if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
                                // User explicitly provided --format argument
                                format_val.clone()
                            } else {
                                // Format was not explicitly provided by user, check environment variable first
                                if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
                                    env_format
                                } else {
                                    // Use default value
                                    "json".to_string()
                                }
                            };

                            let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
                            let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
                            // Note: environment get commands don't have metadata flag

                            let format_options = OutputFormatOptions {
                                with_metadata: false,  // No metadata for environment get
                                with_headers,
                                pretty,
                            };

                            let format = OutputFormat::from_string_with_options(&format_str, format_options).unwrap();

                            let env_name = sub_matches.get_one::<String>("name");

                            let configuration = Configuration::load_or_create_default()?;

                            let target_env_name = if let Some(name) = env_name {
                                // Use the provided name
                                name.clone()
                            } else {
                                // Use the active environment
                                if let Some(active_env) = configuration.get_active_environment() {
                                    active_env
                                } else {
                                    error_utils::report_error_with_remediation(
                                        &"No active environment set",
                                        &[
                                            "Set an active environment with 'pcli2 config environment use'",
                                            "Add an environment with 'pcli2 config environment add' if none exist"
                                        ]
                                    );
                                    return Ok(());
                                }
                            };

                            // Get the environment configuration
                            if let Some(env_config) = configuration.get_environment_config(&target_env_name) {
                                // Create a detailed representation for display
                                #[derive(serde::Serialize)]
                                struct EnvironmentDetails {
                                    name: String,
                                    is_active: bool,
                                    api_base_url: String,
                                    ui_base_url: String,
                                    auth_base_url: String,
                                }

                                let is_active = if let Some(active_env) = configuration.get_active_environment() {
                                    active_env == target_env_name
                                } else {
                                    false
                                };

                                let env_details = EnvironmentDetails {
                                    name: target_env_name,
                                    is_active,
                                    api_base_url: env_config.api_base_url.clone(),
                                    ui_base_url: env_config.ui_base_url.clone(),
                                    auth_base_url: env_config.auth_base_url.clone(),
                                };

                                // Format the response based on the requested format
                                match format {
                                    OutputFormat::Json(options) => {
                                        let json_output = if options.pretty {
                                            serde_json::to_string_pretty(&env_details)
                                        } else {
                                            serde_json::to_string(&env_details)
                                        };
                                        match json_output {
                                            Ok(json) => println!("{}", json),
                                            Err(e) => return Err(CliError::FormattingError(FormattingError::JsonSerializationError(e))),
                                        }
                                    },
                                    OutputFormat::Csv(options) => {
                                        let mut wtr = csv::Writer::from_writer(vec![]);

                                        if options.with_headers {
                                            if let Err(e) = wtr.serialize(("ENVIRONMENT_NAME", "IS_ACTIVE", "API_BASE_URL", "UI_BASE_URL", "AUTH_BASE_URL")) {
                                                return Err(CliError::FormattingError(FormattingError::CsvError(e)));
                                            }
                                        }

                                        if let Err(e) = wtr.serialize((
                                            &env_details.name,
                                            &env_details.is_active,
                                            &env_details.api_base_url,
                                            &env_details.ui_base_url,
                                            &env_details.auth_base_url,
                                        )) {
                                            return Err(CliError::FormattingError(FormattingError::CsvError(e)));
                                        }

                                        let data = match wtr.into_inner() {
                                            Ok(data) => data,
                                            Err(e) => return Err(CliError::FormattingError(FormattingError::CsvIntoInnerError(e))),
                                        };
                                        let csv_output = match String::from_utf8(data) {
                                            Ok(csv_str) => csv_str,
                                            Err(e) => return Err(CliError::FormattingError(FormattingError::Utf8Error(e))),
                                        };
                                        print!("{}", csv_output);
                                    },
                                    OutputFormat::Tree(_) => {
                                        // For tree format, output as JSON (since tree doesn't make sense for single environment)
                                        let json_output = serde_json::to_string_pretty(&env_details);
                                        match json_output {
                                            Ok(json) => println!("{}", json),
                                            Err(e) => return Err(CliError::FormattingError(FormattingError::JsonSerializationError(e))),
                                        }
                                    }
                                }
                            } else {
                                error_utils::report_error_with_remediation(
                                    &format!("Environment '{}' not found", target_env_name),
                                    &[
                                        "Check the environment name spelling",
                                        "List available environments with 'pcli2 config environment list'",
                                        "Add the environment with 'pcli2 config environment add'"
                                    ]
                                );
                            }

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
        Some(("completions", sub_matches)) => {
            trace!("Command: completions");

            let shell = sub_matches.get_one::<String>("shell")
                .ok_or(CliError::MissingRequiredArgument("shell is required".to_string()))?;

            // Generate completions for the specified shell
            pcli2::actions::completions::generate_completions(shell)?;
            Ok(())
        }
        _ => Err(CliError::UnsupportedSubcommand(String::from("unknown"))),
    }
}


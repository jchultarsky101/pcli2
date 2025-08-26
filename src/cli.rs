//! CLI command execution logic.
//!
//! This module contains the core logic for executing CLI commands parsed by the
//! command definition module. It handles the execution of all supported commands
//! including tenant, folder, asset, authentication, context, and configuration operations.

use clap::ArgMatches;
use inquire::Select;
use pcli2::commands::{
    create_cli_commands, COMMAND_ASSET, COMMAND_AUTH, COMMAND_CLEAR, COMMAND_CONFIG, COMMAND_CONTEXT, 
    COMMAND_CREATE, COMMAND_DELETE, COMMAND_EXPORT, COMMAND_FOLDER, COMMAND_GET, 
    COMMAND_IMPORT, COMMAND_LIST, COMMAND_LOGIN, COMMAND_LOGOUT, COMMAND_SET, 
    COMMAND_TENANT,
    PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_FORMAT, PARAMETER_ID, 
    PARAMETER_INPUT, PARAMETER_NAME, PARAMETER_OUTPUT, PARAMETER_PARENT_FOLDER_ID, 
    PARAMETER_PATH, PARAMETER_REFRESH, PARAMETER_TENANT, PARAMETER_UUID,
};
use pcli2::exit_codes::PcliExitCode;
use pcli2::model::{Asset, Folder};
use pcli2::auth::AuthClient;
use pcli2::configuration::Configuration;
use pcli2::folder_cache::FolderCache;
use pcli2::asset_cache::AssetCache;
use pcli2::folder_hierarchy::FolderHierarchy;
use pcli2::keyring::Keyring;
use pcli2::physna_v3::PhysnaApiClient;
use pcli2::format::{OutputFormat, OutputFormatter};
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use tracing::{debug, trace, error};

/// Error types that can occur during CLI command execution
#[derive(Debug, Error)]
pub enum CliError {
    /// Error when an unsupported or undefined subcommand is encountered
    #[error("Undefined or unsupported subcommand")]
    UnsupportedSubcommand(String),
    /// Error related to configuration loading or management
    #[error("Configuration error")]
    ConfigurationError(#[from] pcli2::configuration::ConfigurationError),
    /// Error related to data formatting
    #[error("Formatting error")]
    FormattingError(#[from] pcli2::format::FormattingError),
    /// Error related to security operations (authentication, keyring access)
    #[error("Security error")]
    SecurityError(String),
    /// Error when a required command-line argument is missing
    #[error("Missing required argument: {0}")]
    MissingRequiredArgument(String),
    /// Error related to JSON serialization/deserialization
    #[error("JSON serialization error")]
    JsonError(#[from] serde_json::Error),
}

impl CliError {
    /// Get the appropriate exit code for this error
    /// 
    /// Returns the corresponding `PcliExitCode` based on the error type:
    /// - `UsageError` for unsupported commands or missing arguments
    /// - `ConfigError` for configuration errors
    /// - `DataError` for formatting or JSON errors
    /// - `AuthError` for security-related errors
    pub fn exit_code(&self) -> PcliExitCode {
        match self {
            CliError::UnsupportedSubcommand(_) => PcliExitCode::UsageError,
            CliError::ConfigurationError(_) => PcliExitCode::ConfigError,
            CliError::FormattingError(_) => PcliExitCode::DataError,
            CliError::SecurityError(_) => PcliExitCode::AuthError,
            CliError::MissingRequiredArgument(_) => PcliExitCode::UsageError,
            CliError::JsonError(_) => PcliExitCode::DataError,
        }
    }
}

/// Extract the name of a subcommand from argument matches
/// 
/// # Arguments
/// 
/// * `sub_matches` - The argument matches for the subcommand
/// 
/// # Returns
/// 
/// The name of the subcommand as a String, or "unknown" if no subcommand is found
fn extract_subcommand_name(sub_matches: &ArgMatches) -> String {
    let message = match sub_matches.subcommand() {
        Some(m) => m.0,
        None => "unknown",
    };

    message.to_string()
}

/// Execute the parsed CLI command.
/// 
/// This is the main entry point for command execution. It takes the parsed command
/// arguments and executes the appropriate command logic based on the command structure.
/// 
/// # Arguments
/// 
/// * `configuration` - The application configuration
/// * `_api` - Placeholder for API client (currently unused as we use Physna V3 API directly)
/// 
/// # Returns
/// 
/// * `Ok(())` if the command executed successfully
/// * `Err(CliError)` if an error occurred during command execution
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
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            match client.list_tenants().await {
                                Ok(tenants) => {
                                    let format = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap();
                                    let format = OutputFormat::from_str(format).unwrap();
                                    
                                    match format {
                                        OutputFormat::Json => {
                                            // For JSON format, output a single array containing all tenants
                                            let json = serde_json::to_string_pretty(&tenants)?;
                                            println!("{}", json);
                                        }
                                        OutputFormat::Csv => {
                                            // For CSV format, output header and each tenant name on a separate line
                                            println!("TENANT_NAME");
                                            for tenant in tenants {
                                                println!("{}", tenant.tenant_display_name);
                                            }
                                        }
                                        OutputFormat::Tree => {
                                            // For tree format, output each tenant name on a separate line
                                            for tenant in tenants {
                                                println!("{}", tenant.tenant_display_name);
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
                    
                    // Check if refresh is requested
                    let refresh_requested = sub_matches.get_flag(PARAMETER_REFRESH);
                    
                    // Try to get access token and list folders from Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Build the folder hierarchy to get proper paths for all folders
                            let result = if refresh_requested {
                                trace!("Refresh requested, forcing API fetch");
                                FolderCache::refresh(&mut client, &tenant).await
                            } else {
                                trace!("Using cache or fetching from API");
                                FolderCache::get_or_fetch(&mut client, &tenant).await
                            };
                            
                            match result {
                                Ok(hierarchy) => {
                                    // If a path is specified, filter the hierarchy to show only that subtree
                                    let filtered_hierarchy = if let Some(path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                                        trace!("Filtering hierarchy by path: {}", path);
                                        hierarchy.filter_by_path(path).unwrap_or_else(|| {
                                            eprintln!("Warning: Folder path '{}' not found, showing full hierarchy", path);
                                            hierarchy
                                        })
                                    } else {
                                        hierarchy
                                    };
                                    
                                    // If tree format is requested, display the hierarchical tree structure
                                    if format == OutputFormat::Tree {
                                        filtered_hierarchy.print_tree();
                                        Ok(())
                                    } else {
                                        // For other formats (JSON, CSV), convert to folder list with paths
                                        let folder_list = filtered_hierarchy.to_folder_list();
                                        match folder_list.format(format) {
                                            Ok(output) => {
                                                println!("{}", output);
                                                Ok(())
                                            }
                                            Err(e) => Err(CliError::FormattingError(e)),
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error building folder hierarchy: {}", e);
                                    eprintln!("Error building folder hierarchy: {}", e);
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
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing folder get command");
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
                    
                    let folder_id_param = sub_matches.get_one::<String>(PARAMETER_ID);
                    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    // Must provide either ID or path
                    if folder_id_param.is_none() && folder_path_param.is_none() {
                        return Err(CliError::MissingRequiredArgument("Either folder ID or path must be provided".to_string()));
                    }
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                    let format = OutputFormat::from_str(&format_str).unwrap();
                    
                    // Try to get access token and get folder via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve folder ID from either ID parameter or path
                            let folder_id = if let Some(id) = folder_id_param {
                                id.clone()
                            } else if let Some(path) = folder_path_param {
                                // Build hierarchy and resolve path to folder ID
                                match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                    Ok(hierarchy) => {
                                        if let Some(folder_node) = hierarchy.get_folder_by_path(path) {
                                            folder_node.folder.id.clone()
                                        } else {
                                            return Err(CliError::MissingRequiredArgument(format!("Folder not found at path: {}", path)));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error building folder hierarchy: {}", e);
                                        return Err(CliError::ConfigurationError(pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory));
                                    }
                                }
                            } else {
                                // This shouldn't happen due to our earlier check, but just in case
                                return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
                            };
                            
                            match client.get_folder(&tenant, &folder_id).await {
                                Ok(folder_response) => {
                                    // Build hierarchy to get the path for this folder
                                    match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                        Ok(hierarchy) => {
                                            let path = hierarchy.get_path_for_folder(&folder_id).unwrap_or_else(|| folder_response.name.clone());
                                            let folder = Folder::from_folder_response(folder_response, path);
                                            match folder.format(format) {
                                                Ok(output) => {
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error building folder hierarchy: {}", e);
                                            // Fallback to folder without path
                                            let folder = Folder::from_folder_response(folder_response.clone(), folder_response.name.clone());
                                            match folder.format(format) {
                                                Ok(output) => {
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error fetching folder: {}", e);
                                    eprintln!("Error fetching folder: {}", e);
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
                Some((COMMAND_CREATE, sub_matches)) => {
                    trace!("Executing folder create command");
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
                    
                    let name = sub_matches.get_one::<String>(PARAMETER_NAME)
                        .ok_or(CliError::MissingRequiredArgument(PARAMETER_NAME.to_string()))?
                        .clone();
                        
                    let parent_folder_id_param = sub_matches.get_one::<String>(PARAMETER_PARENT_FOLDER_ID);
                    let parent_folder_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                    let format = OutputFormat::from_str(&format_str).unwrap();
                    
                    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
                    if parent_folder_id_param.is_some() && parent_folder_path_param.is_some() {
                        return Err(CliError::MissingRequiredArgument("Only one of --parent-folder-id or --path can be specified, not both".to_string()));
                    }
                    
                    // Try to get access token and create folder via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve parent folder ID from either ID parameter or path
                            let parent_folder_id = if let Some(id) = parent_folder_id_param {
                                Some(id.clone())
                            } else if let Some(path) = parent_folder_path_param {
                                // Build hierarchy and resolve path to folder ID
                                match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                    Ok(hierarchy) => {
                                        if let Some(folder_node) = hierarchy.get_folder_by_path(path) {
                                            Some(folder_node.folder.id.clone())
                                        } else {
                                            return Err(CliError::MissingRequiredArgument(format!("Parent folder not found at path: {}", path)));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error building folder hierarchy: {}", e);
                                        return Err(CliError::ConfigurationError(pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory));
                                    }
                                }
                            } else {
                                None
                            };
                            
                            match client.create_folder(&tenant, &name, parent_folder_id.as_deref()).await {
                                Ok(folder_response) => {
                                    // Build hierarchy to get the path for this new folder
                                    match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                        Ok(hierarchy) => {
                                            let path = if let Some(parent_id) = &parent_folder_id {
                                                if let Some(parent_path) = hierarchy.get_path_for_folder(parent_id) {
                                                    format!("{}/{}", parent_path, folder_response.name)
                                                } else {
                                                    folder_response.name.clone()
                                                }
                                            } else {
                                                folder_response.name.clone()
                                            };
                                            let folder = Folder::from_folder_response(folder_response, path);
                                            match folder.format(format) {
                                                Ok(output) => {
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error building folder hierarchy: {}", e);
                                            // Fallback to folder without path
                                            let folder = Folder::from_folder_response(folder_response.clone(), folder_response.name.clone());
                                            match folder.format(format) {
                                                Ok(output) => {
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error creating folder: {}", e);
                                    eprintln!("Error creating folder: {}", e);
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
                
                Some((COMMAND_DELETE, sub_matches)) => {
                    trace!("Executing folder delete command");
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
                    
                    let folder_id_param = sub_matches.get_one::<String>(PARAMETER_UUID);
                    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    // Must provide either ID or path
                    if folder_id_param.is_none() && folder_path_param.is_none() {
                        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
                    }
                    
                    // Try to get access token and delete folder via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve folder ID from either ID parameter or path
                            let folder_id = if let Some(id) = folder_id_param {
                                id.clone()
                            } else if let Some(path) = folder_path_param {
                                // Build hierarchy and resolve path to folder ID
                                match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                    Ok(hierarchy) => {
                                        if let Some(folder_node) = hierarchy.get_folder_by_path(path) {
                                            folder_node.folder.id.clone()
                                        } else {
                                            return Err(CliError::MissingRequiredArgument(format!("Folder not found at path: {}", path)));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error building folder hierarchy: {}", e);
                                        return Err(CliError::ConfigurationError(pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory));
                                    }
                                }
                            } else {
                                // This shouldn't happen due to our earlier check, but just in case
                                return Err(CliError::MissingRequiredArgument("Either folder ID or path must be provided".to_string()));
                            };
                            
                            match client.delete_folder(&tenant, &folder_id).await {
                                Ok(_) => {
                                    Ok(())
                                }
                                Err(e) => {
                                    error!("Error deleting folder: {}", e);
                                    eprintln!("Error deleting folder: {}", e);
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
        // Asset resource commands
        Some((COMMAND_ASSET, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_CREATE, sub_matches)) => {
                    trace!("Executing asset create command");
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
                    
                    let file_path = sub_matches.get_one::<PathBuf>("file")
                                .ok_or(CliError::MissingRequiredArgument("file".to_string()))?;
                                
                            // Extract filename from path for use in asset path construction
                            let file_name = file_path
                                .file_name()
                                .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file path".to_string()))?
                                .to_str()
                                .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file name".to_string()))?
                                .to_string();
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                    let format = OutputFormat::from_str(&format_str).unwrap();
                    
                    // Try to get access token and create asset via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Construct the full asset path by combining folder path with filename
                            let asset_path = if let Some(folder_path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                                if folder_path.is_empty() {
                                    file_name.clone()
                                } else {
                                    format!("{}/{}", folder_path, file_name)
                                }
                            } else {
                                // If no folder path specified, just use the filename
                                file_name.clone()
                            };
                            
                            debug!("Creating asset with path: {}", asset_path);
                            
                            match client.create_asset(&tenant, file_path.to_str().unwrap(), Some(&asset_path)).await {
                                Ok(asset_response) => {
                                    let asset = Asset::from_asset_response(asset_response, file_path.to_string_lossy().to_string());
                                    match asset.format(format) {
                                        Ok(output) => {
                                            println!("{}", output);
                                            Ok(())
                                        }
                                        Err(e) => Err(CliError::FormattingError(e)),
                                    }
                                }
                                Err(e) => {
                                    error!("Error creating asset: {}", e);
                                    eprintln!("Error creating asset: {}", e);
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
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing asset list command");
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
                    
                    // Validate format - only JSON and CSV are supported for assets
                    match format {
                        OutputFormat::Tree => {
                            eprintln!("Tree format is not supported for asset listing");
                            return Ok(());
                        }
                        _ => {} // JSON and CSV are supported
                    }
                    
                    // Check if refresh is requested
                    let refresh_requested = sub_matches.get_flag(PARAMETER_REFRESH);
                    
                    // Try to get access token and list assets from Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // If a path is specified, get assets filtered by folder path
                            let asset_list = if let Some(path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                                trace!("Getting assets for folder path: {}", path);
                                match AssetCache::get_assets_for_folder(&mut client, &tenant, path, refresh_requested).await {
                                    Ok(asset_list) => asset_list,
                                    Err(e) => {
                                        error!("Error getting assets for folder '{}': {}", path, e);
                                        eprintln!("Error getting assets for folder '{}': {}", path, e);
                                        return Ok(());
                                    }
                                }
                            } else {
                                // No path specified, get all assets for tenant
                                match if refresh_requested {
                                    trace!("Refresh requested, forcing API fetch for all assets");
                                    AssetCache::refresh(&mut client, &tenant).await
                                } else {
                                    trace!("Using cache or fetching from API for all assets");
                                    AssetCache::get_or_fetch(&mut client, &tenant).await
                                } {
                                    Ok(asset_list_response) => asset_list_response.to_asset_list(),
                                    Err(e) => {
                                        error!("Error fetching assets: {}", e);
                                        eprintln!("Error fetching assets: {}", e);
                                        return Ok(());
                                    }
                                }
                            };
                            
                            match asset_list.format(format) {
                                Ok(output) => {
                                    println!("{}", output);
                                    Ok(())
                                }
                                Err(e) => Err(CliError::FormattingError(e)),
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
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing asset get command");
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
                    
                    let asset_uuid_param = sub_matches.get_one::<String>(PARAMETER_UUID);
                    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    // Must provide either asset UUID or path
                    if asset_uuid_param.is_none() && asset_path_param.is_none() {
                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                    }
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                    let format = OutputFormat::from_str(&format_str).unwrap();
                    
                    // Try to get access token and get asset via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve asset ID from either UUID parameter or path
                            let asset_id = if let Some(uuid) = asset_uuid_param {
                                uuid.clone()
                            } else if let Some(path) = asset_path_param {
                                // To resolve asset by path, we need to:
                                // 1. Get all assets for the tenant
                                // 2. Find the asset with matching path
                                trace!("Resolving asset by path: {}", path);
                                match AssetCache::get_or_fetch(&mut client, &tenant).await {
                                    Ok(asset_list_response) => {
                                        // Find asset with matching path
                                        if let Some(asset_response) = asset_list_response.assets.iter().find(|asset| asset.path == *path) {
                                            asset_response.id.clone()
                                        } else {
                                            return Err(CliError::MissingRequiredArgument(format!("Asset with path '{}' not found", path)));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error fetching assets for path resolution: {}", e);
                                        return Err(CliError::MissingRequiredArgument("Failed to fetch assets for path resolution".to_string()));
                                    }
                                }
                            } else {
                                // This shouldn't happen due to our earlier check, but just in case
                                return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                            };
                            
                            let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                            let _format = OutputFormat::from_str(&format_str).unwrap();
                            
                            match client.get_asset(&tenant, &asset_id).await {
                                Ok(asset_response) => {
                                    let asset = Asset::from_asset_response(asset_response, asset_id.clone());
                                    match asset.format(format) {
                                        Ok(output) => {
                                            println!("{}", output);
                                            Ok(())
                                        }
                                        Err(e) => Err(CliError::FormattingError(e)),
                                    }
                                }
                                Err(e) => {
                                    error!("Error fetching asset: {}", e);
                                    eprintln!("Error fetching asset: {}", e);
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
                Some((COMMAND_DELETE, sub_matches)) => {
                    trace!("Executing asset delete command");
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
                    
                    let asset_uuid_param = sub_matches.get_one::<String>(PARAMETER_UUID);
                    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    // Must provide either asset UUID or path
                    if asset_uuid_param.is_none() && asset_path_param.is_none() {
                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                    }
                    
                    // Try to get access token and delete asset via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get(&"default".to_string(), "client-id".to_string()),
                                keyring.get(&"default".to_string(), "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve asset ID from either UUID parameter or path
                            let asset_id = if let Some(uuid) = asset_uuid_param {
                                uuid.clone()
                            } else if let Some(path) = asset_path_param {
                                // To resolve asset by path, we need to:
                                // 1. Get all assets for the tenant
                                // 2. Find the asset with matching path
                                trace!("Resolving asset by path: {}", path);
                                match AssetCache::get_or_fetch(&mut client, &tenant).await {
                                    Ok(asset_list_response) => {
                                        // Find asset with matching path
                                        if let Some(asset_response) = asset_list_response.assets.iter().find(|asset| asset.path == *path) {
                                            asset_response.id.clone()
                                        } else {
                                            return Err(CliError::MissingRequiredArgument(format!("Asset with path '{}' not found", path)));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error fetching assets for path resolution: {}", e);
                                        return Err(CliError::MissingRequiredArgument("Failed to fetch assets for path resolution".to_string()));
                                    }
                                }
                            } else {
                                // This shouldn't happen due to our earlier check, but just in case
                                return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                            };
                            
                            match client.delete_asset(&tenant, &asset_id).await {
                                Ok(()) => {
                                    println!("Asset deleted successfully");
                                    Ok(())
                                }
                                Err(e) => {
                                    error!("Error deleting asset: {}", e);
                                    eprintln!("Error deleting asset: {}", e);
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

        // Authentication commands
        Some((COMMAND_AUTH, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LOGIN, sub_matches)) => {
                    trace!("Executing login command");
                    
                    // Try to get client credentials from command line or stored values
                    let mut keyring = Keyring::default();
                    let client_id = match sub_matches.get_one::<String>(PARAMETER_CLIENT_ID) {
                        Some(id) => id.clone(),
                        None => {
                            // Try to get stored client ID
                            match keyring.get(&"default".to_string(), "client-id".to_string()) {
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
                            match keyring.get(&"default".to_string(), "client-secret".to_string()) {
                                Ok(Some(stored_secret)) => stored_secret,
                                _ => {
                                    return Err(CliError::MissingRequiredArgument(PARAMETER_CLIENT_SECRET.to_string()));
                                }
                            }
                        }
                    };
                    
                    let auth_client = AuthClient::new(client_id.clone(), client_secret.clone());
                    
                    // Store the client credentials so they're available for token refresh
                    let client_id_result = keyring.put(&"default".to_string(), "client-id".to_string(), client_id.clone());
                    let client_secret_result = keyring.put(&"default".to_string(), "client-secret".to_string(), client_secret.clone());
                    
                    if client_id_result.is_err() || client_secret_result.is_err() {
                        eprintln!("Error storing client credentials");
                        return Err(CliError::SecurityError(String::from("Failed to store client credentials")));
                    }
                    
                    match auth_client.get_access_token().await {
                        Ok(token) => {
                            // Store the access token
                            let token_result = keyring.put(&"default".to_string(), "access-token".to_string(), token);
                            
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
                    match keyring.delete(&"default".to_string(), "access-token".to_string()) {
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
                    match keyring.get(&"default".to_string(), "access-token".to_string()) {
                        Ok(Some(token)) => {
                            // Output the token based on the requested format
                            match format {
                                OutputFormat::Json => {
                                    println!("{{\"access_token\": \"{}\"}}", token);
                                }
                                OutputFormat::Csv => {
                                    println!("ACCESS_TOKEN\n{}", token);
                                }
                                OutputFormat::Tree => {
                                    // For tree format, just output the token value
                                    println!("{}", token);
                                }
                            }
                            Ok(())
                        }
                        Ok(None) => {
                            eprintln!("No access token found. Please login first.");
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
        // Context commands
        Some((COMMAND_CONTEXT, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_SET, sub_matches)) => {
                    match sub_matches.subcommand() {
                        Some(("tenant", sub_matches)) => {
                            trace!("Executing context set tenant command");
                            let name = sub_matches.get_one::<String>(PARAMETER_NAME);
                            
                            // Try to get access token and fetch tenant info from Physna V3 API
                            let mut keyring = Keyring::default();
                            match keyring.get(&"default".to_string(), "access-token".to_string()) {
                                Ok(Some(token)) => {
                                    let mut client = PhysnaApiClient::new().with_access_token(token);
                                    
                                    // Try to get client credentials for automatic token refresh
                                    if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                        keyring.get(&"default".to_string(), "client-id".to_string()),
                                        keyring.get(&"default".to_string(), "client-secret".to_string())
                                    ) {
                                        client = client.with_client_credentials(client_id, client_secret);
                                    }
                                    
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
                                    println!("{{\"active_tenant\": {{\"name\": \"{}\"}}}}", tenant_name);
                                }
                                OutputFormat::Csv => {
                                    println!("ACTIVE_TENANT_NAME
{}", tenant_name);
                                }
                                OutputFormat::Tree => {
                                    println!("Active tenant: {}", tenant_name);
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
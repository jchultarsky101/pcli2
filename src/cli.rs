//! CLI command execution logic.
//!
//! This module contains the core logic for executing CLI commands parsed by the
//! command definition module. It handles the execution of all supported commands
//! including tenant, folder, asset, authentication, context, and configuration operations.

use clap::ArgMatches;
use futures::StreamExt;
use inquire::Select;
use pcli2::commands::{
    create_cli_commands, COMMAND_ASSET, COMMAND_AUTH, COMMAND_CACHE, COMMAND_CLEAR, COMMAND_CONFIG, COMMAND_CONTEXT, 
    COMMAND_CREATE, COMMAND_CREATE_BATCH, COMMAND_DELETE, COMMAND_EXPORT, COMMAND_FOLDER, COMMAND_GET, 
    COMMAND_IMPORT, COMMAND_LIST, COMMAND_LOGIN, COMMAND_LOGOUT, COMMAND_SET, 
    COMMAND_TENANT, COMMAND_MATCH, COMMAND_METADATA,
    PARAMETER_CLIENT_ID, PARAMETER_CLIENT_SECRET, PARAMETER_FORMAT, 
    PARAMETER_INPUT, PARAMETER_NAME, PARAMETER_OUTPUT, PARAMETER_PARENT_FOLDER_ID, 
    PARAMETER_PATH, PARAMETER_REFRESH, PARAMETER_TENANT, PARAMETER_UUID,
};
use pcli2::error_utils;
use pcli2::exit_codes::PcliExitCode;
use pcli2::model::{Asset, Folder, FolderGeometricMatch, FolderGeometricMatchResponse, FolderList};
use pcli2::auth::AuthClient;
use pcli2::physna_v3::ApiError;
use std::time::Duration;
use tokio::time::sleep;
use pcli2::configuration::Configuration;
use pcli2::folder_cache::FolderCache;
use pcli2::asset_cache::AssetCache;
use pcli2::folder_hierarchy::FolderHierarchy;
use pcli2::keyring::Keyring;
use pcli2::physna_v3::PhysnaApiClient;


use pcli2::format::{OutputFormat, OutputFormatter};

/// Resolve a tenant name or ID to a tenant ID
/// 
/// This function handles the case where users provide either a tenant name or ID
/// via the --tenant parameter. It checks if the provided identifier looks like
/// a UUID (tenant ID) or a human-readable name, and resolves names to IDs by
/// calling the list_tenants API endpoint.
/// 
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant_identifier` - The tenant name or ID to resolve
/// 
/// # Returns
/// * `Ok(String)` - The resolved tenant ID
/// * `Err(CliError)` - If the tenant cannot be found
async fn resolve_tenant_identifier_to_id(
    client: &mut PhysnaApiClient,
    tenant_identifier: String,
) -> Result<String, CliError> {
    debug!("Resolving tenant identifier: {}", tenant_identifier);
    
    // First, try to list all tenants to see if we can resolve the identifier
    let tenants = client.list_tenants().await.map_err(|e| {
        CliError::ApiError {
            context: "Failed to list tenants during resolution".to_string(),
            source: Box::new(e),
        }
    })?;
    
    // Look for an exact match by tenant ID first
    for tenant in &tenants {
        if tenant.tenant_id == tenant_identifier {
            debug!("Tenant identifier {} appears to be a direct ID match", tenant_identifier);
            return Ok(tenant.tenant_id.clone());
        }
    }
    
    // Then look for a match by name
    for tenant in &tenants {
        if tenant.tenant_display_name == tenant_identifier || 
           tenant.tenant_short_name.as_str() == tenant_identifier {
            debug!("Resolved tenant identifier '{}' to ID '{}'", tenant_identifier, tenant.tenant_id);
            return Ok(tenant.tenant_id.clone());
        }
    }
    
    // If we can't find the tenant, return an error
    Err(CliError::TenantNotFound {
        identifier: tenant_identifier,
    })
}

/// Helper function to safely extract and parse format parameter with default fallback
fn extract_format_param_with_default(sub_matches: &ArgMatches, param_name: &str) -> Result<OutputFormat, CliError> {
    let format_str = sub_matches.get_one::<String>(param_name)
        .ok_or_else(|| CliError::MissingRequiredArgument(param_name.to_string()))
        .map(|s| s.clone())
        .unwrap_or_else(|_| "json".to_string());
    
    OutputFormat::from_str(&format_str)
        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_str)))
}

/// Helper function to get tenant from parameter or configuration with resolution
async fn get_tenant_id(
    client: &mut PhysnaApiClient,
    sub_matches: &ArgMatches,
    configuration: &Configuration,
) -> Result<String, CliError> {
    let tenant_identifier = match sub_matches.get_one::<String>(PARAMETER_TENANT) {
        Some(tenant_id) => tenant_id.clone(),
        None => {
            if let Some(active_tenant_id) = configuration.get_active_tenant_id() {
                active_tenant_id
            } else {
                return Err(CliError::MissingRequiredArgument(PARAMETER_TENANT.to_string()));
            }
        }
    };
    
    resolve_tenant_identifier_to_id(client, tenant_identifier).await
}

use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use tracing::{debug, trace, error, warn};

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
    /// Error when a tenant cannot be found by name or ID
    #[error("Tenant '{identifier}' not found")]
    TenantNotFound { identifier: String },
    /// Error when a folder cannot be found by path or ID
    #[error("Folder '{identifier}' not found")]
    FolderNotFound { identifier: String },
    /// Error when an API call fails
    #[error("API error: {context}")]
    ApiError { context: String, source: Box<dyn std::error::Error + Send + Sync> },
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
            CliError::TenantNotFound { .. } => PcliExitCode::UsageError,
            CliError::FolderNotFound { .. } => PcliExitCode::UsageError,
            CliError::ApiError { .. } => PcliExitCode::DataError,
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
    trace!("Verbose mode enabled: {}", commands.get_flag("verbose"));

    match commands.subcommand() {
        // Tenant resource commands
        Some((COMMAND_TENANT, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing tenant list command");
                    // Try to get access token and list tenants from Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            match client.list_tenants().await {
                                Ok(tenants) => {
                                    let format_param = sub_matches.get_one::<String>(PARAMETER_FORMAT)
                                        .ok_or_else(|| CliError::MissingRequiredArgument(PARAMETER_FORMAT.to_string()))?;
                                    let format = OutputFormat::from_str(format_param)
                                        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_param)))?;
                                    
                                    match format {
                                        OutputFormat::Json => {
                                            // For JSON format, output a single array containing all tenants
                                            let json = serde_json::to_string_pretty(&tenants)?;
                                            println!("{}", json);
                                        }
                                        OutputFormat::Csv => {
                                            // For CSV format, output header with both tenant name and UUID columns
                                            println!("TENANT_NAME,TENANT_UUID");
                                            for tenant in tenants {
                                                println!("{},{}", tenant.tenant_display_name, tenant.tenant_id);
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
                                    error_utils::report_error(&e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
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
        // Folder resource commands
        Some((COMMAND_FOLDER, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing folder list command");
                    
                    let format_param = sub_matches.get_one::<String>(PARAMETER_FORMAT)
                        .ok_or_else(|| CliError::MissingRequiredArgument(PARAMETER_FORMAT.to_string()))?;
                    let format = OutputFormat::from_str(format_param)
                        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_param)))?;
                    
                    // Check if refresh is requested
                    let refresh_requested = sub_matches.get_flag(PARAMETER_REFRESH);
                    
                    // Try to get access token and list folders from Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Get tenant ID with resolution
                            let tenant = get_tenant_id(&mut client, sub_matches, &configuration).await?;
                            
                            // Check if a specific path is provided
                            if let Some(path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                                trace!("Listing folders for specific path: {} (refresh: {})", path, refresh_requested);
                                
                                if refresh_requested {
                                    // If refresh requested, clear the cache for this tenant
                                    if let Err(e) = FolderCache::invalidate(&tenant) {
                                        debug!("Failed to invalidate folder cache: {}", e);
                                    }
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
                                            match hierarchy.filter_by_path(path) {
                                                Some(filtered_hierarchy) => filtered_hierarchy,
                                                None => {
                                                    eprintln!("Error: Folder path '{}' not found", path);
                                                    return Err(CliError::FolderNotFound { identifier: path.to_string() });
                                                }
                                            }
                                        } else {
                                            hierarchy
                                        };
                                        
                                        // If tree format is requested, display the hierarchical tree structure
                                        if format == OutputFormat::Tree {
                                            filtered_hierarchy.print_tree();
                                            Ok(())
                                        } else {
                                            // For other formats (JSON, CSV), check if recursive is requested
                                            let recursive_requested = sub_matches.get_flag("recursive");
                                            
                                            // Convert to folder list with only direct children if not recursive
                                            let folder_list = if recursive_requested {
                                                // Recursive - show all folders in the hierarchy (existing behavior)
                                                filtered_hierarchy.to_folder_list()
                                            } else {
                                                // Non-recursive - show only direct children of the specified path
                                                // In the filtered hierarchy, the root folders are the ones we want to show
                                                // But we need to get their actual children, not the folders themselves
                                                let mut direct_children_list = FolderList::empty();
                                                
                                                // For each root node in the filtered hierarchy, get its actual children
                                                for root_id in &filtered_hierarchy.root_ids {
                                                    if let Some(root_node) = filtered_hierarchy.nodes.get(root_id) {
                                                        // Get the actual children of this root node
                                                        for child_id in &root_node.children {
                                                            if let Some(child_node) = filtered_hierarchy.nodes.get(child_id) {
                                                                let child_path = filtered_hierarchy.get_path_for_folder(child_id).unwrap_or_else(|| child_node.name().to_string());
                                                                let child_folder = Folder::from_folder_response(child_node.folder.clone(), child_path);
                                                                direct_children_list.insert(child_folder);
                                                            }
                                                        }
                                                    }
                                                }
                                                direct_children_list
                                            };
                                            
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
                                        error_utils::report_error(&e);
                                        Ok(())
                                    }
                                }
                            } else {
                                // No specific path - list all root folders (still need hierarchy for full listing)
                                trace!("Listing all folders for tenant: {} (refresh: {})", &tenant, refresh_requested);
                                
                                let result = if refresh_requested {
                                    trace!("Refresh requested, forcing API fetch");
                                    FolderCache::refresh(&mut client, &tenant).await
                                } else {
                                    trace!("Using cache or fetching from API");
                                    FolderCache::get_or_fetch(&mut client, &tenant).await
                                };
                                
                                match result {
                                    Ok(hierarchy) => {
                                        // If tree format is requested, display the hierarchical tree structure
                                        if format == OutputFormat::Tree {
                                            hierarchy.print_tree();
                                            Ok(())
                                        } else {
                                            // For other formats (JSON, CSV), convert to folder list with paths
                                            let folder_list = hierarchy.to_folder_list();
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
                                        error_utils::report_error(&e);
                                        Ok(())
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&e);
                            Ok(())
                        }
                    }
                }
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing folder get command");
                    
                    let folder_id_param = sub_matches.get_one::<String>(PARAMETER_UUID);
                    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    // Must provide either UUID or path
                    if folder_id_param.is_none() && folder_path_param.is_none() {
                        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
                    }
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT)
                        .ok_or_else(|| CliError::MissingRequiredArgument(PARAMETER_FORMAT.to_string()))
                        .map(|s| s.clone())
                        .unwrap_or_else(|_| "json".to_string());
                    let format = OutputFormat::from_str(&format_str)
                        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_str)))?;
                    
                    // Try to get access token and get folder via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Get tenant ID with resolution
                            let tenant = get_tenant_id(&mut client, sub_matches, &configuration).await?;
                            
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
                                            return Err(CliError::FolderNotFound { identifier: path.to_string() });
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
                                Ok(single_folder_response) => {
                                    // Build hierarchy to get the path for this folder
                                    match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                        Ok(hierarchy) => {
                                            let path = hierarchy.get_path_for_folder(&folder_id).unwrap_or_else(|| single_folder_response.folder.name.clone());
                                            let folder = Folder::from_folder_response(single_folder_response.folder, path);
                                            // Persist the potentially updated access token back to keyring
                                            if let Some(updated_token) = client.get_access_token() {
                                                if let Err(e) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                    warn!("Failed to persist updated access token: {}", e);
                                                }
                                            }
                                            
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
                                            let folder = Folder::from_folder_response(single_folder_response.folder.clone(), single_folder_response.folder.name.clone());
                                            // Persist the potentially updated access token back to keyring
                                            if let Some(updated_token) = client.get_access_token() {
                                                if let Err(e) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                    warn!("Failed to persist updated access token: {}", e);
                                                }
                                            }
                                            
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
                                    error_utils::report_error(&e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
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
                    
                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT)
                        .ok_or_else(|| CliError::MissingRequiredArgument(PARAMETER_FORMAT.to_string()))
                        .map(|s| s.clone())
                        .unwrap_or_else(|_| "json".to_string());
                    let format = OutputFormat::from_str(&format_str)
                        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_str)))?;

                    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
                    if parent_folder_id_param.is_some() && parent_folder_path_param.is_some() {
                        return Err(CliError::MissingRequiredArgument("Only one of --parent-folder-id or --path can be specified, not both".to_string()));
                    }
                    
                    // Try to get access token and create folder via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
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
                                    // Invalidate folder cache for this tenant since we've modified folder state
                                    if let Err(e) = FolderCache::invalidate(&tenant) {
                                        debug!("Failed to invalidate folder cache: {}", e);
                                    }
                                    
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
                                    error_utils::report_error(&e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
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
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
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
                                            return Err(CliError::FolderNotFound { identifier: path.to_string() });
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
                                    // Invalidate folder cache for this tenant since we've modified folder state
                                    if let Err(e) = FolderCache::invalidate(&tenant) {
                                        debug!("Failed to invalidate folder cache: {}", e);
                                    }
                                    
                                    Ok(())
                                }
                                Err(e) => {
                                    error!("Error deleting folder: {}", e);
                                    error_utils::report_error(&CliError::ConfigurationError(pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory));
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
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
        // Asset commands
        Some((COMMAND_ASSET, sub_matches)) => {
            match sub_matches.subcommand() {
                Some((COMMAND_MATCH, sub_matches)) => {
                    trace!("Executing asset match command");
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
                    
                    // Get the reference asset identifier (either UUID or path)
                    let asset_id = if let Some(uuid) = sub_matches.get_one::<String>(PARAMETER_UUID) {
                        uuid.clone()
                    } else if let Some(path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                        debug!("Looking up asset by path: {}", path);
                        // We need to look up the asset by path to get its UUID
                        // Try to get access token
                        let mut keyring = Keyring::default();
                        match keyring.get("default", "access-token".to_string()) {
                            Ok(Some(token)) => {
                                let mut client = PhysnaApiClient::new().with_access_token(token);
                                
                                // Try to get client credentials for automatic token refresh
                                if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                    keyring.get("default", "client-id".to_string()),
                                    keyring.get("default", "client-secret".to_string())
                                ) {
                                    client = client.with_client_credentials(client_id, client_secret);
                                }
                                
                                // Get asset cache or fetch assets from API
                                match AssetCache::get_or_fetch(&mut client, &tenant).await {
                                    Ok(asset_list_response) => {
                                        // Convert to AssetList to use find_by_path
                                        let asset_list = asset_list_response.to_asset_list();
                                        // Find the asset by path
                                        if let Some(asset) = asset_list.find_by_path(path) {
                                            if let Some(uuid) = asset.uuid() {
                                                trace!("Found asset with UUID: {}", uuid);
                                                uuid.clone()
                                            } else {
                                                error_utils::report_error(&CliError::MissingRequiredArgument(format!("Asset found by path '{}' but has no UUID", path)));
                                                return Ok(());
                                            }
                                        } else {
                                            eprintln!("Asset not found by path '{}'", path);
                                            return Ok(());
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error fetching asset cache: {}", e);
                                        eprintln!("Error fetching asset cache: {}", e);
                                        return Ok(());
                                    }
                                }
                            }
                            Ok(None) => {
                                error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                                return Ok(());
                            }
                            Err(e) => {
                                error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                                return Ok(());
                            }
                        }
                    } else {
                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                    };
                    
                    // Get threshold parameter with proper error handling
                    let threshold_param = sub_matches.get_one::<f64>("threshold")
                        .unwrap_or(&80.0);
                    let threshold = *threshold_param;
                    
                    // Validate threshold is between 0 and 100
                    if !(0.0..=100.0).contains(&threshold) {
                        eprintln!("Threshold must be between 0.00 and 100.00");
                        return Ok(());
                    }

                    let format = extract_format_param_with_default(sub_matches, PARAMETER_FORMAT)?;

                    trace!("Performing geometric search for asset {} with threshold {}", asset_id, threshold);
                    
                    // Try to get access token and perform geometric search
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Try the geometric search with retry logic for 409 errors
                            let mut retry_count = 0;
                            let max_retries = 3;
                            let search_result = loop {
                                match client.geometric_search(&tenant, &asset_id, threshold).await {
                                    Ok(result) => break Ok(result),
                                    Err(e) => {
                                        // Check if it's a 409 Conflict error and we should retry
                                        if let ApiError::HttpError(http_err) = &e {
                                            if http_err.status() == Some(reqwest::StatusCode::CONFLICT) && retry_count < max_retries {
                                                retry_count += 1;
                                                trace!("Received 409 Conflict for asset {}, retry {} after 500ms delay", asset_id, retry_count);
                                                sleep(Duration::from_millis(500)).await;
                                                continue;
                                            }
                                        }
                                        // For all other errors or if we've exhausted retries, break with the error
                                        break Err(e);
                                    }
                                }
                            };
                            
                            match search_result {
                                Ok(search_result) => {
                                    trace!("Geometric search completed, processing {} matches", search_result.matches.len());
                                    // Get the reference asset details
                                    match client.get_asset(&tenant, &asset_id).await {
                                        Ok(reference_asset) => {
                                            trace!("Retrieved reference asset details");
                                            // Convert GeometricSearchResponse to FolderGeometricMatchResponse format
                                            let mut matches = Vec::new();
                                            
                                            // Extract the reference asset name from the path (last part after the last slash)
                                            let reference_asset_name = reference_asset.path.split('/').next_back().unwrap_or(&reference_asset.path).to_string();
                                            trace!("Reference asset name: {}", reference_asset_name);
                                            
                                            // Convert each geometric match to a folder match format
                                            for (index, geometric_match) in search_result.matches.iter().enumerate() {
                                                trace!("Processing match {} of {}: {} -> {}", index + 1, search_result.matches.len(), asset_id, geometric_match.asset.id);
                                                // Skip self-matches
                                                if geometric_match.asset.id != asset_id {
                                                    let candidate_asset_name = geometric_match.asset.path.split('/').next_back().unwrap_or(&geometric_match.asset.path).to_string();
                                                    trace!("Adding match: {} -> {} ({}%)", reference_asset_name, candidate_asset_name, geometric_match.match_percentage);
                                                    // Generate comparison URL for single asset match
                                                    let comparison_url = format!("https://app.physna.com/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                                                        tenant,
                                                        asset_id,
                                                        geometric_match.asset.id,
                                                        tenant,
                                                        tenant,
                                                        geometric_match.match_percentage
                                                    );
                                                    
                                                    let folder_match = FolderGeometricMatch {
                                                        reference_asset_name: reference_asset_name.clone(),
                                                        candidate_asset_name,
                                                        match_percentage: geometric_match.match_percentage,
                                                        reference_asset_path: reference_asset.path.clone(),
                                                        candidate_asset_path: geometric_match.asset.path.clone(),
                                                        reference_asset_uuid: asset_id.clone(),
                                                        candidate_asset_uuid: geometric_match.asset.id.clone(),
                                                        comparison_url,
                                                    };
                                                    matches.push(folder_match);
                                                } else {
                                                    trace!("Skipping self-match for asset {}", asset_id);
                                                }
                                            }
                                            
                                            trace!("Formatting {} matches for output", matches.len());
                                            // Create the response object (now a simple vector)
                                            let folder_match_response: FolderGeometricMatchResponse = matches;
                                            
                                            // Format and output the results
                                            match folder_match_response.format(format) {
                                                Ok(output) => {
                                                    trace!("Output formatted successfully");
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error getting reference asset details: {}", e);
                                            Ok(())
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error performing geometric search for asset {} after {} retries: {}", asset_id, retry_count, e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
                Some(("geometric-match-folder", sub_matches)) => {
                    trace!("Executing asset geometric-match-folder command");
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

                    // Get the source folder path
                    let folder_path = sub_matches.get_one::<String>(PARAMETER_PATH)
                        .ok_or(CliError::MissingRequiredArgument("folder path must be provided".to_string()))?
                        .clone();
                    trace!("Processing folder: {}", folder_path);

                    // Get threshold parameter with proper error handling
                    let threshold_param = sub_matches.get_one::<f64>("threshold")
                        .unwrap_or(&80.0);
                    let threshold = *threshold_param;

                    // Validate threshold is between 0 and 100
                    if !(0.0..=100.0).contains(&threshold) {
                        eprintln!("Threshold must be between 0.00 and 100.00");
                        return Ok(());
                    }

                    // Get concurrency parameter
                    let concurrent_param = sub_matches.get_one::<usize>("concurrent")
                        .unwrap_or(&5);
                    let concurrent = *concurrent_param;
                    trace!("Using concurrency level: {}", concurrent);

                    // Get progress parameter
                    let show_progress = sub_matches.get_flag("progress");
                    trace!("Progress bar enabled: {}", show_progress);

                    let format = extract_format_param_with_default(sub_matches, PARAMETER_FORMAT)?;

                    trace!("Retrieving access token for tenant: {}", tenant);
                    // Try to get access token and perform folder-based geometric search
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            // Get client credentials for creating multiple clients
                            let client_credentials = if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                Some((client_id, client_secret))
                            } else {
                                None
                            };

                            trace!("Fetching assets for folder: {}", folder_path);
                            // Create a client for initial operations
                            let mut client = PhysnaApiClient::new().with_access_token(token.clone());
                            if let Some((client_id, client_secret)) = &client_credentials {
                                client = client.with_client_credentials(client_id.clone(), client_secret.clone());
                            }

                            // Get all assets in the specified folder
                            match AssetCache::get_assets_for_folder(&mut client, &tenant, &folder_path, false).await {
                                Ok(asset_list) => {
                                    trace!("Found {} assets in folder", asset_list.len());
                                    
                                    // Get all assets from the AssetList
                                    let assets = asset_list.get_all_assets();
                                    trace!("Processing {} assets", assets.len());

                                    // Create progress bar if requested
                                    let progress_bar = if show_progress {
                                        let pb = indicatif::ProgressBar::new(assets.len() as u64);
                                        pb.set_style(
                                            indicatif::ProgressStyle::default_bar()
                                                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                                                .unwrap()
                                                .progress_chars("#>-")
                                        );
                                        Some(pb)
                                    } else {
                                        None
                                    };

                                    // Create a stream of futures for processing assets concurrently
                                    let base_url = "https://app-api.physna.com/v3".to_string(); // Use default base URL
                                    let tenant_id = tenant.clone();
                                    
                                    let results: Result<Vec<_>, _> = futures::stream::iter(assets)
                                        .map(|asset| {
                                            let base_url = base_url.clone();
                                            let tenant_id = tenant_id.clone();
                                            let token = token.clone();
                                            let client_credentials = client_credentials.clone();
                                            let progress_bar = progress_bar.clone();
                                            let asset_name = asset.name().to_string();
                                            let asset_uuid = asset.uuid().cloned();
                                            let asset_path = asset.path().to_string();

                                            async move {
                                                trace!("Processing asset: {} ({})", asset_name, asset_uuid.as_deref().unwrap_or("unknown"));
                                                
                                                if let Some(asset_uuid) = asset_uuid {
                                                    // Create a new client for each request to avoid borrowing issues
                                                    let mut client = PhysnaApiClient::new().with_base_url(base_url).with_access_token(token.clone());
                                                    if let Some((client_id, client_secret)) = client_credentials.clone() {
                                                        client = client.with_client_credentials(client_id, client_secret);
                                                    }

                                                    trace!("Performing geometric search for asset: {} ({})", asset_name, asset_uuid);
                                                    // Try the geometric search with retry logic for 409 errors
                                                    let mut retry_count = 0;
                                                    let max_retries = 3;
                                                    let search_result = loop {
                                                        match client.geometric_search(&tenant_id, &asset_uuid, threshold).await {
                                                            Ok(result) => break Ok(result),
                                                            Err(e) => {
                                                                // Check if it's a 409 Conflict error and we should retry
                                                                if let ApiError::HttpError(http_err) = &e {
                                                                    if http_err.status() == Some(reqwest::StatusCode::CONFLICT) && retry_count < max_retries {
                                                                        retry_count += 1;
                                                                        trace!("Received 409 Conflict for asset {}, retry {} after 500ms delay", asset_uuid, retry_count);
                                                                        sleep(Duration::from_millis(500)).await;
                                                                        continue;
                                                                    }
                                                                }
                                                                // For all other errors or if we've exhausted retries, break with the error
                                                                break Err(e);
                                                            }
                                                        }
                                                    };
                                                    
                                                    match search_result {
                                                        Ok(search_result) => {
                                                            trace!("Geometric search completed for {}, found {} matches", asset_uuid, search_result.matches.len());
                                                            
                                                            // Process matches, skipping self-matches
                                                            let mut asset_matches = Vec::new();
                                                            for geometric_match in search_result.matches {
                                                                // Skip self-matches by comparing UUIDs
                                                                if geometric_match.asset.id != asset_uuid {
                                                                    let candidate_asset_name = geometric_match.asset.path.split('/').next_back().unwrap_or(&geometric_match.asset.path).to_string();
                                                                    trace!("Adding match: {} -> {} ({}%)", asset_name, candidate_asset_name, geometric_match.match_percentage);
                                                                    // Generate comparison URL
                                                                    let comparison_url = format!("https://app.physna.com/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                                                                        tenant_id,
                                                                        asset_uuid,
                                                                        geometric_match.asset.id,
                                                                        tenant_id,
                                                                        tenant_id,
                                                                        geometric_match.match_percentage
                                                                    );
                                                                    
                                                                    let folder_match = FolderGeometricMatch {
                                                                        reference_asset_name: asset_name.clone(),
                                                                        candidate_asset_name,
                                                                        match_percentage: geometric_match.match_percentage,
                                                                        reference_asset_path: asset_path.clone(),
                                                                        candidate_asset_path: geometric_match.asset.path.clone(),
                                                                        reference_asset_uuid: asset_uuid.clone(),
                                                                        candidate_asset_uuid: geometric_match.asset.id.clone(),
                                                                        comparison_url,
                                                                    };
                                                                    asset_matches.push(folder_match);
                                                                } else {
                                                                    trace!("Skipping self-match for asset {}", asset_uuid);
                                                                }
                                                            }
                                                            
                                                            // Update progress bar if present
                                                            if let Some(pb) = &progress_bar {
                                                                pb.inc(1);
                                                                pb.set_message(format!("Processed: {}", asset_name));
                                                            }
                                                            
                                                            Ok(asset_matches)
                                                        }
                                                        Err(e) => {
                                                            error!("Error performing geometric search for asset {} after {} retries: {}", asset_uuid, retry_count, e);
                                                            
                                                            // Update progress bar if present
                                                            if let Some(pb) = &progress_bar {
                                                                pb.inc(1);
                                                                pb.set_message(format!("Failed: {}", asset_name));
                                                            }
                                                            
                                                            Err(e)
                                                        }
                                                    }
                                                } else {
                                                    // Update progress bar if present
                                                    if let Some(pb) = &progress_bar {
                                                        pb.inc(1);
                                                        pb.set_message(format!("Skipped: {} (no UUID)", asset_name));
                                                    }
                                                    
                                                    Err(ApiError::AuthError("Asset has no UUID".to_string()))
                                                }
                                            }
                                        })
                                        .buffer_unordered(concurrent)
                                        .collect::<Vec<_>>()
                                        .await
                                        .into_iter()
                                        .collect();

                                    // Finish progress bar if present
                                    if let Some(pb) = progress_bar {
                                        pb.finish_with_message("Batch processing complete");
                                    }

                                    match results {
                                        Ok(asset_match_results) => {
                                            // Flatten all matches into a single vector
                                            let all_matches: Vec<FolderGeometricMatch> = asset_match_results.into_iter().flatten().collect();
                                            trace!("Processed all assets, found {} total matches", all_matches.len());

                                            // Filter out duplicate pairs (A->B and B->A are the same match)
                                            let mut unique_matches = Vec::new();
                                            let mut seen_pairs = std::collections::HashSet::new();

                                            for match_result in all_matches {
                                                // Create a canonical pair identifier by sorting the UUIDs
                                                let mut pair = vec![match_result.reference_asset_uuid.clone(), match_result.candidate_asset_uuid.clone()];
                                                pair.sort();
                                                let pair_key = format!("{}-{}", pair[0], pair[1]);

                                                if !seen_pairs.contains(&pair_key) {
                                                    seen_pairs.insert(pair_key);
                                                    unique_matches.push(match_result);
                                                }
                                            }

                                            // Sort the unique matches by match percentage (descending), then by reference asset path (ascending)
                                            unique_matches.sort_by(|a, b| {
                                                // First compare by match percentage (descending)
                                                b.match_percentage
                                                    .partial_cmp(&a.match_percentage)
                                                    .unwrap_or(std::cmp::Ordering::Equal)
                                                    .then_with(|| {
                                                        // Then by reference asset path (ascending)
                                                        a.reference_asset_path.cmp(&b.reference_asset_path)
                                                    })
                                            });

                                            // Create the response object (now a simple vector)
                                            let folder_match_response: FolderGeometricMatchResponse = unique_matches;

                                            trace!("Formatting results for output");
                                            // Format and output the results
                                            match folder_match_response.format(format) {
                                                Ok(output) => {
                                                    trace!("Output formatted successfully");
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                        Err(e) => {
                                            error_utils::report_error(&e);
                                            Ok(())
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error getting assets for folder '{}': {}", folder_path, e);
                                    eprintln!("Error getting assets for folder '{}': {}", folder_path, e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
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
                    
                    let format = extract_format_param_with_default(sub_matches, PARAMETER_FORMAT)?;
                    
                    // Try to get access token and create asset via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
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
                            
                            // Try to resolve the folder path to a folder ID
                            let folder_id = if let Some(folder_path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                                if !folder_path.is_empty() {
                                    // Build hierarchy and resolve path to folder ID
                                    match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                        Ok(hierarchy) => {
                                            if let Some(folder_node) = hierarchy.get_folder_by_path(folder_path) {
                                                Some(folder_node.folder.id.clone())
                                            } else {
                                                return Err(CliError::FolderNotFound { identifier: folder_path.to_string() });
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error building folder hierarchy: {}", e);
                                            return Err(CliError::ConfigurationError(pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory));
                                        }
                                    }
                                } else {
                                    None // Empty folder path
                                }
                            } else {
                                None // No folder path specified
                            };
                            
                            match client.create_asset(&tenant, file_path.to_str().unwrap(), sub_matches.get_one::<String>(PARAMETER_PATH).map(|s| s.as_str()), folder_id.as_deref()).await {
                                Ok(asset_response) => {
                                    // Invalidate cache for this tenant since we've modified asset state
                                    match AssetCache::load() {
                                        Ok(cache) => {
                                            cache.invalidate_tenant(&tenant);
                                            if let Err(e) = cache.save() {
                                                debug!("Failed to save invalidated cache: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            debug!("Failed to load cache for invalidation: {}", e);
                                        }
                                    }
                                    
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
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
                Some((COMMAND_CREATE_BATCH, sub_matches)) => {
                    trace!("Executing asset create-batch command");
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

                    let glob_pattern = sub_matches.get_one::<String>("files")
                        .ok_or(CliError::MissingRequiredArgument("files".to_string()))?
                        .clone();

                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT)
                        .ok_or_else(|| CliError::MissingRequiredArgument(PARAMETER_FORMAT.to_string()))
                        .map(|s| s.clone())
                        .unwrap_or_else(|_| "json".to_string());
                    let format = OutputFormat::from_str(&format_str)
                        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_str)))?;

                    let concurrent_param = sub_matches.get_one::<usize>("concurrent")
                        .unwrap_or(&5);
                    let concurrent = *concurrent_param;
                    let show_progress = sub_matches.get_flag("progress");

                    // Try to get access token and create assets via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);

                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }

                            // Resolve the folder path to a folder ID
                            let folder_id = if let Some(folder_path) = sub_matches.get_one::<String>(PARAMETER_PATH) {
                                if !folder_path.is_empty() {
                                    debug!("Resolving folder path: {}", folder_path);
                                    // Build hierarchy and resolve path to folder ID
                                    match FolderHierarchy::build_from_api(&mut client, &tenant).await {
                                        Ok(hierarchy) => {
                                            if let Some(folder_node) = hierarchy.get_folder_by_path(folder_path) {
                                                debug!("Found folder ID: {}", folder_node.folder.id);
                                                Some(folder_node.folder.id.clone())
                                            } else {
                                                debug!("Folder not found at path: {}", folder_path);
                                                return Err(CliError::FolderNotFound { identifier: folder_path.to_string() });
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error building folder hierarchy: {}", e);
                                            return Err(CliError::ConfigurationError(pcli2::configuration::ConfigurationError::FailedToFindConfigurationDirectory));
                                        }
                                    }
                                } else {
                                    debug!("Empty folder path provided");
                                    None // Empty folder path
                                }
                            } else {
                                debug!("No folder path specified");
                                None // No folder path specified
                            };

                            match client.create_assets_batch(&tenant, &glob_pattern, sub_matches.get_one::<String>(PARAMETER_PATH).map(|s| s.as_str()), folder_id.as_deref(), concurrent, show_progress).await {
                                Ok(asset_responses) => {
                                    // Invalidate cache for this tenant since we've modified asset state
                                    match AssetCache::load() {
                                        Ok(cache) => {
                                            cache.invalidate_tenant(&tenant);
                                            if let Err(e) = cache.save() {
                                                debug!("Failed to save invalidated cache: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            debug!("Failed to load cache for invalidation: {}", e);
                                        }
                                    }
                                    
                                    // Convert responses to assets
                                    let assets: Vec<Asset> = asset_responses.into_iter()
                                        .map(|asset_response| {
                                            // For batch uploads, we'll use the asset path from the API response
                                            let path = asset_response.path.clone();
                                            Asset::from_asset_response(asset_response, path)
                                        })
                                        .collect();

                                    // Create an asset list for formatting
                                    let mut asset_list = pcli2::model::AssetList::empty();
                                    for asset in assets {
                                        asset_list.insert(asset);
                                    }
                                    match asset_list.format(format) {
                                        Ok(output) => {
                                            println!("{}", output);
                                            Ok(())
                                        }
                                        Err(e) => Err(CliError::FormattingError(e)),
                                    }
                                }
                                Err(e) => {
                                    error!("Error creating assets batch: {}", e);
                                    eprintln!("Error creating assets batch: {}. Some assets may have been uploaded successfully before the error occurred.", e);
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
                Some((COMMAND_LIST, sub_matches)) => {
                    trace!("Executing asset list command");
                    
                    let format_param = sub_matches.get_one::<String>(PARAMETER_FORMAT)
                        .ok_or_else(|| CliError::MissingRequiredArgument(PARAMETER_FORMAT.to_string()))?;
                    let format = OutputFormat::from_str(format_param)
                        .map_err(|_| CliError::MissingRequiredArgument(format!("Invalid format: {}", format_param)))?;
                    
                    // Validate format - only JSON and CSV are supported for assets
                    if format == OutputFormat::Tree {
                        eprintln!("Tree format is not supported for asset listing");
                        return Ok(());
                    }
                    
                    // Check if refresh is requested
                    let refresh_requested = sub_matches.get_flag(PARAMETER_REFRESH);
                    
                    // Check if metadata should be included
                    let _include_metadata = sub_matches.get_flag("metadata");
                    
                    // Try to get access token and list assets from Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Get tenant ID with resolution
                            let tenant = get_tenant_id(&mut client, sub_matches, &configuration).await?;
                            
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
                            
                            // Check if metadata should be included
                            let _include_metadata = sub_matches.get_flag("metadata");
                            
                            // Persist the potentially updated access token back to keyring
                            if let Some(updated_token) = client.get_access_token() {
                                if let Err(e) = keyring.put("default", "access-token".to_string(), updated_token) {
                                    warn!("Failed to persist updated access token: {}", e);
                                }
                            }
                            
                            match asset_list.format_with_metadata(format, _include_metadata) {
                                Ok(output) => {
                                    println!("{}", output);
                                    Ok(())
                                }
                                Err(e) => Err(CliError::FormattingError(e)),
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }

                Some((COMMAND_METADATA, sub_matches)) => {
                    match sub_matches.subcommand() {
                        // Handle asset metadata create
                        Some((COMMAND_CREATE, sub_matches)) => {
                            trace!("Executing asset metadata create command");
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
                            
                            // Get metadata parameters from command line
                            let metadata_name = sub_matches.get_one::<String>("name")
                                .ok_or(CliError::MissingRequiredArgument("name".to_string()))?;
                            let metadata_value = sub_matches.get_one::<String>("value")
                                .ok_or(CliError::MissingRequiredArgument("value".to_string()))?;
                            let metadata_type = sub_matches.get_one::<String>("type")
                                .map(|s| s.as_str())
                                .unwrap_or("text");
                            
                            // Convert the single metadata entry to JSON value using shared function
                            let json_value = pcli2::metadata::convert_single_metadata_to_json_value(
                                metadata_name, 
                                metadata_value, 
                                metadata_type
                            );
                            
                            // Create a HashMap with the single metadata entry
                            let mut metadata: std::collections::HashMap<String, serde_json::Value> = 
                                std::collections::HashMap::new();
                            metadata.insert(metadata_name.clone(), json_value);
                            
                            let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                            let _format = OutputFormat::from_str(&format_str).unwrap();
                            
                            // Check if refresh is requested for metadata field cache
                            let refresh_requested = sub_matches.get_flag(PARAMETER_REFRESH);
                            
                            // Try to get access token and update asset metadata via Physna V3 API
                            let mut keyring = Keyring::default();
                            match keyring.get("default", "access-token".to_string()) {
                                Ok(Some(token)) => {
                                    let mut client = PhysnaApiClient::new().with_access_token(token);
                                    
                                    // Try to get client credentials for automatic token refresh
                                    if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                        keyring.get("default", "client-id".to_string()),
                                        keyring.get("default", "client-secret".to_string())
                                    ) {
                                        client = client.with_client_credentials(client_id, client_secret);
                                    }
                                    
                                    // Resolve asset ID from either UUID parameter or path
                                    let asset_id = if let Some(id) = asset_uuid_param {
                                        id.clone()
                                    } else if let Some(path) = asset_path_param {
                                        // Look up asset by path to get UUID
                                        debug!("Looking up asset by path: {}", path);
                                        // Get asset cache or fetch assets from API
                                        match AssetCache::get_or_fetch(&mut client, &tenant).await {
                                            Ok(asset_list_response) => {
                                                // Convert to AssetList to use find_by_path
                                                let asset_list = asset_list_response.to_asset_list();
                                                // Find the asset by path
                                                if let Some(asset) = asset_list.find_by_path(path) {
                                                    if let Some(uuid) = asset.uuid() {
                                                        trace!("Found asset with UUID: {}", uuid);
                                                        uuid.clone()
                                                    } else {
                                                        eprintln!("Asset found by path '{}' but has no UUID", path);
                                                        return Ok(());
                                                    }
                                                } else {
                                                    eprintln!("Asset not found by path '{}'", path);
                                                    return Ok(());
                                                }
                                            }
                                            Err(e) => {
                                                error!("Error getting asset cache: {}", e);
                                                eprintln!("Error getting asset cache: {}", e);
                                                return Ok(());
                                            }
                                        }
                                    } else {
                                        // This shouldn't happen due to our earlier check, but just in case
                                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                                    };
                                    
                                    // Get list of existing metadata fields to check if new ones need to be created
                                    match pcli2::metadata_cache::MetadataCache::get_or_fetch(&mut client, &tenant, refresh_requested).await {
                                        Ok(metadata_fields_response) => {
                                            // Extract existing field names
                                            let existing_field_names: std::collections::HashSet<String> = 
                                                metadata_fields_response.metadata_fields
                                                    .iter()
                                                    .map(|field| field.name.clone())
                                                    .collect();
                                            
                                            // Check for new metadata fields that need to be created
                                            for (field_name, _value) in &metadata {
                                                if !existing_field_names.contains(field_name) {
                                                    trace!("Creating new metadata field: {}", field_name);
                                                    // Use the provided type or default to "text"
                                                    let field_type_opt = sub_matches.get_one::<String>("type");
                                                    let field_type_str = field_type_opt.map(|s| s.as_str()).unwrap_or("text");
                                                    match client.create_metadata_field(&tenant, field_name, Some(field_type_str)).await {
                                                        Ok(_) => {
                                                            debug!("Successfully created metadata field: {} with type {}", field_name, field_type_str);
                                                            // Invalidate the cache since we've added a new field
                                                            match pcli2::metadata_cache::MetadataCache::load() {
                                                                Ok(cache) => {
                                                                    let mut mutable_cache = cache;
                                                                    mutable_cache.invalidate_tenant(&tenant);
                                                                    if let Err(e) = mutable_cache.save() {
                                                                        debug!("Failed to save invalidated metadata cache: {}", e);
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    debug!("Failed to load metadata cache for invalidation: {}", e);
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!("Error creating metadata field '{}': {}", field_name, e);
                                                            eprintln!("Error creating metadata field '{}': {}", field_name, e);
                                                            return Ok(());
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // Now update the asset metadata with the new or existing fields
                                            match client.update_asset_metadata(&tenant, &asset_id, &metadata).await {
                                                Ok(()) => {
                                                    // On successful metadata update, return no output as requested
                                                    Ok(())
                                                }
                                                Err(e) => {
                                                    error!("Error updating asset metadata: {}", e);
                                                    eprintln!("Error updating asset metadata: {}", e);
                                                    Ok(())
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error fetching metadata fields list: {}", e);
                                            eprintln!("Error fetching metadata fields list: {}", e);
                                            Ok(())
                                        }
                                    }
                                }
                                Ok(None) => {
                                    error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                                    Ok(())
                                }
                                Err(e) => {
                                    error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                                    Ok(())
                                }
                            }
                        }
                        // Handle asset metadata delete
                        Some((COMMAND_DELETE, sub_matches)) => {
                            trace!("Executing asset metadata delete command");
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
                            
                            // Get metadata names from command line (can be multiple occurrences or comma-separated)
                            let metadata_name_strings: Vec<String> = sub_matches.get_many::<String>("name")
                                .ok_or(CliError::MissingRequiredArgument("name".to_string()))?
                                .flat_map(|name_str| name_str.split(',').map(|s| s.trim().to_string()))
                                .collect();
                            let metadata_names: Vec<&str> = metadata_name_strings.iter().map(|s| s.as_str()).collect();
                            
                            let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                            let _format = OutputFormat::from_str(&format_str).unwrap();
                            
                            // Try to get access token and delete asset metadata via Physna V3 API
                            let mut keyring = Keyring::default();
                            match keyring.get("default", "access-token".to_string()) {
                                Ok(Some(token)) => {
                                    let mut client = PhysnaApiClient::new().with_access_token(token);
                                    
                                    // Try to get client credentials for automatic token refresh
                                    if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                        keyring.get("default", "client-id".to_string()),
                                        keyring.get("default", "client-secret".to_string())
                                    ) {
                                        client = client.with_client_credentials(client_id, client_secret);
                                    }
                                    
                                    // Resolve asset ID from either UUID parameter or path
                                    let asset_id = if let Some(id) = asset_uuid_param {
                                        id.clone()
                                    } else if let Some(path) = asset_path_param {
                                        // Look up asset by path to get UUID
                                        debug!("Looking up asset by path: {}", path);
                                        // Get asset cache or fetch assets from API
                                        match AssetCache::get_or_fetch(&mut client, &tenant).await {
                                            Ok(asset_list_response) => {
                                                // Convert to AssetList to use find_by_path
                                                let asset_list = asset_list_response.to_asset_list();
                                                // Find the asset by path
                                                if let Some(asset) = asset_list.find_by_path(path) {
                                                    if let Some(uuid) = asset.uuid() {
                                                        trace!("Found asset with UUID: {}", uuid);
                                                        uuid.clone()
                                                    } else {
                                                        eprintln!("Asset found by path '{}' but has no UUID", path);
                                                        return Ok(());
                                                    }
                                                } else {
                                                    eprintln!("Asset not found by path '{}'", path);
                                                    return Ok(());
                                                }
                                            }
                                            Err(e) => {
                                                error!("Error getting asset cache: {}", e);
                                                eprintln!("Error getting asset cache: {}", e);
                                                return Ok(());
                                            }
                                        }
                                    } else {
                                        // This shouldn't happen due to our earlier check, but just in case
                                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                                    };
                                    
                                    match client.delete_asset_metadata(&tenant, &asset_id, metadata_names).await {
                                        Ok(()) => {
                                            // On successful metadata deletion, return no output
                                            Ok(())
                                        }
                                        Err(e) => {
                                            error!("Error deleting asset metadata: {}", e);
                                            eprintln!("Error deleting asset metadata: {}", e);
                                            Ok(())
                                        }
                                    }
                                }
                                Ok(None) => {
                                    error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                                    Ok(())
                                }
                                Err(e) => {
                                    error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                                    Ok(())
                                }
                            }
                        }
                        // Handle asset metadata get
                        Some(("get", sub_matches)) => {
                            trace!("Executing asset metadata get command");
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
                            
                            // Try to get access token and get asset metadata via Physna V3 API
                            let mut keyring = Keyring::default();
                            match keyring.get("default", "access-token".to_string()) {
                                Ok(Some(token)) => {
                                    let mut client = PhysnaApiClient::new().with_access_token(token);
                                    
                                    // Try to get client credentials for automatic token refresh
                                    if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                        keyring.get("default", "client-id".to_string()),
                                        keyring.get("default", "client-secret".to_string())
                                    ) {
                                        client = client.with_client_credentials(client_id, client_secret);
                                    }
                                    
                                    // Resolve asset ID from either UUID parameter or path
                                    let asset_id = if let Some(id) = asset_uuid_param {
                                        id.clone()
                                    } else if let Some(path) = asset_path_param {
                                        // Look up asset by path to get UUID
                                        debug!("Looking up asset by path: {}", path);
                                        // Get asset cache or fetch assets from API
                                        match AssetCache::get_or_fetch(&mut client, &tenant).await {
                                            Ok(asset_list_response) => {
                                                // Convert to AssetList to use find_by_path
                                                let asset_list = asset_list_response.to_asset_list();
                                                // Find the asset by path
                                                if let Some(asset) = asset_list.find_by_path(path) {
                                                    if let Some(uuid) = asset.uuid() {
                                                        trace!("Found asset with UUID: {}", uuid);
                                                        uuid.clone()
                                                    } else {
                                                        eprintln!("Asset found by path '{}' but has no UUID", path);
                                                        return Ok(());
                                                    }
                                                } else {
                                                    eprintln!("Asset not found by path '{}'", path);
                                                    return Ok(());
                                                }
                                            }
                                            Err(e) => {
                                                error!("Error getting asset cache: {}", e);
                                                eprintln!("Error getting asset cache: {}", e);
                                                return Ok(());
                                            }
                                        }
                                    } else {
                                        // This shouldn't happen due to our earlier check, but just in case
                                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                                    };
                                    
                                    // Get the asset details which includes metadata
                                    match client.get_asset(&tenant, &asset_id).await {
                                        Ok(asset_response) => {
                                            // Extract metadata from the asset response
                                            let metadata = &asset_response.metadata;
                                            
                                            match format {
                                                OutputFormat::Json => {
                                                    // Output metadata as JSON
                                                    match serde_json::to_string_pretty(metadata) {
                                                        Ok(json_output) => {
                                                            println!("{}", json_output);
                                                            Ok(())
                                                        }
                                                        Err(e) => {
                                                            error!("Error serializing metadata to JSON: {}", e);
                                                            eprintln!("Error serializing metadata to JSON: {}", e);
                                                            Ok(())
                                                        }
                                                    }
                                                }
                                                OutputFormat::Csv => {
                                                    // Output metadata in CSV format that matches create-batch input
                                                    let asset_path_for_csv = if let Some(path) = asset_path_param {
                                                        path.to_string()
                                                    } else {
                                                        // Get the path from the asset response
                                                        asset_response.path.clone()
                                                    };
                                                    
                                                    // Output CSV header
                                                    println!("ASSET_PATH,NAME,VALUE");
                                                    
                                                    // Output each metadata field as a row
                                                    for (name, value) in metadata {
                                                        // Convert JSON value to string representation for CSV
                                                        let value_str = match value {
                                                            serde_json::Value::String(s) => s.clone(),
                                                            _ => value.to_string(),
                                                        };
                                                        
                                                        // Escape quotes in the value for CSV
                                                        let escaped_value = value_str.replace("\"", "\"\"");
                                                        println!("{},\"{}\",\"{}\"", asset_path_for_csv, name, escaped_value);
                                                    }
                                                    Ok(())
                                                }
                                                OutputFormat::Tree => {
                                                    // For tree format, we'll output a simple representation
                                                    println!("Asset: {}", asset_id);
                                                    let asset_path_for_display = if let Some(path) = asset_path_param {
                                                        path.to_string()
                                                    } else {
                                                        asset_response.path.clone()
                                                    };
                                                    println!("Path: {}", asset_path_for_display);
                                                    println!("Metadata:");
                                                    for (name, value) in metadata {
                                                        println!("  {}: {}", name, value);
                                                    }
                                                    Ok(())
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error fetching asset metadata: {}", e);
                                            eprintln!("Error fetching asset metadata: {}", e);
                                            Ok(())
                                        }
                                    }
                                }
                                Ok(None) => {
                                    error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
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
                Some((COMMAND_GET, sub_matches)) => {
                    trace!("Executing asset get command");
                    // Get tenant identifier from explicit parameter or fall back to active tenant from configuration
                    let tenant_identifier = match sub_matches.get_one::<String>(PARAMETER_TENANT) {
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
                    let _format = OutputFormat::from_str(&format_str).unwrap();
                    
                    // Try to get access token and get asset via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve tenant identifier to tenant ID
                            let tenant = resolve_tenant_identifier_to_id(&mut client, tenant_identifier).await?;
                            
                            // Resolve asset ID from either UUID parameter or path
                            let asset_id = if let Some(uuid) = asset_uuid_param {
                                uuid.clone()
                            } else if let Some(path) = asset_path_param {
                                // To resolve asset by path, we need to:
                                // 1. Get all assets for the tenant
                                // 2. Find the asset with matching path
                                // Look up asset by path to get UUID (efficiently)
                                trace!("Resolving asset by path: {}", path);
                                debug!("About to call resolve_asset_path_to_uuid for path: {}", path);
                                match pcli2::resolution_utils::resolve_asset_path_to_uuid(&mut client, &tenant, path).await {
                                    Ok(uuid) => {
                                        // Path resolution succeeded, but token might have been refreshed during the process
                                        if let Some(updated_token) = client.get_access_token() {
                                            if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                warn!("Failed to persist updated access token: {}", token_err);
                                            }
                                        }
                                        uuid
                                    },
                                    Err(e) => {
                                        // Even if path resolution failed, persist the potentially updated access token back to keyring
                                        if let Some(updated_token) = client.get_access_token() {
                                            if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                warn!("Failed to persist updated access token: {}", token_err);
                                            }
                                        }
                                        
                                        // Convert ApiError to CliError
                                        let cli_error = CliError::ConfigurationError(
                                            pcli2::configuration::ConfigurationError::FailedToLoadData {
                                                cause: Box::new(e)
                                            }
                                        );
                                        
                                        eprintln!("Error resolving asset path '{}': {}", path, cli_error);
                                        return Ok(());
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
                                    // Convert AssetResponse to Asset
                                    let asset = pcli2::model::Asset::from_asset_response(asset_response, asset_id.clone());
                                    
                                    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
                                    let _format = OutputFormat::from_str(&format_str).unwrap();
                                    // Persist the potentially updated access token back to keyring
                                    if let Some(updated_token) = client.get_access_token() {
                                        if let Err(e) = keyring.put("default", "access-token".to_string(), updated_token) {
                                            warn!("Failed to persist updated access token: {}", e);
                                        }
                                    }
                                    
                                    match asset.format(_format) {
                                        Ok(output) => {
                                            println!("{}", output);
                                            Ok(())
                                        }
                                        Err(e) => Err(CliError::FormattingError(e)),
                                    }
                                }
                                Err(e) => {
                                    // Even if the operation failed, persist the potentially updated access token back to keyring
                                    if let Some(updated_token) = client.get_access_token() {
                                        if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                            warn!("Failed to persist updated access token: {}", token_err);
                                        }
                                    }
                                    
                                    error!("Error fetching asset: {}", e);
                                    match e {
                                        pcli2::physna_v3::ApiError::RetryFailed(msg) => {
                                            eprintln!("Error fetching asset: {}", msg);
                                        }
                                        pcli2::physna_v3::ApiError::HttpError(http_err) => {
                                            if http_err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                                                eprintln!("Error: The asset with ID '{}' cannot be found in tenant '{}'", asset_id, tenant);
                                            } else if http_err.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
                                                eprintln!("Error: Unauthorized access. Please check your authentication credentials.");
                                            } else if http_err.status() == Some(reqwest::StatusCode::FORBIDDEN) {
                                                eprintln!("Error: Access forbidden. You don't have permission to access this asset.");
                                            } else {
                                                eprintln!("Error fetching asset: HTTP error {}", http_err);
                                            }
                                        }
                                        _ => {
                                            eprintln!("Error fetching asset: {}", e);
                                        }
                                    }
                                    Ok(())
                                }
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
                Some((COMMAND_DELETE, sub_matches)) => {
                    trace!("Executing asset delete command");
                    
                    let asset_uuid_param = sub_matches.get_one::<String>(PARAMETER_UUID);
                    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
                    
                    // Must provide either asset UUID or path
                    if asset_uuid_param.is_none() && asset_path_param.is_none() {
                        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
                    }
                    
                    // Try to get access token and delete asset via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Get tenant ID with resolution
                            let tenant = get_tenant_id(&mut client, sub_matches, &configuration).await?;
                            
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
                                    // Invalidate cache for this tenant since we've modified asset state
                                    match AssetCache::load() {
                                        Ok(cache) => {
                                            cache.invalidate_tenant(&tenant);
                                            if let Err(e) = cache.save() {
                                                debug!("Failed to save invalidated cache: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            debug!("Failed to load cache for invalidation: {}", e);
                                        }
                                    }
                                    
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
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }
                Some(("dependencies", sub_matches)) => {
                    trace!("Executing asset dependencies command");
                    // Get tenant identifier from explicit parameter or fall back to active tenant from configuration
                    let tenant_identifier = match sub_matches.get_one::<String>(PARAMETER_TENANT) {
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
                    
                    // Try to get access token and get asset dependencies via Physna V3 API
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Resolve tenant identifier to tenant ID
                            let tenant = resolve_tenant_identifier_to_id(&mut client, tenant_identifier).await?;
                            
                            // Resolve asset path to get dependencies
                            if let Some(path) = asset_path_param {
                                trace!("Getting dependencies for asset by path: {}", path);
                                
                                // Check if recursive flag is set
                                let recursive = sub_matches.get_flag("recursive");
                                
                                if recursive {
                                    // Handle recursive dependencies
                                    match get_asset_dependencies_recursive(&mut client, &tenant, path).await {
                                        Ok(mut dependencies_response) => {
                                            // Set the original asset path for tree formatting
                                            dependencies_response.original_asset_path = path.to_string();
                                            
                                            // Even if the API call succeeded, persist the potentially updated access token back to keyring
                                            if let Some(updated_token) = client.get_access_token() {
                                                if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                    warn!("Failed to persist updated access token: {}", token_err);
                                                }
                                            }
                                            
                                            match dependencies_response.format(format) {
                                                Ok(output) => {
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                        Err(e) => {
                                            // Even if the recursive call failed, persist the potentially updated access token back to keyring
                                            if let Some(updated_token) = client.get_access_token() {
                                                if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                    warn!("Failed to persist updated access token: {}", token_err);
                                                }
                                            }
                                            
                                            error!("Error getting recursive asset dependencies for path '{}': {}", path, e);
                                            eprintln!("Error getting recursive asset dependencies for path '{}': {}", path, e);
                                            Ok(())
                                        }
                                    }
                                } else {
                                    // Handle non-recursive dependencies (original behavior)
                                    match client.get_asset_dependencies_by_path(&tenant, path).await {
                                        Ok(mut dependencies_response) => {
                                            // Set the original asset path for tree formatting
                                            dependencies_response.original_asset_path = path.to_string();
                                            
                                            // Even if the API call succeeded, persist the potentially updated access token back to keyring
                                            if let Some(updated_token) = client.get_access_token() {
                                                if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                    warn!("Failed to persist updated access token: {}", token_err);
                                                }
                                            }
                                            
                                            match dependencies_response.format(format) {
                                                Ok(output) => {
                                                    println!("{}", output);
                                                    Ok(())
                                                }
                                                Err(e) => Err(CliError::FormattingError(e)),
                                            }
                                        }
                                        Err(e) => {
                                            // Even if the API call failed, persist the potentially updated access token back to keyring
                                            if let Some(updated_token) = client.get_access_token() {
                                                if let Err(token_err) = keyring.put("default", "access-token".to_string(), updated_token) {
                                                    warn!("Failed to persist updated access token: {}", token_err);
                                                }
                                            }
                                            
                                            error!("Error getting asset dependencies for path '{}': {}", path, e);
                                            eprintln!("Error getting asset dependencies for path '{}': {}", path, e);
                                            Ok(())
                                        }
                                    }
                                }
                            } else if let Some(uuid) = asset_uuid_param {
                                trace!("Getting dependencies for asset by UUID: {}", uuid);
                                // For UUID-based lookup, we would need a different API endpoint
                                // For now, we'll implement only path-based dependency lookup
                                error!("UUID-based dependency lookup not yet implemented");
                                eprintln!("Error: UUID-based dependency lookup not yet implemented. Please use --path instead.");
                                Ok(())
                            } else {
                                // This shouldn't happen due to our earlier check, but just in case
                                error!("Either asset UUID or path must be provided");
                                eprintln!("Error: Either asset UUID or path must be provided");
                                Ok(())
                            }
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument(format!("Error retrieving access token: {}", e)));
                            Ok(())
                        }
                    }
                }                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    sub_matches,
                ))),
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
            match sub_matches.subcommand() {
                Some((COMMAND_SET, sub_matches)) => {
                    match sub_matches.subcommand() {
                        Some(("tenant", sub_matches)) => {
                            trace!("Executing context set tenant command");
                            let name = sub_matches.get_one::<String>(PARAMETER_NAME);
                            
                            // Try to get access token and fetch tenant info from Physna V3 API
                            let mut keyring = Keyring::default();
                            match keyring.get("default", "access-token".to_string()) {
                                Ok(Some(token)) => {
                                    let mut client = PhysnaApiClient::new().with_access_token(token);
                                    
                                    // Try to get client credentials for automatic token refresh
                                    if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                        keyring.get("default", "client-id".to_string()),
                                        keyring.get("default", "client-secret".to_string())
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
                                            error_utils::report_error(&e);
                                            Ok(())
                                        }
                                    }
                                }
                                Ok(None) => {
                                    error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
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
                                    println!("ACTIVE_TENANT_NAME\n{}", tenant_name);
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
                Some(("inference", sub_matches)) => {
                    trace!("Executing asset metadata inference command");
                    
                    // Try to get access token and perform metadata inference
                    let mut keyring = Keyring::default();
                    match keyring.get("default", "access-token".to_string()) {
                        Ok(Some(token)) => {
                            let mut client = PhysnaApiClient::new().with_access_token(token);
                            
                            // Try to get client credentials for automatic token refresh
                            if let (Ok(Some(client_id)), Ok(Some(client_secret))) = (
                                keyring.get("default", "client-id".to_string()),
                                keyring.get("default", "client-secret".to_string())
                            ) {
                                client = client.with_client_credentials(client_id, client_secret);
                            }
                            
                            // Get tenant ID with resolution using helper function
                            let tenant = get_tenant_id(&mut client, sub_matches, &configuration).await?;
                            
                            // Get parameters
                            let asset_path = sub_matches.get_one::<String>(PARAMETER_PATH)
                                .ok_or(CliError::MissingRequiredArgument("Asset path is required".to_string()))?;
                            
                            // Get metadata names - handle both repeated flags and comma-separated values
                            let mut metadata_names = Vec::new();
                            if let Some(name_values) = sub_matches.get_many::<String>("name") {
                                for name_value in name_values {
                                    // Split by comma to handle comma-separated names in a single parameter
                                    let names: Vec<&str> = name_value.split(',').map(|s| s.trim()).collect();
                                    for name in names {
                                        if !name.is_empty() {
                                            metadata_names.push(name.to_string());
                                        }
                                    }
                                }
                            }
                            
                            if metadata_names.is_empty() {
                                return Err(CliError::MissingRequiredArgument("At least one metadata name must be specified".to_string()));
                            }
                            
                            let threshold = *sub_matches.get_one::<f64>("threshold").unwrap_or(&80.0);
                            let recursive = sub_matches.get_flag("recursive");
                            
                            // Validate threshold is between 0 and 100
                            if !(0.0..=100.0).contains(&threshold) {
                                eprintln!("Threshold must be between 0.00 and 100.00");
                                return Ok(());
                            }
                            
                            let _format = extract_format_param_with_default(sub_matches, PARAMETER_FORMAT)?;
                            
                            trace!("Performing metadata inference for asset {} with threshold {} and {} metadata fields", asset_path, threshold, metadata_names.len());
                            
                            // Execute the metadata inference logic
                            execute_metadata_inference(&mut client, &tenant, asset_path, &metadata_names, threshold, recursive).await?;
                            
                            Ok(())
                        }
                        Ok(None) => {
                            error_utils::report_error(&CliError::MissingRequiredArgument("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'".to_string()));
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
        // Cache commands
        Some((COMMAND_CACHE, _sub_matches)) => {
            match _sub_matches.subcommand() {
                Some((_command_purge, _)) => {
                    trace!("Executing cache purge command");
                    
                    // Purge all cached data
                    match FolderCache::purge_all() {
                        Ok(_) => {
                            println!("Successfully purged all cached data");
                            Ok(())
                        }
                        Err(e) => {
                            error_utils::report_error(&e);
                            Ok(())
                        }
                    }
                }
                _ => Err(CliError::UnsupportedSubcommand(extract_subcommand_name(
                    _sub_matches,
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

/// Execute the metadata inference logic
/// 
/// This function performs metadata inference by taking a reference asset, extracting specified
/// metadata fields from it, finding geometrically similar assets, and applying those metadata
/// fields to the matching assets.
/// 
/// # Arguments
/// * `client` - Mutable reference to the Physna API client
/// * `tenant` - Tenant ID to operate within
/// * `reference_asset_path` - Path to the reference asset
/// * `metadata_names` - Vector of metadata field names to copy
/// * `threshold` - Geometric similarity threshold (0.00-100.00)
/// * `recursive` - Whether to apply inference recursively to discovered matches
/// 
/// # Returns
/// * `Ok(())` - Successfully completed metadata inference
/// * `Err(CliError)` - If there was an error during inference
async fn execute_metadata_inference(
    client: &mut PhysnaApiClient,
    tenant: &str,
    reference_asset_path: &str,
    metadata_names: &[String],
    threshold: f64,
    recursive: bool,
) -> Result<(), CliError> {
    use std::collections::HashSet;
    
    // Queue for recursive processing
    let mut to_process = vec![reference_asset_path.to_string()];
    let mut processed_assets = HashSet::new();
    
    while let Some(current_asset_path) = to_process.pop() {
        // Skip if already processed
        if processed_assets.contains(&current_asset_path) {
            continue;
        }
        
        processed_assets.insert(current_asset_path.clone());
        
        trace!("Processing asset: {} for metadata inference", current_asset_path);
        
        // 1. Get the reference asset by path
        let asset_uuid = match pcli2::resolution_utils::resolve_asset_path_to_uuid(client, tenant, &current_asset_path).await {
            Ok(uuid) => uuid,
            Err(_) => {
                error!("Reference asset not found: {}", current_asset_path);
                continue;
            }
        };
        
        // 2. Get current metadata values from this asset
        let mut reference_asset_metadata = std::collections::HashMap::new();
        
        match client.get_asset(tenant, &asset_uuid).await {
            Ok(asset_response) => {
                // Extract the requested metadata fields only
                for metadata_name in metadata_names {
                    if let Some(value) = asset_response.metadata.get(metadata_name) {
                        reference_asset_metadata.insert(metadata_name.clone(), value.clone());
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Could not get asset details for '{}': {}", current_asset_path, e);
                continue;
            }
        }
        
        if reference_asset_metadata.is_empty() {
            trace!("No requested metadata found for asset: {}", current_asset_path);
            continue;
        }
        
        // 3. Perform geometric search for this asset at the specified threshold
        let geometric_matches = match client.geometric_search(tenant, &asset_uuid, threshold).await {
            Ok(matches) => matches,
            Err(e) => {
                error!("Geometric search failed for asset '{}': {}", current_asset_path, e);
                continue;
            }
        };
        
        trace!("Found {} geometric matches for asset {}", geometric_matches.matches.len(), current_asset_path);
        
        // 4. Apply metadata to matching assets
        for match_result in geometric_matches.matches {
            let candidate_asset_id = &match_result.asset.id;
            let candidate_asset_path = &match_result.asset.path;
            
            // Skip the original asset itself to avoid self-assignment if it appears in results
            if candidate_asset_id == &asset_uuid {
                continue;
            }
            
            trace!("Applying metadata to candidate: {} (path: {})", candidate_asset_id, candidate_asset_path);
            
            // Update metadata for candidate asset using the reference metadata
            match client.update_asset_metadata(tenant, candidate_asset_id, &reference_asset_metadata).await {
                Ok(_) => {
                    trace!("Updated metadata for asset {} (path: {})", candidate_asset_id, candidate_asset_path);
                }
                Err(e) => {
                    error!("Failed to update metadata for asset '{}': {}", candidate_asset_path, e);
                }
            }
            
            // If recursive, add this candidate to the processing queue (if not already processed)
            if recursive && !processed_assets.contains(candidate_asset_path) {
                to_process.push(candidate_asset_path.clone());
            }
        }
    }
    
    Ok(())
}

/// Get asset dependencies recursively
/// 
/// This function fetches all dependencies of an asset including dependencies of dependencies
/// by making multiple API calls to build a complete dependency tree.
/// 
/// # Arguments
/// * `client` - Mutable reference to the Physna API client
/// * `tenant` - The tenant ID
/// * `asset_path` - The path of the asset to get dependencies for
/// 
/// # Returns
/// * `Ok(AssetDependenciesResponse)` - The complete dependency tree response
/// * `Err(CliError)` - If there was an error during API calls
async fn get_asset_dependencies_recursive(
    client: &mut PhysnaApiClient,
    tenant: &str,
    asset_path: &str,
) -> Result<pcli2::model::AssetDependenciesResponse, CliError> {
    use std::collections::HashSet;
    
    let mut all_dependencies = Vec::new();
    let mut to_process = vec![asset_path.to_string()];
    let mut processed_assets = HashSet::new();
    
    while let Some(current_path) = to_process.pop() {
        // Skip if already processed to avoid cycles
        if processed_assets.contains(&current_path) {
            continue;
        }
        
        processed_assets.insert(current_path.clone());
        
        match client.get_asset_dependencies_by_path(tenant, &current_path).await {
            Ok(deps_response) => {
                for dep in deps_response.dependencies {
                    all_dependencies.push(dep.clone());
                    // Add dependency to queue for further processing if not already processed
                    if !processed_assets.contains(&dep.asset.path) {
                        to_process.push(dep.asset.path.clone());
                    }
                }
            }
            Err(_) => {
                // If we can't get dependencies for this asset, just continue
                continue;
            }
        }
    }
    
    // Remove duplicates while preserving order
    let mut seen_paths = HashSet::new();
    let unique_dependencies: Vec<_> = all_dependencies
        .into_iter()
        .filter(|dep| seen_paths.insert(dep.asset.path.clone()))
        .collect();
    
    let total_count = unique_dependencies.len();
    
    // Create the final response with all dependencies
    Ok(pcli2::model::AssetDependenciesResponse {
        dependencies: unique_dependencies,
        page_data: pcli2::model::PageData {
            total: total_count,
            per_page: total_count,
            current_page: 1,
            last_page: 1,
            start_index: 1,
            end_index: total_count,
        },
        original_asset_path: asset_path.to_string(),
    })
}
//! Shared command parameters for all CLI commands.
//!
//! This module defines common parameters that are used across multiple command modules.
//! It provides a centralized place to define parameter names and common argument configurations.

use crate::format::OutputFormat;
use clap::{Arg, ArgAction};
use std::path::PathBuf;

// Command names
/// Command name for tenant operations
pub const COMMAND_TENANT: &str = "tenant";
/// Command name for folder operations
pub const COMMAND_FOLDER: &str = "folder";
/// Command name for asset operations
pub const COMMAND_ASSET: &str = "asset";
/// Command name for file operations (not yet implemented)
pub const COMMAND_FILE: &str = "file";

// CRUD operations
/// Command name for creating resources
pub const COMMAND_CREATE: &str = "create";
/// Command name for creating multiple resources
pub const COMMAND_CREATE_BATCH: &str = "create-batch";
/// Command name for retrieving resources
pub const COMMAND_GET: &str = "get";
/// Command name for listing resources
pub const COMMAND_LIST: &str = "list";
/// Command name for updating resources
pub const COMMAND_UPDATE: &str = "update";
/// Command name for deleting resources
pub const COMMAND_DELETE: &str = "delete";
/// Command name for matching assets geometrically
pub const COMMAND_MATCH: &str = "geometric-match";
/// Command name for metadata operations
pub const COMMAND_METADATA: &str = "metadata";

// Auth commands
/// Command name for authentication operations
pub const COMMAND_AUTH: &str = "auth";
/// Command name for login operations
pub const COMMAND_LOGIN: &str = "login";
/// Command name for logout operations
pub const COMMAND_LOGOUT: &str = "logout";

// Cache commands
/// Command name for cache operations
pub const COMMAND_CACHE: &str = "cache";
/// Command name for purging cache
pub const COMMAND_PURGE: &str = "purge";

// Context commands
/// Command name for context operations
pub const COMMAND_CONTEXT: &str = "context";
/// Command name for setting context
pub const COMMAND_SET: &str = "set";
/// Command name for clearing context
pub const COMMAND_CLEAR: &str = "clear";

// Config commands
/// Command name for configuration operations
pub const COMMAND_CONFIG: &str = "config";
/// Command name for exporting configuration
pub const COMMAND_EXPORT: &str = "export";
/// Command name for importing configuration
pub const COMMAND_IMPORT: &str = "import";

// Parameter names
/// Parameter name for output format
pub const PARAMETER_FORMAT: &str = "format";
/// Parameter name for output file path
pub const PARAMETER_OUTPUT: &str = "output";
/// Parameter name for input file path
pub const PARAMETER_INPUT: &str = "input";
/// Parameter name for OAuth2 client ID
pub const PARAMETER_CLIENT_ID: &str = "client-id";
/// Parameter name for OAuth2 client secret
pub const PARAMETER_CLIENT_SECRET: &str = "client-secret";
/// Parameter name for resource ID
pub const PARAMETER_ID: &str = "id";
/// Parameter name for resource UUID
pub const PARAMETER_UUID: &str = "uuid";
/// Parameter name for asset UUID
pub const PARAMETER_ASSET_UUID: &str = "asset-uuid";
/// Parameter name for resource name
pub const PARAMETER_NAME: &str = "name";
/// Parameter name for tenant ID or alias
pub const PARAMETER_TENANT: &str = "tenant";
/// Parameter name for parent folder ID
pub const PARAMETER_PARENT_FOLDER_ID: &str = "parent-folder-id";
/// Parameter name for folder path
pub const PARAMETER_PATH: &str = "path";
/// Parameter name for refresh flag
pub const PARAMETER_REFRESH: &str = "refresh";
/// Parameter name for file to upload
pub const PARAMETER_FILE: &str = "file";
/// Parameter name for recursive operations
pub const PARAMETER_RECURSIVE: &str = "recursive";

/// Create the global format parameter.
///
/// This parameter is used across multiple commands for output formatting.
pub fn format_parameter() -> Arg {
    Arg::new(PARAMETER_FORMAT)
        .short('f')
        .long(PARAMETER_FORMAT)
        .num_args(1)
        .required(false)
        .default_value("json")
        .global(true)
        .help("Output data format")
        .value_parser(OutputFormat::names())
}

/// Create the global output file parameter.
pub fn output_file_parameter() -> Arg {
    Arg::new(PARAMETER_OUTPUT)
        .short('o')
        .long(PARAMETER_OUTPUT)
        .num_args(1)
        .required(false)
        .help("Output file path")
        .value_parser(clap::value_parser!(PathBuf))
}

/// Create the global input file parameter.
pub fn input_file_parameter() -> Arg {
    Arg::new(PARAMETER_INPUT)
        .short('i')
        .long(PARAMETER_INPUT)
        .num_args(1)
        .required(false)
        .help("Input file path")
        .value_parser(clap::value_parser!(PathBuf))
}

/// Create the client ID parameter.
pub fn client_id_parameter() -> Arg {
    Arg::new(PARAMETER_CLIENT_ID)
        .long(PARAMETER_CLIENT_ID)
        .num_args(1)
        .required(false)
        .help("Client ID for OAuth2 authentication")
}

/// Create the client secret parameter.
pub fn client_secret_parameter() -> Arg {
    Arg::new(PARAMETER_CLIENT_SECRET)
        .long(PARAMETER_CLIENT_SECRET)
        .num_args(1)
        .required(false)
        .help("Client secret for OAuth2 authentication")
}

/// Create the ID parameter.
pub fn id_parameter() -> Arg {
    Arg::new(PARAMETER_ID)
        .short('i')
        .long(PARAMETER_ID)
        .num_args(1)
        .required(false)
        .help("Resource ID")
}

/// Create the UUID parameter.
pub fn uuid_parameter() -> Arg {
    Arg::new(PARAMETER_UUID)
        .short('u')
        .long(PARAMETER_UUID)
        .num_args(1)
        .required(false)
        .help("Resource UUID")
}

/// Create the asset UUID parameter.
pub fn asset_uuid_parameter() -> Arg {
    Arg::new(PARAMETER_ASSET_UUID)
        .long(PARAMETER_ASSET_UUID)
        .num_args(1)
        .required(false)
        .help("Asset UUID")
}

/// Create the name parameter.
pub fn name_parameter() -> Arg {
    Arg::new(PARAMETER_NAME)
        .short('n')
        .long(PARAMETER_NAME)
        .num_args(1)
        .required(false)
        .help("Resource name")
}

/// Create the tenant parameter.
pub fn tenant_parameter() -> Arg {
    Arg::new(PARAMETER_TENANT)
        .short('t')
        .long(PARAMETER_TENANT)
        .num_args(1)
        .required(false)
        .help("Tenant ID or alias")
}

/// Create the parent folder ID parameter.
pub fn parent_folder_id_parameter() -> Arg {
    Arg::new(PARAMETER_PARENT_FOLDER_ID)
        .long(PARAMETER_PARENT_FOLDER_ID)
        .num_args(1)
        .required(false)
        .help("Parent folder ID for creating subfolders")
}

/// Create the path parameter.
pub fn path_parameter() -> Arg {
    Arg::new(PARAMETER_PATH)
        .short('p')
        .long(PARAMETER_PATH)
        .num_args(1)
        .required(false)
        .help("Folder path (e.g., /Root/Child/Grandchild)")
}

/// Create the refresh parameter.
pub fn refresh_parameter() -> Arg {
    Arg::new(PARAMETER_REFRESH)
        .short('r')
        .long(PARAMETER_REFRESH)
        .action(ArgAction::SetTrue)
        .required(false)
        .help("Force refresh cache data from API")
}

/// Create the recursive parameter.
pub fn recursive_parameter() -> Arg {
    Arg::new(PARAMETER_RECURSIVE)
        .short('R')
        .long(PARAMETER_RECURSIVE)
        .action(ArgAction::SetTrue)
        .required(false)
        .help("Recursively apply operation (default: false for CSV/JSON, true for tree)")
}
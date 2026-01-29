//! Shared command parameters for all CLI commands.
//!
//! This module defines common parameters that are used across multiple command modules.
//! It provides a centralized place to define parameter names and common argument configurations.

use crate::format::OutputFormat;
use clap::{Arg, ArgAction, ArgGroup};
use std::path::PathBuf;
use uuid::Uuid;

// CRUD operations
pub const COMMAND_CREATE: &str = "create";
pub const COMMAND_CREATE_BATCH: &str = "create-batch";
pub const COMMAND_GET: &str = "get";
pub const COMMAND_LIST: &str = "list";
pub const COMMAND_UPDATE: &str = "update";
pub const COMMAND_DELETE: &str = "delete";

// Asset commands
pub const COMMAND_ASSET: &str = "asset";
pub const COMMAND_MATCH: &str = "geometric-match";
pub const COMMAND_MATCH_FOLDER: &str = "geometric-match-folder";
pub const COMMAND_PART_MATCH: &str = "part-match"; // Allow non snake case since it's used as a command name
pub const COMMAND_PART_MATCH_FOLDER: &str = "part-match-folder"; // Allow non snake case since it's used as a command name
pub const COMMAND_VISUAL_MATCH: &str = "visual-match"; // Allow non snake case since it's used as a command name
pub const COMMAND_VISUAL_MATCH_FOLDER: &str = "visual-match-folder"; // Allow non snake case since it's used as a command name
pub const COMMAND_TEXT_MATCH: &str = "text-match"; // Allow non snake case since it's used as a command name
pub const PARAMETER_FUZZY: &str = "fuzzy";
pub const COMMAND_METADATA: &str = "metadata";
pub const COMMAND_INFERENCE: &str = "inference"; // Allow non snake case since it's used as a command name
pub const COMMAND_DEPENDENCIES: &str = "dependencies";
pub const COMMAND_DOWNLOAD: &str = "download";
pub const COMMAND_DOWNLOAD_FOLDER: &str = "download-folder";

// Auth commands
pub const COMMAND_AUTH: &str = "auth";
pub const COMMAND_LOGIN: &str = "login";
pub const COMMAND_LOGOUT: &str = "logout";
pub const COMMAND_CLEAR_TOKEN: &str = "clear-token";
pub const COMMAND_EXPIRATION: &str = "expiration"; // Allow non snake case since it's used as a command name
                                                   //
                                                   // Tenant commands
pub const COMMAND_TENANT: &str = "tenant";

// Folder commands
pub const COMMAND_FOLDER: &str = "folder";
pub const COMMAND_FILE: &str = "file";
pub const COMMAND_UPLOAD: &str = "upload";

// Context commands have been moved to tenant command
pub const COMMAND_SET: &str = "set";
pub const COMMAND_CLEAR: &str = "clear";

// Config commands
pub const COMMAND_CONFIG: &str = "config";
pub const COMMAND_EXPORT: &str = "export";
pub const COMMAND_IMPORT: &str = "import";
pub const COMMAND_ENVIRONMENT: &str = "environment";
pub const COMMAND_ADD: &str = "add";
pub const COMMAND_USE: &str = "use";
pub const COMMAND_REMOVE: &str = "remove";
pub const COMMAND_ENVIRONMENT_LIST: &str = "list";
pub const COMMAND_ENVIRONMENT_GET: &str = "get";
pub const COMMAND_RESET: &str = "reset";
pub const COMMAND_CURRENT: &str = "current";
pub const COMMAND_STATE: &str = "state";

// Environment parameter names
pub const PARAMETER_API_URL: &str = "api-url";
pub const PARAMETER_UI_URL: &str = "ui-url";
pub const PARAMETER_AUTH_URL: &str = "auth-url";

// Parameter names
pub const PARAMETER_FORMAT: &str = "format";
pub const PARAMETER_METADATA: &str = "metadata";
pub const PARAMETER_PRETTY: &str = "pretty";
pub const PARAMETER_HEADERS: &str = "headers";
pub const PARAMETER_OUTPUT: &str = "output";
pub const PARAMETER_FILE: &str = "file";
pub const PARAMETER_FILES: &str = "files";
pub const PARAMETER_CLIENT_ID: &str = "client-id";
pub const PARAMETER_CLIENT_SECRET: &str = "client-secret";
pub const PARAMETER_UUID: &str = "uuid";
pub const PARAMETER_NAME: &str = "name";
pub const PARAMETER_TENANT_NAME: &str = PARAMETER_NAME;
pub const PARAMETER_TENANT_ID: &str = "id";
pub const PARAMETER_TENANT: &str = "tenant";
pub const PARAMETER_TENANT_UUID: &str = "tenant-uuid";
pub const PARAMETER_FOLDER_UUID: &str = "folder-uuid";
pub const PARAMETER_FOLDER_PATH: &str = "folder-path";
pub const PARAMETER_PARENT_FOLDER_UUID: &str = "parent-folder-uuid";
pub const PARAMETER_PARENT_FOLDER_PATH: &str = "parent-folder-path";
pub const PARAMETER_PATH: &str = "path";
pub const PARAMETER_REFRESH: &str = "refresh";
pub const PARAMETER_RECURSIVE: &str = "recursive";
pub const PARAMETER_CONCURRENT: &str = "concurrent";
pub const PARAMETER_PROGRESS: &str = "progress";
pub const PARAMETER_FOLDER_PATHS: &str = "folder-paths";
pub const PARAMETER_CONTINUE_ON_ERROR: &str = "continue-on-error";
pub const PARAMETER_DELAY: &str = "delay";
pub const PARAMETER_LOCAL_PATH: &str = "local-path";
pub const PARAMETER_SKIP_EXISTING: &str = "skip-existing";
pub const PARAMETER_RESUME: &str = "resume";

// Format options
pub const FORMAT_CSV: &str = "csv";
pub const FORMAT_JSON: &str = "json";
pub const FORMAT_TREE: &str = "tree";

/// Create the global format parameter.
///
/// This parameter is used across multiple commands for output formatting.
pub fn format_parameter() -> Arg {
    Arg::new(PARAMETER_FORMAT)
        .short('f')
        .long(PARAMETER_FORMAT)
        .num_args(1)
        .required(false)
        .env("PCLI2_FORMAT")
        .default_value("json")
        .global(true)
        .help("Output data format")
        .value_parser(OutputFormat::names())
}

/// This parameter iflag is used across multiple commands for output formatting.
pub fn format_pretty_parameter() -> Arg {
    Arg::new(PARAMETER_PRETTY)
        .long(PARAMETER_PRETTY)
        .action(ArgAction::SetTrue)
        .required(false)
        .help("Format the output pretty")
}

/// This parameter iflag is used across multiple commands for output formatting.
pub fn format_with_headers_parameter() -> Arg {
    Arg::new(PARAMETER_HEADERS)
        .long(PARAMETER_HEADERS)
        .action(ArgAction::SetTrue)
        .required(false)
        .env("PCLI2_HEADERS")
        .help("Format the output with headers")
}

/// This parameter iflag is used across multiple commands for output formatting.
pub fn format_with_metadata_parameter() -> Arg {
    Arg::new(PARAMETER_METADATA)
        .long(PARAMETER_METADATA)
        .action(ArgAction::SetTrue)
        .required(false)
        .help("Format the output to include metadata")
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
pub fn file_parameter() -> Arg {
    Arg::new(PARAMETER_FILE)
        .long(PARAMETER_FILE)
        .num_args(1)
        .required(false)
        .help("Input file path")
        .value_parser(clap::value_parser!(PathBuf))
}

pub fn multiple_files_parameter() -> Arg {
    Arg::new(PARAMETER_FILES)
        .long(PARAMETER_FILES)
        .num_args(1)
        .required(true)
        .help("Glob pattern to match files to upload (e.g., \"data/puzzle/*.STL\")")
        .value_parser(clap::value_parser!(String))
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

/// Create the UUID parameter.
pub fn uuid_parameter() -> Arg {
    Arg::new(PARAMETER_UUID)
        .short('u')
        .long(PARAMETER_UUID)
        .required(false) // this parameter should be used in a group with the patheee
        .value_parser(clap::value_parser!(Uuid))
        .help("Resource UUID")
}

/// Create the Folder UUID parameter.
pub fn folder_uuid_parameter() -> Arg {
    Arg::new(PARAMETER_FOLDER_UUID)
        .long(PARAMETER_FOLDER_UUID)
        .num_args(1)
        .required(true)
        .value_parser(clap::value_parser!(Uuid))
        .help("Resource's folder UUID")
}

/// Create the path parameter.
pub fn folder_path_parameter() -> Arg {
    Arg::new(PARAMETER_FOLDER_PATH)
        .short('p')
        .long(PARAMETER_FOLDER_PATH)
        .num_args(1)
        .required(true)
        .help("Folder path (e.g., /Root/Child/Grandchild)")
}

/// Create the parent folder UUID parameter.
pub fn parent_folder_uuid_parameter() -> Arg {
    Arg::new(PARAMETER_PARENT_FOLDER_UUID)
        .long(PARAMETER_PARENT_FOLDER_UUID)
        .num_args(1)
        .required(true)
        .value_parser(clap::value_parser!(Uuid))
        .help("Parent folder UUID where the new folder will be created")
}

/// Create the parent folder path parameter.
pub fn parent_folder_path_parameter() -> Arg {
    Arg::new(PARAMETER_PARENT_FOLDER_PATH)
        .long(PARAMETER_PARENT_FOLDER_PATH)
        .num_args(1)
        .required(true)
        .help("Parent folder path where the new folder will be created (e.g., /Root/Child/Grandchild)")
}

/// Create asset idenitfier group: it must be either --uuid or --path
pub fn asset_identifier_group() -> ArgGroup {
    ArgGroup::new("asset-identifier")
        .args([PARAMETER_UUID, PARAMETER_PATH])
        .multiple(false)
        .required(true)
}

pub fn asset_identifier_multiple_group() -> ArgGroup {
    ArgGroup::new("asset-identifier")
        .args([PARAMETER_UUID, PARAMETER_PATH])
        .multiple(true)
        .required(true)
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

/// Create the tenant name parameter.
pub fn tenant_name_parameter() -> Arg {
    Arg::new(PARAMETER_TENANT_NAME)
        .long(PARAMETER_TENANT_NAME)
        .num_args(1)
        .required(false)
        .help("Tenant short name (as shown in tenant list)")
}

/// Create the name parameter.
pub fn tenant_id_parameter() -> Arg {
    Arg::new(PARAMETER_TENANT_ID)
        .long(PARAMETER_TENANT_ID)
        .num_args(1)
        .required(false)
        .help("Tenant UUID")
}

/// Create the tenant UUID parameter.
pub fn tenant_uuid_parameter() -> Arg {
    Arg::new(PARAMETER_TENANT_UUID)
        .long(PARAMETER_TENANT_UUID)
        .num_args(1)
        .required(false)
        .value_parser(clap::value_parser!(uuid::Uuid))
        .help("Tenant UUID")
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

/// Create folder identifier group: it must be either --folder-uuid or --folder-path
pub fn folder_identifier_group() -> ArgGroup {
    ArgGroup::new("folder-identifier")
        .args([PARAMETER_FOLDER_UUID, PARAMETER_FOLDER_PATH])
        .multiple(false)
        .required(true)
}

/// Create parent folder identifier group: it must be either --parent-folder-uuid or --parent-folder-path
pub fn parent_folder_identifier_group() -> ArgGroup {
    ArgGroup::new("parent-folder-identifier")
        .args([PARAMETER_PARENT_FOLDER_UUID, PARAMETER_PARENT_FOLDER_PATH])
        .multiple(false)
        .required(true)
}

/// Create tenant identifier group: it must be either --tenant-uuid or --tenant-name
pub fn tenant_identifier_group() -> ArgGroup {
    ArgGroup::new("tenant-identifier")
        .args([PARAMETER_TENANT_UUID, PARAMETER_TENANT_NAME]) // Using PARAMETER_TENANT_UUID for UUID and PARAMETER_TENANT_NAME for name
        .multiple(false)
        .required(true)
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

/// Create the API URL parameter.
pub fn api_url_parameter() -> Arg {
    Arg::new(PARAMETER_API_URL)
        .long(PARAMETER_API_URL)
        .num_args(1)
        .required(false)
        .help("API base URL (e.g., https://app-api.physna.com/v3)")
}

/// Create the UI URL parameter.
pub fn ui_url_parameter() -> Arg {
    Arg::new(PARAMETER_UI_URL)
        .long(PARAMETER_UI_URL)
        .num_args(1)
        .required(false)
        .help("UI base URL (e.g., https://app.physna.com)")
}

/// Create the Auth URL parameter.
pub fn auth_url_parameter() -> Arg {
    Arg::new(PARAMETER_AUTH_URL)
        .long(PARAMETER_AUTH_URL)
        .num_args(1)
        .required(false)
        .help("Authentication URL (e.g., https://physna-app.auth.us-east-2.amazoncognito.com/oauth2/token)")
}

/// Create the continue-on-error parameter.
pub fn continue_on_error_parameter() -> Arg {
    Arg::new(PARAMETER_CONTINUE_ON_ERROR)
        .long(PARAMETER_CONTINUE_ON_ERROR)
        .action(ArgAction::SetTrue)
        .required(false)
        .help("Continue downloading other assets if one fails")
}

/// Create the delay parameter.
pub fn delay_parameter() -> Arg {
    Arg::new(PARAMETER_DELAY)
        .long(PARAMETER_DELAY)
        .num_args(1)
        .required(false)
        .default_value("0")
        .value_parser(|s: &str| -> Result<usize, String> {
            let val: usize = s.parse().map_err(|_| "Must be a number".to_string())?;
            if val > 180 {
                Err("Value must be between 0 and 180".to_string())
            } else {
                Ok(val)
            }
        })
}

/// Create the resume parameter.
pub fn resume_parameter() -> Arg {
    Arg::new(PARAMETER_RESUME)
        .long(PARAMETER_RESUME)
        .action(ArgAction::SetTrue)
        .required(false)
        .help("Resume download by skipping files that already exist in the destination directory")
}

//! CLI command definitions and argument parsing.
//!
//! This module defines all the CLI commands and their arguments using the clap crate.
//! It provides a structured way to define the command-line interface for the Physna CLI.

use crate::format::OutputFormat;
use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

// Resource commands
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

/// Create and configure all CLI commands and their arguments.
///
/// This function defines the entire command-line interface for the Physna CLI,
/// including all subcommands, arguments, and their relationships.
///
/// # Returns
///
/// An `ArgMatches` instance containing the parsed command-line arguments.
pub fn create_cli_commands() -> ArgMatches {
    let format_parameter = Arg::new(PARAMETER_FORMAT)
        .short('f')
        .long(PARAMETER_FORMAT)
        .num_args(1)
        .required(false)
        .default_value("json")
        .global(true)
        .help("Output data format")
        .value_parser(OutputFormat::names());

    let output_file_parameter = Arg::new(PARAMETER_OUTPUT)
        .short('o')
        .long(PARAMETER_OUTPUT)
        .num_args(1)
        .required(false)
        .help("Output file path")
        .value_parser(clap::value_parser!(PathBuf));

    let input_file_parameter = Arg::new(PARAMETER_INPUT)
        .short('i')
        .long(PARAMETER_INPUT)
        .num_args(1)
        .required(false)
        .help("Input file path")
        .value_parser(clap::value_parser!(PathBuf));

    let client_id_parameter = Arg::new(PARAMETER_CLIENT_ID)
        .long(PARAMETER_CLIENT_ID)
        .num_args(1)
        .required(false)
        .help("Client ID for OAuth2 authentication");

    let client_secret_parameter = Arg::new(PARAMETER_CLIENT_SECRET)
        .long(PARAMETER_CLIENT_SECRET)
        .num_args(1)
        .required(false)
        .help("Client secret for OAuth2 authentication");

    let id_parameter = Arg::new(PARAMETER_ID)
        .short('i')
        .long(PARAMETER_ID)
        .num_args(1)
        .required(false)
        .help("Resource ID");
        
    let uuid_parameter = Arg::new(PARAMETER_UUID)
        .short('u')
        .long(PARAMETER_UUID)
        .num_args(1)
        .required(false)
        .help("Resource UUID");

    let _asset_uuid_parameter = Arg::new(PARAMETER_ASSET_UUID)
        .long(PARAMETER_ASSET_UUID)
        .num_args(1)
        .required(false)
        .help("Asset UUID");

    let name_parameter = Arg::new(PARAMETER_NAME)
        .short('n')
        .long(PARAMETER_NAME)
        .num_args(1)
        .required(false)
        .help("Resource name");

    let tenant_parameter = Arg::new(PARAMETER_TENANT)
        .short('t')
        .long(PARAMETER_TENANT)
        .num_args(1)
        .required(false)
        .help("Tenant ID or alias");
        
    
        
    let parent_folder_id_parameter = Arg::new(PARAMETER_PARENT_FOLDER_ID)
        .long(PARAMETER_PARENT_FOLDER_ID)
        .num_args(1)
        .required(false)
        .help("Parent folder ID for creating subfolders");
        
    let path_parameter = Arg::new(PARAMETER_PATH)
        .short('p')
        .long(PARAMETER_PATH)
        .num_args(1)
        .required(false)
        .help("Folder path (e.g., /Root/Child/Grandchild)");

    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Enable verbose output for debugging"),
        )
        .subcommand(
            // Tenant resource commands
            Command::new(COMMAND_TENANT)
                .about("Manage tenants")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_CREATE)
                        .about("Create a new tenant")
                        .arg(name_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Get tenant details")
                        .arg(id_parameter.clone())
                        .arg(format_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_LIST)
                        .about("List all tenants")
                        .visible_alias("ls")
                        .arg(format_parameter.clone().value_parser(["json", "csv"])),
                )
                .subcommand(
                    Command::new(COMMAND_UPDATE)
                        .about("Update tenant configuration")
                        .arg(id_parameter.clone())
                        .arg(name_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_DELETE)
                        .about("Delete a tenant")
                        .visible_alias("rm")
                        .arg(id_parameter.clone()),
                ),
        )
        .subcommand(
            // Folder resource commands
            Command::new(COMMAND_FOLDER)
                .about("Manage folders")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_CREATE)
                        .about("Create a new folder")
                        .arg(tenant_parameter.clone())
                        .arg(name_parameter.clone())
                        .arg(parent_folder_id_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(format_parameter.clone())
                        .group(clap::ArgGroup::new("parent")
                            .args([PARAMETER_PARENT_FOLDER_ID, PARAMETER_PATH])
                            .multiple(false)
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Get folder details")
                        .arg(tenant_parameter.clone())
                        .arg(uuid_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(format_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_LIST)
                        .about("List all folders")
                        .visible_alias("ls")
                        .arg(tenant_parameter.clone())
                        .arg(format_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(
                            Arg::new(PARAMETER_REFRESH)
                                .short('r')
                                .long(PARAMETER_REFRESH)
                                .action(clap::ArgAction::SetTrue)
                                .required(false)
                                .help("Force refresh folder cache data from API"),
                        )
                        .arg(
                            Arg::new("recursive")
                                .short('R')
                                .long("recursive")
                                .action(clap::ArgAction::SetTrue)
                                .required(false)
                                .help("Recursively list all subfolders (default: false for CSV/JSON, true for tree)"),
                        ),
                )
                
                .subcommand(
                    Command::new(COMMAND_DELETE)
                        .about("Delete a folder")
                        .visible_alias("rm")
                        .arg(tenant_parameter.clone())
                        .arg(uuid_parameter.clone())
                        .arg(path_parameter.clone()),
                ),
        )
        .subcommand(
            // Authentication commands
            Command::new(COMMAND_AUTH)
                .about("Authentication operations")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_LOGIN)
                        .about("Login using client credentials")
                        .arg(client_id_parameter.clone())
                        .arg(client_secret_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_LOGOUT)
                        .about("Logout and clear session"),
                )
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Get current access token")
                        .arg(format_parameter.clone().value_parser(["json", "csv"])),
                ),
        )
        .subcommand(
            // Asset commands
            Command::new(COMMAND_ASSET)
                .about("Manage assets")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_CREATE)
                        .about("Create a new asset by uploading a file")
                        .arg(tenant_parameter.clone())
                        .arg(
                            Arg::new("file")
                                .long("file")
                                .num_args(1)
                                .required(true)
                                .help("Path to the file to upload")
                                .value_parser(clap::value_parser!(PathBuf)),
                        )
                        .arg(path_parameter.clone())
                        .arg(format_parameter.clone().value_parser(["json", "csv"])),
                )
                .subcommand(
                    Command::new(COMMAND_CREATE_BATCH)
                        .about("Create multiple assets by uploading files matching a glob pattern")
                        .arg(tenant_parameter.clone())
                        .arg(
                            Arg::new("files")
                                .long("files")
                                .num_args(1)
                                .required(true)
                                .help("Glob pattern to match files to upload (e.g., \"data/puzzle/*.STL\")")
                                .value_parser(clap::value_parser!(String)),
                        )
                        .arg(path_parameter.clone().required(true)) // Make path required for batch operations
                        .arg(format_parameter.clone().value_parser(["json", "csv"]))
                        .arg(
                            Arg::new("concurrent")
                                .long("concurrent")
                                .num_args(1)
                                .required(false)
                                .default_value("5")
                                .help("Maximum number of concurrent uploads")
                                .value_parser(clap::value_parser!(usize)),
                        )
                        .arg(
                            Arg::new("progress")
                                .long("progress")
                                .action(clap::ArgAction::SetTrue)
                                .required(false)
                                .help("Display progress bar during upload"),
                        ),
                )

                .subcommand(
                    Command::new(COMMAND_LIST)
                        .about("List all assets in a folder")
                        .visible_alias("ls")
                        .arg(tenant_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(
                            Arg::new(PARAMETER_REFRESH)
                                .short('r')
                                .long(PARAMETER_REFRESH)
                                .action(clap::ArgAction::SetTrue)
                                .required(false)
                                .help("Force refresh asset cache data from API"),
                        )
                        .arg(
                            Arg::new("metadata")
                                .short('m')
                                .long("metadata")
                                .action(clap::ArgAction::SetTrue)
                                .required(false)
                                .help("Include metadata fields in the output (adds metadata columns for CSV, metadata object for JSON)"),
                        )
                        .arg(format_parameter.clone().value_parser(["json", "csv"])),
                )
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Get asset details")
                        .arg(tenant_parameter.clone())
                        .arg(uuid_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(format_parameter.clone().value_parser(["json", "csv"]))
                        .group(clap::ArgGroup::new("asset_identifier")
                            .args([PARAMETER_UUID, PARAMETER_PATH])
                            .multiple(false)
                            .required(true)
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_DELETE)
                        .about("Delete an asset")
                        .visible_alias("rm")
                        .arg(tenant_parameter.clone())
                        .arg(uuid_parameter.clone())
                        .arg(path_parameter.clone())
                        .group(clap::ArgGroup::new("asset_identifier")
                            .args([PARAMETER_UUID, PARAMETER_PATH])
                            .multiple(false)
                            .required(true)
                        ),
                )
                .subcommand(
                    Command::new("geometric-match")
                        .about("Find geometrically similar assets")
                        .arg(tenant_parameter.clone())
                        .arg(uuid_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(
                            Arg::new("threshold")
                                .long("threshold")
                                .num_args(1)
                                .required(false)
                                .default_value("80.0")
                                .help("Similarity threshold (0.00 to 100.00)")
                                .value_parser(clap::value_parser!(f64)),
                        )
                        .arg(format_parameter.clone().value_parser(["json", "csv"]))
                        .group(clap::ArgGroup::new("reference_asset")
                            .args([PARAMETER_UUID, PARAMETER_PATH])
                            .multiple(false)
                            .required(true)
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_UPDATE)
                        .about("Update an asset's metadata")
                        .arg(tenant_parameter.clone())
                        .arg(uuid_parameter.clone())
                        .arg(path_parameter.clone())
                        .arg(name_parameter.clone())
                        .arg(format_parameter.clone().value_parser(["json", "csv"]))
                        .group(clap::ArgGroup::new("asset_identifier")
                            .args([PARAMETER_UUID, PARAMETER_PATH])
                            .multiple(false)
                            .required(true)
                        ),
                )
                .subcommand(
                    Command::new("geometric-match-folder")
                        .about("Find geometrically similar assets for all assets in a folder")
                        .arg(tenant_parameter.clone())
                        .arg(path_parameter.clone().required(true))
                        .arg(
                            Arg::new("threshold")
                                .long("threshold")
                                .num_args(1)
                                .required(false)
                                .default_value("80.0")
                                .help("Similarity threshold (0.00 to 100.00)")
                                .value_parser(clap::value_parser!(f64)),
                        )
                        .arg(format_parameter.clone().value_parser(["json", "csv"]))
                        .arg(
                            Arg::new("concurrent")
                                .long("concurrent")
                                .num_args(1)
                                .required(false)
                                .default_value("5")
                                .help("Maximum number of concurrent operations")
                                .value_parser(clap::value_parser!(usize)),
                        )
                        .arg(
                            Arg::new("progress")
                                .long("progress")
                                .action(clap::ArgAction::SetTrue)
                                .required(false)
                                .help("Display progress bar during processing"),
                        )
                )
                .subcommand(
                    // Metadata commands - subcommands for managing asset metadata
                    Command::new(COMMAND_METADATA)
                        .about("Manage asset metadata")
                        .subcommand_required(true)
                        .subcommand(
                            Command::new(COMMAND_CREATE)
                                .about("Add metadata to an asset")
                                .arg(tenant_parameter.clone())
                                .arg(uuid_parameter.clone())
                                .arg(path_parameter.clone())
                                .arg(
                                    Arg::new("name")
                                        .long("name")
                                        .num_args(1)
                                        .required(true)
                                        .value_parser(clap::value_parser!(String))
                                        .help("Metadata property name")
                                )
                                .arg(
                                    Arg::new("value")
                                        .long("value")
                                        .num_args(1)
                                        .required(true)
                                        .value_parser(clap::value_parser!(String))
                                        .help("Metadata property value")
                                )
                                .arg(
                                    Arg::new("type")
                                        .long("type")
                                        .num_args(1)
                                        .required(false)
                                        .value_parser(["text", "number", "boolean"])
                                        .default_value("text")
                                        .help("Metadata field type (text, number, boolean) - default: text")
                                )
                                .arg(format_parameter.clone().value_parser(["json", "csv"]))
                                .group(clap::ArgGroup::new("asset_identifier")
                                    .args([PARAMETER_UUID, PARAMETER_PATH])
                                    .multiple(false)
                                    .required(true)
                                ),
                        )
                        .subcommand(
                            Command::new(COMMAND_DELETE)
                                .about("Delete specific metadata fields from an asset")
                                .visible_alias("rm")
                                .arg(tenant_parameter.clone())
                                .arg(uuid_parameter.clone())
                                .arg(path_parameter.clone())
                                .arg(
                                    Arg::new("name")
                                        .short('n')
                                        .long("name")
                                        .num_args(1)
                                        .required(true)
                                        .value_parser(clap::value_parser!(String))
                                        .help("Metadata property name (can be provided multiple times or as comma-separated list)")
                                        .action(clap::ArgAction::Append)
                                )
                                .arg(format_parameter.clone().value_parser(["json", "csv"]))
                                .group(clap::ArgGroup::new("asset_identifier")
                                    .args([PARAMETER_UUID, PARAMETER_PATH])
                                    .multiple(false)
                                    .required(true)
                                ),
                        )
                        .subcommand(
                            Command::new("create-batch")
                                .about("Create metadata for multiple assets from a CSV file")
                                .long_about(
                                    "Create metadata for multiple assets from a CSV file.\n\n\
                                    The CSV file must have the following columns in the specified order:\n\
                                    - ASSET_PATH: The full path of the asset in Physna\n\
                                    - NAME: The name of the metadata field to set\n\
                                    - VALUE: The value to set for the metadata field\n\n\
                                    CSV File Requirements:\n\
                                    - The first row must contain the headers ASSET_PATH,NAME,VALUE\n\
                                    - The file must be UTF-8 encoded\n\
                                    - Values containing commas, quotes, or newlines must be enclosed in double quotes\n\
                                    - Empty rows will be ignored\n\
                                    - Each row represents a single metadata field assignment for an asset\n\n\
                                    If an asset has multiple metadata fields to update, include multiple rows \n\
                                    with the same ASSET_PATH but different NAME and VALUE combinations.\n\n\
                                    Example CSV format:\n\
                                    ASSET_PATH,NAME,VALUE\n\
                                    folder/subfolder/asset1.stl,Material,Steel\n\
                                    folder/subfolder/asset1.stl,Weight,\"15.5 kg\"\n\
                                    folder/subfolder/asset2.ipt,Material,Aluminum\n\n\
                                    The command will group metadata by asset path and update all metadata \
                                    for each asset in a single API call."
                                )
                                .arg(tenant_parameter.clone())
                                .arg(
                                    Arg::new("csv-file")
                                        .long("csv-file")
                                        .num_args(1)
                                        .required(true)
                                        .help("Path to the CSV file containing metadata entries")
                                        .value_parser(clap::value_parser!(PathBuf)),
                                )
                                .arg(format_parameter.clone().value_parser(["json", "csv"]))
                                .arg(
                                    Arg::new("progress")
                                        .long("progress")
                                        .action(clap::ArgAction::SetTrue)
                                        .required(false)
                                        .help("Display progress bar during processing"),
                                ),
                        ),
                ),
        )
        .subcommand(
            // Context commands
            Command::new(COMMAND_CONTEXT)
                .about("Context management")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_SET)
                        .about("Set context")
                        .subcommand(
                            Command::new("tenant")
                                .about("Set active tenant")
                                .arg(name_parameter.clone().required(false))
                                .arg(id_parameter.clone()),
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Get current context")
                        .arg(format_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_CLEAR)
                        .about("Clear context")
                        .subcommand(
                            Command::new("tenant").about("Clear active tenant"),
                        ),
                ),
        )
        .subcommand(
            // Cache commands
            Command::new(COMMAND_CACHE)
                .about("Cache management")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_PURGE)
                        .about("Purge all cached data")
                        .long_about("Delete all cached data including folder hierarchies and asset lists. \
                                    This is useful for clearing stale cache data or preparing for uninstallation."),
                ),
        )
        .subcommand(
            // Configuration commands
            Command::new(COMMAND_CONFIG)
                .about("Configuration management")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_GET)
                        .about("Get configuration details")
                        .arg(format_parameter.clone())
                        .subcommand(Command::new("path").about("Show configuration file path")),
                )
                .subcommand(
                    Command::new(COMMAND_LIST)
                        .about("List configuration")
                        .visible_alias("ls")
                        .arg(format_parameter.clone()),
                )
                .subcommand(
                    Command::new(COMMAND_EXPORT)
                        .about("Export configuration to file")
                        .arg(output_file_parameter),
                )
                .subcommand(
                    Command::new(COMMAND_IMPORT)
                        .about("Import configuration from file")
                        .arg(input_file_parameter),
                ),
        )
        .get_matches()
}

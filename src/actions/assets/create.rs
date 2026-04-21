//! Create asset functionality.
//!
//! This module provides functionality for creating and uploading assets,
//! including batch operations and metadata management.

use crate::{
    actions::folders::resolve_folder_uuid_by_path,
    actions::CliActionError,
    commands::params::{
        PARAMETER_CONTINUE_ON_ERROR, PARAMETER_FILE, PARAMETER_FILES, PARAMETER_FOLDER_PATH,
        PARAMETER_FOLDER_UUID, PARAMETER_PATH, PARAMETER_UUID,
    },
    configuration::Configuration,
    error::CliError,
    error_utils,
    folder_hierarchy::FolderHierarchy,
    format::OutputFormatter,
    metadata::convert_single_metadata_to_json_value,
    model::{Asset, AssetList},
    param_utils::get_format_parameter_value,
    param_utils::get_tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, trace};

/// Cache for asset metadata during batch processing to avoid repeated API calls
struct AssetMetadataCache {
    cache: HashMap<String, Asset>,
}

impl AssetMetadataCache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<&Asset> {
        self.cache.get(key)
    }

    fn insert(&mut self, key: String, asset: Asset) {
        self.cache.insert(key, asset);
    }
}

/// Convert a string value to JSON type
/// For batch operations, we keep all values as strings to avoid type conflicts
/// The API will validate and return clear errors if there's a type mismatch
///
/// # Arguments
/// * `value` - The string value to convert
/// * `_existing_type` - Optional existing field type (currently ignored)
///
/// # Returns
/// JSON value (always String for safety)
fn convert_string_to_json_type(value: &str, _existing_type: Option<&str>) -> Value {
    // Handle empty string - represents "delete" operation
    if value.is_empty() {
        return Value::Null;
    }

    // Keep as string to avoid type conflicts
    // The API handles type validation and provides clear error messages
    Value::String(value.to_string())
}

/// Create a single asset by uploading a file.
///
/// This function handles the "asset create" command, uploading a file
/// to the Physna API and creating a new asset.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was created successfully
/// * `Err(CliError)` - If an error occurred during the creation
pub async fn create_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing file upload...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path - handle specially if needed, but for asset creation we need the actual folder
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Check if the folder exists and set its path
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder = folder;
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned());
    } else {
        // When using --folder-uuid, try to build the folder hierarchy to get the full path
        // This is optional - if it fails, we'll just use the folder name without the full path
        match FolderHierarchy::build_from_api(&mut api, &tenant.uuid).await {
            Ok(hierarchy) => {
                if let Some(path) = hierarchy.get_path_for_folder(&folder_uuid) {
                    folder.set_path(path);
                }
            }
            Err(_) => {
                // If we can't build the hierarchy, just use the folder name as the path
                // This allows the create operation to proceed even if hierarchy fetch fails
                folder.set_path(folder.name());
            }
        }
    }

    let file_path = sub_matches
        .get_one::<PathBuf>(PARAMETER_FILE)
        .ok_or(CliError::MissingRequiredArgument("file".to_string()))?;

    // Extract filename from path for use in asset path construction
    let file_name = file_path
        .file_name()
        .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file path".to_string()))?
        .to_str()
        .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file name".to_string()))?
        .to_string();

    // Construct the full asset path by combining folder path with filename
    let folder_path = folder.path();
    let asset_path = if folder_path.is_empty() || folder_path == "/" {
        file_name.clone()
    } else {
        format!("{}/{}", folder_path, file_name)
    };

    debug!("Creating asset with path: {}", asset_path);

    let asset = api
        .create_asset(&tenant.uuid, file_path, &asset_path, &folder_uuid)
        .await?;
    println!("{}", asset.format(format)?);

    Ok(())
}

/// Create multiple assets in batch from a glob pattern.
///
/// This function handles the "asset create-batch" command, uploading multiple files
/// matching a glob pattern to the Physna API.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the assets were created successfully
/// * `Err(CliError)` - If an error occurred during the creation
pub async fn create_asset_batch(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"create asset batch\" command...");

    let glob_pattern = sub_matches
        .get_one::<String>(PARAMETER_FILES)
        .ok_or(CliError::MissingRequiredArgument("files".to_string()))?
        .clone();
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").unwrap_or(&5);
    let concurrent = *concurrent_param;
    let show_progress = sub_matches.get_flag("progress");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path - handle specially if needed, but for asset creation we need the actual folder
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Check if the folder exists
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder = folder;
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned())
    } else {
        // When using --folder-uuid, we need to build the folder hierarchy to get the full path
        let hierarchy = FolderHierarchy::build_from_api(&mut api, &tenant.uuid).await?;
        if let Some(path) = hierarchy.get_path_for_folder(&folder_uuid) {
            folder.set_path(path);
        }
    }

    let assets = api
        .create_assets_batch(
            &tenant.uuid,
            &glob_pattern,
            Some(folder.path().as_str()),
            Some(&folder_uuid),
            concurrent,
            show_progress,
        )
        .await?;
    println!("{}", AssetList::from(assets).format(format)?);

    Ok(())
}

/// Create metadata for multiple assets from a CSV file.
///
/// This function handles the "asset metadata create-batch" command, which creates or updates
/// metadata for multiple assets from a CSV file.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the metadata was created successfully
/// * `Err(CliError)` - If an error occurred during the creation
pub async fn create_asset_metadata_batch(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"create asset metadata batch\" command...");

    let csv_file_path = sub_matches
        .get_one::<std::path::PathBuf>("csv-file")
        .ok_or(CliError::MissingRequiredArgument("csv-file".to_string()))?;

    let show_progress = sub_matches.get_flag("progress");
    let continue_on_error = sub_matches.get_flag(PARAMETER_CONTINUE_ON_ERROR);

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Read the CSV file and store raw string values
    // Structure: {asset_path: {metadata_name: raw_string_value}}
    let mut raw_asset_metadata: HashMap<String, HashMap<String, String>> = HashMap::new();

    let file = std::fs::File::open(csv_file_path)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
    let mut reader = csv::Reader::from_reader(file);

    for result in reader.records() {
        let record: csv::StringRecord = result
            .map_err(|e| CliError::FormattingError(crate::format::FormattingError::CsvError(e)))?;

        if record.len() >= 3 {
            let asset_path: &str = record[0].trim();
            let metadata_name: &str = record[1].trim();
            let metadata_value: &str = record[2].trim();

            // Keep empty values - they mean "delete existing metadata"
            if metadata_value.is_empty() {
                debug!(
                    "Empty value for metadata field '{}' on asset '{}' (will delete existing)",
                    metadata_name, asset_path
                );
            }

            // Group metadata by asset path (strip leading slash if present)
            let clean_asset_path = asset_path.strip_prefix('/').unwrap_or(asset_path);
            raw_asset_metadata
                .entry(clean_asset_path.to_string())
                .or_default()
                .insert(metadata_name.to_string(), metadata_value.to_string());
        }
    }

    // Pre-flight token expiration check
    const TIME_PER_ASSET_SECONDS: u64 = 5;
    const SAFETY_MARGIN_SECONDS: u64 = 300;

    let time_remaining = api.get_token_time_remaining().unwrap_or(0);
    let estimated_time_needed = (raw_asset_metadata.len() as u64) * TIME_PER_ASSET_SECONDS;

    if time_remaining > 0 && (time_remaining as u64) < estimated_time_needed + SAFETY_MARGIN_SECONDS
    {
        let time_remaining_min = time_remaining / 60;
        eprintln!(
            ":warning: Warning: Token expires in approximately {} minutes, but batch operation may take {} minutes",
            time_remaining_min,
            (estimated_time_needed / 60).max(1)
        );
        eprintln!("  Token will be refreshed automatically if needed during processing.");
        eprintln!();
    }

    // Create in-memory cache for asset metadata to avoid repeated API calls
    let mut asset_cache = AssetMetadataCache::new();

    // Process each asset
    let total_assets = raw_asset_metadata.len();
    let mut current_asset = 0;
    let mut success_count = 0;
    let mut failure_count = 0;
    let mut auth_failure_occurred = false;

    for (asset_path, raw_metadata) in &raw_asset_metadata {
        if show_progress {
            current_asset += 1;
            eprint!(
                "\rProcessing asset {}/{}: {}",
                current_asset, total_assets, asset_path
            );
        }

        // Proactively refresh token if expiring soon
        const TOKEN_REFRESH_THRESHOLD_SECONDS: u64 = 120;
        if let Err(e) = api
            .refresh_token_if_expiring_soon(TOKEN_REFRESH_THRESHOLD_SECONDS)
            .await
        {
            debug!("Proactive token refresh failed: {}", e);
        }

        // Get the asset to read existing metadata types (use cache if available)
        let asset = match asset_cache.get(asset_path) {
            Some(cached) => cached.clone(),
            None => {
                match api.get_asset_by_path(&tenant.uuid, asset_path).await {
                    Ok(asset) => {
                        // Cache the asset for potential reuse
                        asset_cache.insert(asset_path.clone(), asset.clone());
                        asset
                    }
                    Err(e) => {
                        let error_str = format!("{}", e);
                        if error_str.contains("Authentication")
                            || error_str.contains("unauthorized")
                            || error_str.contains("forbidden")
                        {
                            auth_failure_occurred = true;
                            error_utils::report_error_with_remediation(
                                &format!("Authentication failed while looking up asset '{}': {}", asset_path, e),
                                &[
                                    "Your access token may have expired",
                                    "Try running 'pcli2 auth expiration' to check token status",
                                    "Re-authenticate with 'pcli2 auth login' and retry the batch operation",
                                ],
                            );
                            failure_count += 1;
                            break;
                        }

                        error_utils::report_error_with_remediation(
                            &format!("Asset not found: '{}'", asset_path),
                            &[
                                "Verify the asset path in your CSV file matches the actual asset path in Physna",
                                "Check that the asset hasn't been deleted from the system",
                                "Verify you're using the correct tenant for this asset",
                                "Check for path format mismatches (e.g., leading slash differences)",
                                "Verify the asset exists using 'pcli2 asset list --folder-path /' or similar command"
                            ]
                        );
                        failure_count += 1;

                        if continue_on_error {
                            continue;
                        }

                        if show_progress {
                            eprintln!();
                        }
                        eprintln!(
                            "Batch operation stopped: {} successful, {} failed (use --continue-on-error to skip unresolvable asset paths)",
                            success_count, failure_count
                        );
                        return Err(CliError::PhysnaExtendedApiError(e));
                    }
                }
            }
        };

        // Split into fields to delete (empty value) and fields to update (non-empty value)
        let mut fields_to_delete: Vec<String> = Vec::new();
        let mut typed_metadata: HashMap<String, serde_json::Value> = HashMap::new();

        for (field_name, raw_value) in raw_metadata {
            let json_value = convert_string_to_json_type(raw_value, None);
            if json_value.is_null() {
                fields_to_delete.push(field_name.clone());
            } else {
                typed_metadata.insert(field_name.clone(), json_value);
            }
        }

        // Delete fields with empty values
        if !fields_to_delete.is_empty() {
            let keys: Vec<&str> = fields_to_delete.iter().map(|s| s.as_str()).collect();
            if let Err(e) = api
                .delete_asset_metadata(&tenant.uuid.to_string(), &asset.uuid().to_string(), keys)
                .await
            {
                let error_str = format!("{}", e);
                if error_str.contains("Authentication")
                    || error_str.contains("unauthorized")
                    || error_str.contains("forbidden")
                {
                    auth_failure_occurred = true;
                    error_utils::report_error_with_remediation(
                        &format!(
                            "Authentication failed while deleting metadata for asset '{}': {}",
                            asset_path, e
                        ),
                        &[
                            "Your access token may have expired",
                            "Try running 'pcli2 auth expiration' to check token status",
                            "Re-authenticate with 'pcli2 auth login' and retry the batch operation",
                        ],
                    );
                    failure_count += 1;
                    break;
                }
                error_utils::report_error_with_remediation(
                    &format!(
                        "Failed to delete metadata fields for asset '{}': {}",
                        asset_path, e
                    ),
                    &[
                        "Verify the metadata field names exist on this asset",
                        "Check that you have sufficient permissions to modify this asset",
                    ],
                );
                failure_count += 1;
                if show_progress {
                    eprintln!();
                }
                eprintln!(
                    "Batch operation stopped: {} successful, {} failed",
                    success_count, failure_count
                );
                return Err(CliError::PhysnaExtendedApiError(e));
            }
        }

        // Update fields with non-empty values
        if !typed_metadata.is_empty() {
            if let Err(e) = api
                .update_asset_metadata_with_registration(
                    &tenant.uuid,
                    &asset.uuid(),
                    &typed_metadata,
                )
                .await
            {
                let error_str = format!("{}", e);

                if error_str.contains("Authentication")
                    || error_str.contains("unauthorized")
                    || error_str.contains("forbidden")
                {
                    auth_failure_occurred = true;
                    error_utils::report_error_with_remediation(
                        &format!(
                            "Authentication failed while updating metadata for asset '{}': {}",
                            asset_path, e
                        ),
                        &[
                            "Your access token may have expired",
                            "Try running 'pcli2 auth expiration' to check token status",
                            "Re-authenticate with 'pcli2 auth login' and retry the batch operation",
                        ],
                    );
                    failure_count += 1;
                    break;
                }

                if error_str.contains("must be a") || error_str.contains("Metadata type mismatch")
                {
                    error_utils::report_error_with_remediation(
                        &format!("Type conflict for asset '{}': {}", asset_path, e),
                        &[
                            "The metadata field already exists with a different type",
                            "Delete the existing field first if you need to change its type",
                            "Or provide a value that matches the existing field type",
                        ],
                    );
                } else {
                    error_utils::report_error_with_remediation(
                        &format!(
                            "Failed to update metadata for asset '{}': {}",
                            asset_path, e
                        ),
                        &[
                            "Verify metadata field names and values are valid",
                            "Check that you have sufficient permissions to modify this asset",
                            "Verify your network connectivity",
                            "Confirm the asset hasn't been deleted or modified recently",
                        ],
                    );
                }
                failure_count += 1;
                if show_progress {
                    eprintln!();
                }
                eprintln!(
                    "Batch operation stopped: {} successful, {} failed",
                    success_count, failure_count
                );
                return Err(CliError::PhysnaExtendedApiError(e));
            }
        }

        success_count += 1;
    }

    if show_progress {
        eprintln!();
    }

    if show_progress || failure_count > 0 {
        eprintln!(
            "Batch operation completed: {} successful, {} failed",
            success_count, failure_count
        );
    }

    if auth_failure_occurred {
        return Err(CliError::SecurityError(
            "Batch operation stopped due to authentication failure. Please re-authenticate and retry.".to_string(),
        ));
    }

    Ok(())
}

/// Update an asset's metadata with the specified key-value pair.
///
/// This function handles the "asset metadata create" command, which adds or updates
/// metadata for a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the metadata was updated successfully
/// * `Err(CliError)` - If an error occurred during the update
pub async fn update_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Execute \"asset metadata create\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Get metadata parameters from command line
    let metadata_name = sub_matches
        .get_one::<String>("name")
        .ok_or(CliError::MissingRequiredArgument("name".to_string()))?;
    let metadata_value = sub_matches
        .get_one::<String>("value")
        .ok_or(CliError::MissingRequiredArgument("value".to_string()))?;
    let metadata_type = sub_matches
        .get_one::<String>("type")
        .map(|s| s.as_str())
        .unwrap_or("text");

    // Convert the single metadata entry to JSON value using shared function
    let json_value =
        convert_single_metadata_to_json_value(metadata_name, metadata_value, metadata_type);

    // Create a HashMap with the single metadata entry
    // This hashmap represents the desired metadata fields to update
    let mut metadata: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    metadata.insert(metadata_name.clone(), json_value);

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either asset UUID or path must be provided".to_string(),
        ));
    };

    // Update the asset's metadata with automatic registration of new keys
    api.update_asset_metadata_with_registration(&tenant.uuid, &asset.uuid(), &metadata)
        .await?;

    // No output on success (per requirements)

    Ok(())
}

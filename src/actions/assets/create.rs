//! Create asset functionality.
//!
//! This module provides functionality for creating and uploading assets,
//! including batch operations and metadata management.

use crate::{
    actions::folders::resolve_folder_uuid_by_path,
    actions::CliActionError,
    commands::params::{
        PARAMETER_FILE, PARAMETER_FILES, PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID,
        PARAMETER_PATH, PARAMETER_UUID,
    },
    configuration::Configuration,
    error::CliError,
    error_utils,
    folder_hierarchy::FolderHierarchy,
    format::OutputFormatter,
    metadata::convert_single_metadata_to_json_value,
    model::AssetList,
    param_utils::get_format_parameter_value,
    param_utils::get_tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, trace};

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

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Read the CSV file
    let file = std::fs::File::open(csv_file_path)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
    let mut reader = csv::Reader::from_reader(file);

    // Parse the CSV records
    let mut asset_metadata_map: HashMap<String, HashMap<String, serde_json::Value>> =
        HashMap::new();

    for result in reader.records() {
        let record: csv::StringRecord = result
            .map_err(|e| CliError::FormattingError(crate::format::FormattingError::CsvError(e)))?;

        if record.len() >= 3 {
            let asset_path: &str = record[0].trim();
            let metadata_name: &str = record[1].trim();
            let metadata_value: &str = record[2].trim();

            // Use the same conversion logic as individual metadata command (default to text type)
            let json_value = crate::metadata::convert_single_metadata_to_json_value(
                metadata_name, // name parameter (not used in function)
                metadata_value,
                "text", // default to text type since CSV doesn't specify type
            );

            // Group metadata by asset path (strip leading slash if present for consistency with asset paths in system)
            let clean_asset_path = asset_path.strip_prefix('/').unwrap_or(asset_path);
            asset_metadata_map
                .entry(clean_asset_path.to_string())
                .or_default()
                .insert(metadata_name.to_string(), json_value);
        }
    }

    // Process each asset with its metadata
    let total_assets = asset_metadata_map.len();
    let mut current_asset = 0;

    for (asset_path, metadata) in &asset_metadata_map {
        if show_progress {
            current_asset += 1;
            eprint!(
                "\rProcessing asset {}/{}: {}",
                current_asset, total_assets, asset_path
            );
        }

        // Get the asset by the normalized path
        match api.get_asset_by_path(&tenant.uuid, asset_path).await {
            Ok(asset) => {
                // Update the asset's metadata with automatic registration of new keys
                if let Err(e) = api
                    .update_asset_metadata_with_registration(&tenant.uuid, &asset.uuid(), metadata)
                    .await
                {
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
            }
            Err(_e) => {
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
            }
        }
    }

    if show_progress {
        eprintln!(); // New line after progress indicator
    }

    // No output on success (per UNIX best practices)
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

use crate::actions::CliActionError;
use crate::{
    actions::folders::resolve_folder_uuid_by_path,
    commands::params::{
        PARAMETER_FILE, PARAMETER_FILES, PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID,
        PARAMETER_FUZZY, PARAMETER_PATH, PARAMETER_UUID,
    },
    configuration::Configuration,
    error::CliError,
    error_utils,
    format::{CsvRecordProducer, OutputFormatter},
    metadata::convert_single_metadata_to_json_value,
    model::{normalize_path, AssetList, Folder},
    param_utils::{get_format_parameter_value, get_tenant},
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{path::PathBuf, str::FromStr};
use tracing::{debug, trace};
use uuid::Uuid;

pub async fn list_assets(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Listing assets...");

    let format = get_format_parameter_value(sub_matches).await;
    let configuration = Configuration::load_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // If a path is specified, get assets filtered by folder path
    if let Some(path) = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH) {
        trace!("Listing assets for folder path: {}", path);

        let path = normalize_path(path);
        trace!("Normalized folder path: {}", &path);

        let assets = api
            .list_assets_by_parent_folder_path(&tenant.uuid, path.as_str())
            .await?;

        println!("{}", assets.format(format)?);
    };

    Ok(())
}

pub async fn print_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset get\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    // Get format parameters directly from sub_matches since asset get command has all format flags
    let format_str = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .unwrap_or(&"json".to_string())
        .clone();
    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(crate::actions::CliActionError::FormattingError)?;

    let asset_uuid_param = sub_matches
        .get_one::<String>(PARAMETER_UUID)
        .map(|s| Uuid::from_str(s).unwrap());
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param.as_ref(),
        asset_path_param,
    )
    .await?;

    // Format the asset considering the metadata flag
    println!(
        "{}",
        asset.format_with_metadata_flag(format, with_metadata)?
    );

    Ok(())
}

pub async fn print_asset_dependencies(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset dependencies\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    let asset_uuid_param = sub_matches
        .get_one::<String>(PARAMETER_UUID)
        .map(|s| Uuid::from_str(s).unwrap());
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param.as_ref(),
        asset_path_param,
    )
    .await?;

    // Get the full assembly tree with all recursive dependencies
    let assembly_tree = ctx
        .api()
        .get_asset_dependencies_by_path(&tenant_uuid, asset.path().as_str())
        .await?;

    // For tree and JSON formats, output the assembly tree directly to preserve hierarchy
    if matches!(format, crate::format::OutputFormat::Tree(_))
        || matches!(format, crate::format::OutputFormat::Json(_))
    {
        println!("{}", assembly_tree.format(format)?);
    } else {
        // For other formats (CSV), extract all dependencies from the full tree structure
        let all_dependencies = extract_all_dependencies_from_tree(&assembly_tree);

        // Create an AssetDependencyList from the response to format properly
        let dependency_list = crate::model::AssetDependencyList {
            path: asset.path().to_string(),
            dependencies: all_dependencies,
        };

        println!("{}", dependency_list.format(format)?);
    }

    Ok(())
}

// Helper function to extract all dependencies from AssemblyTree recursively
fn extract_all_dependencies_from_tree(
    assembly_tree: &crate::model::AssemblyTree,
) -> Vec<crate::model::AssetDependency> {
    let mut all_dependencies = Vec::new();

    // Process all nodes in the tree recursively, starting with the root assembly name as the parent path
    let root_name = assembly_tree.root().asset().name();
    collect_dependencies_recursive(assembly_tree.root(), &mut all_dependencies, root_name);

    all_dependencies
}

// Recursive helper to collect all dependencies with assembly path tracking
fn collect_dependencies_recursive(
    node: &crate::model::AssemblyNode,
    dependencies: &mut Vec<crate::model::AssetDependency>,
    parent_assembly_path: String,
) {
    for child in node.children() {
        // Calculate the assembly path for this child
        let child_name = child.asset().name();
        let current_assembly_path = if parent_assembly_path.is_empty() {
            child_name.clone()
        } else {
            format!("{}/{}", parent_assembly_path, child_name)
        };

        // Create an AssetResponse from the child asset
        let asset_response = crate::model::AssetResponse {
            uuid: child.asset().uuid(),
            tenant_id: Uuid::nil(), // Placeholder - would need actual tenant ID if available
            path: child.asset().path(),
            folder_id: None,
            asset_type: child
                .asset()
                .file_type()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            created_at: child.asset().created_at().cloned().unwrap_or_default(),
            updated_at: child.asset().updated_at().cloned().unwrap_or_default(),
            state: child
                .asset()
                .processing_status()
                .cloned()
                .unwrap_or_else(|| "missing".to_string()),
            is_assembly: child.has_children(),
            metadata: std::collections::HashMap::new(), // Empty metadata
            parent_folder_id: None,
            owner_id: None,
        };

        // Create AssetDependency from the child
        let asset_dependency = crate::model::AssetDependency {
            path: child.asset().path(),
            asset: Some(asset_response),
            occurrences: 1, // Default occurrence count
            has_dependencies: child.has_children(),
            assembly_path: current_assembly_path.clone(), // Clone to use in both places
            original_asset_path: None, // This will be set when processing folder dependencies
        };

        // Store the assembly path in a way that can be accessed later
        // We'll store it in the AssetResponse's path field or use a custom field
        // Actually, let's extend the AssetDependency to include assembly path info
        // For now, we'll need to modify how we handle this in the CSV formatter

        dependencies.push(asset_dependency);

        // Recursively process children of this child with updated assembly path
        collect_dependencies_recursive(child, dependencies, current_assembly_path);
    }
}

pub async fn create_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing file upload...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
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
    let mut folder: Folder = folder.into();
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned())
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
    let asset_path = if let Some(folder_path) = folder_path_param {
        if folder_path.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", folder_path, file_name)
        }
    } else {
        file_name.clone()
    };

    debug!("Creating asset with path: {}", asset_path);

    let asset = api
        .create_asset(&tenant.uuid, file_path, &asset_path, &folder_uuid)
        .await?;
    println!("{}", asset.format(format)?);

    Ok(())
}

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
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
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
    let mut folder: Folder = folder.into();
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned())
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

use std::collections::HashMap;

/// Create metadata for multiple assets from a CSV file.
///
/// This function handles the "asset metadata create-batch" command, which creates or updates
/// metadata for multiple assets from a CSV file.
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

    let asset_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
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

pub async fn print_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    let asset_uuid_param = sub_matches
        .get_one::<String>(PARAMETER_UUID)
        .map(|s| Uuid::from_str(s).unwrap());
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param.as_ref(),
        asset_path_param,
    )
    .await?;

    match asset.metadata() {
        Some(metadata) => println!("{}", metadata.format(format)?),
        None => (),
    };

    Ok(())
}

/// Delete an asset by UUID or path.
///
/// This function handles the "asset delete" command, removing a specific asset
/// identified by either its UUID or path from the Physna API.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was deleted successfully
/// * `Err(CliError)` - If an error occurred during deletion
pub async fn delete_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset delete\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Delete the asset
    ctx.api()
        .delete_asset(&tenant_uuid.to_string(), &asset.uuid().to_string())
        .await?;

    Ok(())
}

/// Download an asset by UUID or path to a local file.
///
/// This function handles the "asset download" command, retrieving a specific asset
/// identified by either its UUID or path from the Physna API and saving it to a local file.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was downloaded successfully
/// * `Err(CliError)` - If an error occurred during download
pub async fn download_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset download\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_FILE)
    {
        output_path.clone()
    } else {
        // Use the asset name as the default output file name
        // For assemblies, we still use the original name since we'll extract the ZIP contents
        let asset_name = asset.name();

        let mut path = std::path::PathBuf::new();
        path.push(asset_name);
        path
    };

    // Download the asset file with retry logic
    let file_content = download_asset_with_retry(
        ctx.api(),
        &tenant_uuid.to_string(),
        &asset.uuid().to_string(),
    )
    .await?;

    // Write the file content to the output file
    std::fs::write(&output_file_path, file_content).map_err(CliActionError::IoError)?;

    // If the asset is an assembly, the downloaded file is a ZIP file that needs to be extracted
    if asset.is_assembly() {
        extract_zip_and_cleanup(&output_file_path)?;
    }

    Ok(())
}

pub async fn download_asset_thumbnail(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset thumbnail\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_FILE)
    {
        output_path.clone()
    } else {
        // Use the asset name as the default output file name with .png extension
        let asset_name = asset.name();

        // Get the stem of the asset name (without extension) and add .png
        let path_stem = std::path::Path::new(&asset_name)
            .file_stem()
            .unwrap_or(std::ffi::OsStr::new(&asset_name))
            .to_string_lossy()
            .to_string();

        let mut path = std::path::PathBuf::new();
        path.push(format!("{}.png", path_stem));
        path
    };

    // Download the asset thumbnail
    let thumbnail_content = ctx
        .api()
        .download_asset_thumbnail(&tenant_uuid.to_string(), &asset.uuid().to_string())
        .await
        .map_err(CliActionError::ApiError)?;

    // Write the thumbnail content to the output file
    std::fs::write(&output_file_path, thumbnail_content).map_err(CliActionError::IoError)?;

    Ok(())
}

fn extract_zip_and_cleanup(zip_path: &std::path::PathBuf) -> Result<(), CliError> {
    use std::io::Cursor;

    // Read the ZIP file content
    let zip_content = std::fs::read(zip_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Create a cursor from the content
    let cursor = Cursor::new(zip_content);

    // Create a ZipArchive from the cursor
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;

    // Extract all files to the same directory as the ZIP file
    let parent_dir = zip_path.parent().ok_or_else(|| {
        CliError::ActionError(crate::actions::CliActionError::IoError(
            std::io::Error::other("Could not get parent directory"),
        ))
    })?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;

        let file_path = parent_dir.join(file.mangled_name());

        if file.is_dir() {
            std::fs::create_dir_all(&file_path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;
        } else {
            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    CliError::ActionError(crate::actions::CliActionError::IoError(e))
                })?;
            }

            let mut output_file = std::fs::File::create(&file_path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            std::io::copy(&mut file, &mut output_file)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;
        }
    }

    // Remove the original ZIP file after successful extraction
    std::fs::remove_file(zip_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    Ok(())
}

pub async fn geometric_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing geometric match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters directly from sub_matches since geometric match commands have all format flags
    let format_str = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .unwrap();

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Perform geometric search
    let mut search_results = ctx
        .api()
        .geometric_search(&tenant_uuid, &asset.uuid(), threshold)
        .await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let comparison_url = if base_url.ends_with("/tenants") {
            format!(
                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.match_percentage
            )
        } else {
            format!(
                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.match_percentage
            )
        };
        match_result.comparison_url = Some(comparison_url);
    }

    // Create a basic AssetResponse from the asset for the reference
    let metadata_map = if let Some(asset_metadata) = asset.metadata() {
        // Convert AssetMetadata to HashMap<String, serde_json::Value>
        let mut map = std::collections::HashMap::new();
        for key in asset_metadata.keys() {
            if let Some(value) = asset_metadata.get(key) {
                map.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let reference_asset_response = crate::model::AssetResponse {
        uuid: asset.uuid(),
        tenant_id: tenant_uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,  // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedGeometricSearchResponse {
        reference_asset: reference_asset_response,
        matches: search_results.matches,
    };

    println!("{}", enhanced_response.format(format)?);

    Ok(())
}

pub async fn part_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing part match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters directly from sub_matches since part match commands have all format flags
    let format_str = if let Some(format_val) =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
    {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string()
        }
    };

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Perform part search
    let mut search_results = ctx
        .api()
        .part_search(&tenant_uuid, &asset.uuid(), threshold)
        .await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let comparison_url = if base_url.ends_with("/tenants") {
            format!(
                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.forward_match_percentage.unwrap_or(0.0),
                match_result.reverse_match_percentage.unwrap_or(0.0)
            )
        } else {
            format!(
                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.forward_match_percentage.unwrap_or(0.0),
                match_result.reverse_match_percentage.unwrap_or(0.0)
            )
        };
        match_result.comparison_url = Some(comparison_url);
    }

    // Create a basic AssetResponse from the asset for the reference
    let metadata_map = if let Some(asset_metadata) = asset.metadata() {
        // Convert AssetMetadata to HashMap<String, serde_json::Value>
        let mut map = std::collections::HashMap::new();
        for key in asset_metadata.keys() {
            if let Some(value) = asset_metadata.get(key) {
                map.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let reference_asset_response = crate::model::AssetResponse {
        uuid: asset.uuid(),
        tenant_id: tenant_uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,  // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedPartSearchResponse {
        reference_asset: reference_asset_response,
        matches: search_results.matches,
    };

    // Format the response considering the metadata flag
    println!(
        "{}",
        enhanced_response.format_with_metadata_flag(format, with_metadata)?
    );

    Ok(())
}

pub async fn geometric_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing geometric match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument(
            crate::commands::params::PARAMETER_FOLDER_PATH.to_string(),
        ))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters
    let format_str = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .unwrap_or(&"json".to_string())
        .clone();

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if !(1..=10).contains(&val) {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Invalid value for '--concurrent': must be between 1 and 10, got {}",
                    val
                )));
            }
            val
        }
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api
            .list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str())
            .await?;

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        error_utils::report_error_with_remediation(
            &"No assets found in the specified folder(s)",
            &[
                "Verify the folder path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the specified folder(s)",
            ],
        );
        return Ok(());
    }

    // Create multi-progress bar if show_progress is true
    let multi_progress = if show_progress {
        let mp = MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(ProgressBar::new(all_assets.len() as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Overall Progress")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent));

    // Prepare for concurrent processing
    let mut all_matches = Vec::new();

    // Use a set to track unique pairs to avoid duplicates (reference UUID, candidate UUID)
    let mut seen_pairs = std::collections::HashSet::new();

    // Create tasks for concurrent processing
    type TaskResult = Result<
        Vec<crate::model::EnhancedGeometricSearchResponse>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();
    for (asset_uuid, asset) in &all_assets {
        let semaphore = semaphore.clone();
        let mut api_clone = api.clone(); // Clone the API client
        let tenant_uuid = tenant.uuid;
        let asset_uuid = *asset_uuid;
        let asset_clone = asset.clone();
        let folder_paths_clone = folder_paths.clone();
        let tenant_clone = tenant.clone();
        let multi_progress_clone = multi_progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this task if multi-progress is enabled
            let individual_pb = if let Some((ref mp, _)) = multi_progress_clone {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "{{spinner:.green}} Processing: {} {{msg}}",
                            asset_clone.name()
                        ))
                        .unwrap(),
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting geometric search...");
            }

            let result = match api_clone
                .geometric_search(&tenant_uuid, &asset_uuid, threshold)
                .await
            {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!(
                            "Processing {} matches...",
                            search_results.matches.len()
                        ));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration =
                            crate::configuration::Configuration::load_or_create_default().map_err(
                                |e| {
                                    CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e),
                                }
                            )
                                },
                            )?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let base_url = ui_base_url.trim_end_matches('/');
                        let comparison_url = if base_url.ends_with("/tenants") {
                            format!(
                                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.match_percentage
                            )
                        } else {
                            format!(
                                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.match_percentage
                            )
                        };
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        // For exclusive mode, both reference and candidate assets must be in specified folders
                        let candidate_in_specified_folders = folder_paths_clone
                            .iter()
                            .any(|folder_path| {
                                let normalized_folder_path = crate::model::normalize_path(folder_path);
                                let normalized_candidate_path = crate::model::normalize_path(&match_result.asset.path);
                                normalized_candidate_path.starts_with(&normalized_folder_path)
                            });
                        
                        let reference_in_specified_folders = folder_paths_clone
                            .iter()
                            .any(|folder_path| {
                                let normalized_folder_path = crate::model::normalize_path(folder_path);
                                let normalized_reference_path = crate::model::normalize_path(asset_clone.path());
                                normalized_reference_path.starts_with(&normalized_folder_path)
                            });

                        if exclusive && (!candidate_in_specified_folders || !reference_in_specified_folders) {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
                            // Convert AssetMetadata to HashMap<String, serde_json::Value>
                            let mut map = std::collections::HashMap::new();
                            for key in asset_metadata.keys() {
                                if let Some(value) = asset_metadata.get(key) {
                                    map.insert(
                                        key.clone(),
                                        serde_json::Value::String(value.clone()),
                                    );
                                }
                            }
                            map
                        } else {
                            std::collections::HashMap::new()
                        };

                        let reference_asset_response = crate::model::AssetResponse {
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(),      // Placeholder for creation time
                            updated_at: "".to_string(),      // Placeholder for update time
                            state: "active".to_string(),     // Default state
                            is_assembly: false,              // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None,         // No owner ID
                        };

                        let enhanced_match = crate::model::EnhancedGeometricSearchResponse {
                            reference_asset: reference_asset_response,
                            matches: vec![match_result.clone()],
                        };

                        asset_matches.push(enhanced_match);
                    }

                    // Update progress bar to show completion
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Found {} matches", asset_matches.len()));
                    }

                    Ok(asset_matches)
                }
                Err(e) => {
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform geometric search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    Ok(Vec::new()) // Return empty vector on error
                }
            };

            // Remove the individual progress bar when done
            if let Some(pb) = individual_pb {
                pb.finish_and_clear();
            }

            result
        });

        tasks.push(task);
    }

    // Process tasks and collect results
    for task in tasks {
        match task.await {
            Ok(Ok(asset_matches)) => {
                for enhanced_match in asset_matches {
                    // Apply duplicate filtering to each match
                    for match_result in &enhanced_match.matches {
                        // Create a unique pair identifier to avoid duplicates
                        // We want to avoid having both (A,B) and (B,A) in results
                        let (ref_uuid, cand_uuid) =
                            if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
                                (enhanced_match.reference_asset.uuid, match_result.asset.uuid)
                            } else {
                                (match_result.asset.uuid, enhanced_match.reference_asset.uuid)
                            };

                        let pair_key = (ref_uuid, cand_uuid);

                        if !seen_pairs.contains(&pair_key) {
                            seen_pairs.insert(pair_key);
                            all_matches.push(enhanced_match.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error_utils::report_error_with_remediation(
                    &format!("Error processing asset: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify the asset exists and is accessible",
                        "Retry the operation",
                    ],
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation",
                    ],
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!(
            "Processed {} assets. Found {} unique matches.",
            all_assets.len(),
            all_matches.len()
        ));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(
                        crate::model::GeometricMatchPair::from_reference_and_match(
                            enhanced_response.reference_asset.clone(),
                            match_result,
                        ),
                    );
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(
                        crate::model::GeometricMatchPair::from_reference_and_match(
                            enhanced_response.reference_asset.clone(),
                            match_result,
                        ),
                    );
                }
            }

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv::Writer::from_writer(vec![]);
            let output;

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &flattened_matches {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::GeometricMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    format!("{}", match_pair.match_percentage),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(
                        crate::model::GeometricMatchPair::from_reference_and_match(
                            enhanced_response.reference_asset.clone(),
                            match_result,
                        ),
                    );
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

    Ok(())
}

pub async fn part_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing part match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument(
            crate::commands::params::PARAMETER_FOLDER_PATH.to_string(),
        ))?
        .cloned()
        .collect();

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters
    let format_str = if let Some(format_val) =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
    {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string()
        }
    };

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if !(1..=10).contains(&val) {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Invalid value for '--concurrent': must be between 1 and 10, got {}",
                    val
                )));
            }
            val
        }
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api
            .list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str())
            .await?;

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        error_utils::report_error_with_remediation(
            &"No assets found in the specified folder(s)",
            &[
                "Verify the folder path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the specified folder(s)",
            ],
        );
        return Ok(());
    }

    // Create multi-progress bar if show_progress is true
    let multi_progress = if show_progress {
        let mp = MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(ProgressBar::new(all_assets.len() as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Overall Progress")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent));

    // Prepare for concurrent processing
    let mut all_matches = Vec::new();

    // Use a set to track unique pairs to avoid duplicates (reference UUID, candidate UUID)
    let mut seen_pairs = std::collections::HashSet::new();

    // Create tasks for concurrent processing
    type TaskResult = Result<
        Vec<crate::model::EnhancedPartSearchResponse>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();
    for (asset_uuid, asset) in &all_assets {
        let semaphore = semaphore.clone();
        let mut api_clone = api.clone(); // Clone the API client
        let tenant_uuid = tenant.uuid;
        let asset_uuid = *asset_uuid;
        let asset_clone = asset.clone();
        let folder_paths_clone = folder_paths.clone();
        let tenant_clone = tenant.clone();
        let multi_progress_clone = multi_progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this task if multi-progress is enabled
            let individual_pb = if let Some((ref mp, _)) = multi_progress_clone {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "{{spinner:.green}} Processing: {} {{msg}}",
                            asset_clone.name()
                        ))
                        .unwrap(),
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting part search...");
            }

            let result = match api_clone
                .part_search(&tenant_uuid, &asset_uuid, threshold)
                .await
            {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!(
                            "Processing {} matches...",
                            search_results.matches.len()
                        ));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration =
                            crate::configuration::Configuration::load_or_create_default().map_err(
                                |e| {
                                    CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e),
                                }
                            )
                                },
                            )?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let base_url = ui_base_url.trim_end_matches('/');
                        let comparison_url = if base_url.ends_with("/tenants") {
                            format!(
                                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.forward_match_percentage.unwrap_or(0.0),
                                match_result.reverse_match_percentage.unwrap_or(0.0)
                            )
                        } else {
                            format!(
                                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.forward_match_percentage.unwrap_or(0.0),
                                match_result.reverse_match_percentage.unwrap_or(0.0)
                            )
                        };
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        // For exclusive mode, both reference and candidate assets must be in specified folders
                        let candidate_in_specified_folders = folder_paths_clone
                            .iter()
                            .any(|folder_path| {
                                let normalized_folder_path = crate::model::normalize_path(folder_path);
                                let normalized_candidate_path = crate::model::normalize_path(&match_result.asset.path);
                                normalized_candidate_path.starts_with(&normalized_folder_path)
                            });
                        
                        let reference_in_specified_folders = folder_paths_clone
                            .iter()
                            .any(|folder_path| {
                                let normalized_folder_path = crate::model::normalize_path(folder_path);
                                let normalized_reference_path = crate::model::normalize_path(asset_clone.path());
                                normalized_reference_path.starts_with(&normalized_folder_path)
                            });

                        if exclusive && (!candidate_in_specified_folders || !reference_in_specified_folders) {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
                            // Convert AssetMetadata to HashMap<String, serde_json::Value>
                            let mut map = std::collections::HashMap::new();
                            for key in asset_metadata.keys() {
                                if let Some(value) = asset_metadata.get(key) {
                                    map.insert(
                                        key.clone(),
                                        serde_json::Value::String(value.clone()),
                                    );
                                }
                            }
                            map
                        } else {
                            std::collections::HashMap::new()
                        };

                        let reference_asset_response = crate::model::AssetResponse {
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(),      // Placeholder for creation time
                            updated_at: "".to_string(),      // Placeholder for update time
                            state: "active".to_string(),     // Default state
                            is_assembly: false,              // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None,         // No owner ID
                        };

                        let enhanced_match = crate::model::EnhancedPartSearchResponse {
                            reference_asset: reference_asset_response,
                            matches: vec![match_result.clone()],
                        };

                        asset_matches.push(enhanced_match);
                    }

                    // Update progress bar to show completion
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Found {} matches", asset_matches.len()));
                    }

                    Ok(asset_matches)
                }
                Err(e) => {
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform part search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    Ok(Vec::new()) // Return empty vector on error
                }
            };

            // Remove the individual progress bar when done
            if let Some(pb) = individual_pb {
                pb.finish_and_clear();
            }

            result
        });

        tasks.push(task);
    }

    // Process tasks and collect results
    for task in tasks {
        match task.await {
            Ok(Ok(asset_matches)) => {
                for enhanced_match in asset_matches {
                    // Apply duplicate filtering to each match
                    for match_result in &enhanced_match.matches {
                        // Create a unique pair identifier to avoid duplicates
                        // We want to avoid having both (A,B) and (B,A) in results
                        let (ref_uuid, cand_uuid) =
                            if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
                                (enhanced_match.reference_asset.uuid, match_result.asset.uuid)
                            } else {
                                (match_result.asset.uuid, enhanced_match.reference_asset.uuid)
                            };

                        let pair_key = (ref_uuid, cand_uuid);

                        if !seen_pairs.contains(&pair_key) {
                            seen_pairs.insert(pair_key);
                            all_matches.push(enhanced_match.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error_utils::report_error_with_remediation(
                    &format!("Error processing asset: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify the asset exists and is accessible",
                        "Retry the operation",
                    ],
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation",
                    ],
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!(
            "Processed {} assets. Found {} unique matches.",
            all_assets.len(),
            all_matches.len()
        ));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::PartMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result,
                    ));
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::PartMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result,
                    ));
                }
            }

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv::Writer::from_writer(vec![]);
            let output;

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &flattened_matches {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::PartMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair
                        .forward_match_percentage
                        .map_or_else(|| "0.0".to_string(), |val| format!("{}", val)),
                    match_pair
                        .reverse_match_percentage
                        .map_or_else(|| "0.0".to_string(), |val| format!("{}", val)),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::PartMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result,
                    ));
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

    Ok(())
}

pub async fn visual_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing visual match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get format parameters directly from sub_matches since visual match commands have all format flags
    let format_str = if let Some(format_val) =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
    {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string()
        }
    };

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Perform visual search
    let mut search_results = ctx.api().visual_search(&tenant_uuid, &asset.uuid()).await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let comparison_url = if base_url.ends_with("/tenants") {
            format!(
                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                base_url,    // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
            )
        } else {
            format!(
                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
            )
        };
        match_result.comparison_url = Some(comparison_url);
    }

    // Create a basic AssetResponse from the asset for the reference
    let metadata_map = if let Some(asset_metadata) = asset.metadata() {
        // Convert AssetMetadata to HashMap<String, serde_json::Value>
        let mut map = std::collections::HashMap::new();
        for key in asset_metadata.keys() {
            if let Some(value) = asset_metadata.get(key) {
                map.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let reference_asset_response = crate::model::AssetResponse {
        uuid: asset.uuid(),
        tenant_id: tenant_uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,  // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    // Create visual match pairs that exclude match percentages since visual search doesn't have them
    let visual_match_pairs: Vec<crate::model::VisualMatchPair> = search_results
        .matches
        .into_iter()
        .map(|match_result| crate::model::VisualMatchPair {
            reference_asset: reference_asset_response.clone(),
            candidate_asset: match_result.asset,
            comparison_url: match_result.comparison_url,
        })
        .collect();

    // Format the response based on the output format
    match format {
        crate::format::OutputFormat::Json(_) => {
            println!("{}", serde_json::to_string_pretty(&visual_match_pairs)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            let mut wtr = csv::Writer::from_writer(vec![]);

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &visual_match_pairs {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::VisualMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in &visual_match_pairs {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            let output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            println!("{}", serde_json::to_string_pretty(&visual_match_pairs)?);
        }
    }

    Ok(())
}

pub async fn visual_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing visual match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument(
            crate::commands::params::PARAMETER_FOLDER_PATH.to_string(),
        ))?
        .cloned()
        .collect();

    // Get format parameters
    let format_str = if let Some(format_val) =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
    {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string()
        }
    };

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if !(1..=10).contains(&val) {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Invalid value for '--concurrent': must be between 1 and 10, got {}",
                    val
                )));
            }
            val
        }
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api
            .list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str())
            .await?;

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        error_utils::report_error_with_remediation(
            &"No assets found in the specified folder(s)",
            &[
                "Verify the folder path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the specified folder(s)",
            ],
        );
        return Ok(());
    }

    // Create multi-progress bar if show_progress is true
    let multi_progress = if show_progress {
        let mp = MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(ProgressBar::new(all_assets.len() as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Overall Progress")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent));

    // Prepare for concurrent processing
    let mut all_matches = Vec::new();

    // Use a set to track unique pairs to avoid duplicates (reference UUID, candidate UUID)
    let mut seen_pairs = std::collections::HashSet::new();

    // Create tasks for concurrent processing
    type TaskResult = Result<
        Vec<crate::model::EnhancedPartSearchResponse>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();
    for (asset_uuid, asset) in &all_assets {
        let semaphore = semaphore.clone();
        let mut api_clone = api.clone(); // Clone the API client
        let tenant_uuid = tenant.uuid;
        let asset_uuid = *asset_uuid;
        let asset_clone = asset.clone();
        let folder_paths_clone = folder_paths.clone();
        let tenant_clone = tenant.clone();
        let multi_progress_clone = multi_progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this task if multi-progress is enabled
            let individual_pb = if let Some((ref mp, _)) = multi_progress_clone {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "{{spinner:.green}} Processing: {} {{msg}}",
                            asset_clone.name()
                        ))
                        .unwrap(),
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting visual search...");
            }

            let result = match api_clone.visual_search(&tenant_uuid, &asset_uuid).await {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!(
                            "Processing {} matches...",
                            search_results.matches.len()
                        ));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration =
                            crate::configuration::Configuration::load_or_create_default().map_err(
                                |e| {
                                    CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e),
                                }
                            )
                                },
                            )?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let base_url = ui_base_url.trim_end_matches('/');
                        let comparison_url = if base_url.ends_with("/tenants") {
                            format!(
                                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                            )
                        } else {
                            format!(
                                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                            )
                        };
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        // For exclusive mode, both reference and candidate assets must be in specified folders
                        let candidate_in_specified_folders = folder_paths_clone
                            .iter()
                            .any(|folder_path| {
                                let normalized_folder_path = crate::model::normalize_path(folder_path);
                                let normalized_candidate_path = crate::model::normalize_path(&match_result.asset.path);
                                normalized_candidate_path.starts_with(&normalized_folder_path)
                            });
                        
                        let reference_in_specified_folders = folder_paths_clone
                            .iter()
                            .any(|folder_path| {
                                let normalized_folder_path = crate::model::normalize_path(folder_path);
                                let normalized_reference_path = crate::model::normalize_path(asset_clone.path());
                                normalized_reference_path.starts_with(&normalized_folder_path)
                            });

                        if exclusive && (!candidate_in_specified_folders || !reference_in_specified_folders) {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
                            // Convert AssetMetadata to HashMap<String, serde_json::Value>
                            let mut map = std::collections::HashMap::new();
                            for key in asset_metadata.keys() {
                                if let Some(value) = asset_metadata.get(key) {
                                    map.insert(
                                        key.clone(),
                                        serde_json::Value::String(value.clone()),
                                    );
                                }
                            }
                            map
                        } else {
                            std::collections::HashMap::new()
                        };

                        let reference_asset_response = crate::model::AssetResponse {
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(),      // Placeholder for creation time
                            updated_at: "".to_string(),      // Placeholder for update time
                            state: "active".to_string(),     // Default state
                            is_assembly: false,              // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None,         // No owner ID
                        };

                        let enhanced_match = crate::model::EnhancedPartSearchResponse {
                            reference_asset: reference_asset_response,
                            matches: vec![match_result.clone()],
                        };

                        asset_matches.push(enhanced_match);
                    }

                    // Update progress bar to show completion
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Found {} matches", asset_matches.len()));
                    }

                    Ok(asset_matches)
                }
                Err(e) => {
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform visual search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    Ok(Vec::new()) // Return empty vector on error
                }
            };

            // Remove the individual progress bar when done
            if let Some(pb) = individual_pb {
                pb.finish_and_clear();
            }

            result
        });

        tasks.push(task);
    }

    // Process tasks and collect results
    for task in tasks {
        match task.await {
            Ok(Ok(asset_matches)) => {
                for enhanced_match in asset_matches {
                    // Apply duplicate filtering to each match
                    for match_result in &enhanced_match.matches {
                        // Create a unique pair identifier to avoid duplicates
                        // We want to avoid having both (A,B) and (B,A) in results
                        let (ref_uuid, cand_uuid) =
                            if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
                                (enhanced_match.reference_asset.uuid, match_result.asset.uuid)
                            } else {
                                (match_result.asset.uuid, enhanced_match.reference_asset.uuid)
                            };

                        let pair_key = (ref_uuid, cand_uuid);

                        if !seen_pairs.contains(&pair_key) {
                            seen_pairs.insert(pair_key);
                            all_matches.push(enhanced_match.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error_utils::report_error_with_remediation(
                    &format!("Error processing asset: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify the asset exists and is accessible",
                        "Retry the operation",
                    ],
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation",
                    ],
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!(
            "Processed {} assets. Found {} unique matches.",
            all_assets.len(),
            all_matches.len()
        ));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::VisualMatchPair {
                        reference_asset: enhanced_response.reference_asset.clone(),
                        candidate_asset: match_result.asset,
                        comparison_url: match_result.comparison_url,
                    });
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::VisualMatchPair {
                        reference_asset: enhanced_response.reference_asset.clone(),
                        candidate_asset: match_result.asset,
                        comparison_url: match_result.comparison_url,
                    });
                }
            }

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv::Writer::from_writer(vec![]);
            let output;

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &flattened_matches {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::VisualMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::VisualMatchPair {
                        reference_asset: enhanced_response.reference_asset.clone(),
                        candidate_asset: match_result.asset,
                        comparison_url: match_result.comparison_url,
                    });
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

    Ok(())
}

use std::fs::File;
use std::io::Write;
use zip::write::FileOptions;
use zip::ZipWriter;

/// Reprocess an asset by UUID or path.
///
/// This function handles the "asset reprocess" command, triggering reprocessing
/// of a specific asset identified by either its UUID or path in the Physna API.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was reprocessed successfully
/// * `Err(CliError)` - If an error occurred during reprocessing
pub async fn reprocess_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset reprocess\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Trigger reprocessing of the asset
    ctx.api()
        .reprocess_asset(&tenant_uuid, &asset.uuid())
        .await?;

    // No output on success (following UNIX convention)
    Ok(())
}

/// Download all assets in a folder as a ZIP archive.
///
/// This function handles the "asset download-folder" command, retrieving all assets
/// in a specified folder from the Physna API and packaging them into a ZIP file.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the folder was downloaded successfully
/// * `Err(CliError)` - If an error occurred during download
pub async fn download_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset download-folder\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder UUID or path from command line
    let folder_uuid_param =
        sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_FOLDER_UUID);
    let folder_path_param =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        // Resolve folder UUID by path
        crate::actions::folders::resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_FILE)
    {
        output_path.clone()
    } else {
        // Use the folder name as the default output file name
        // Determine the folder name from the provided path or get it from the folder details
        let folder_name = if let Some(path) = folder_path_param {
            // If the folder was specified by path, extract the folder name from the path
            let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if path_segments.is_empty() {
                "untitled".to_string()
            } else {
                path_segments.last().unwrap().to_string()
            }
        } else {
            // If the folder was specified by UUID, get the folder details to determine the name
            let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
            let folder: crate::model::Folder = folder.into();

            let folder_path = folder.path();
            let path_segments: Vec<&str> =
                folder_path.split('/').filter(|s| !s.is_empty()).collect();
            if path_segments.is_empty() {
                "untitled".to_string()
            } else {
                path_segments.last().unwrap().to_string()
            }
        };

        let mut path = std::path::PathBuf::new();
        path.push(format!("{}.zip", folder_name));
        path
    };

    // Get all assets in the folder
    let assets_response = api
        .list_assets_by_parent_folder_uuid(&tenant.uuid, Some(&folder_uuid))
        .await?;
    let assets: Vec<_> = assets_response.get_all_assets().to_vec();

    if assets.is_empty() {
        error_utils::report_error_with_remediation(
            &format!("No assets found in folder with UUID: {}", folder_uuid),
            &[
                "Verify the folder UUID or path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the folder",
            ],
        );
        return Ok(());
    }

    // Check if progress should be displayed
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);

    // Create a temporary directory to store downloaded files
    let temp_dir = std::env::temp_dir().join(format!("pcli2_temp_{}", folder_uuid));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;

    // Create progress bar if requested
    let progress_bar = if show_progress {
        let pb = ProgressBar::new(assets.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Downloading assets")
            .unwrap()
            .progress_chars("#>-"));
        Some(pb)
    } else {
        None
    };

    // Download each asset to the temporary directory
    for asset in &assets {
        let file_content = api
            .download_asset(
                &tenant.uuid.to_string(),
                &asset.uuid().to_string(),
                Some(&asset.name()),
            )
            .await?;

        let asset_file_path = temp_dir.join(asset.name());
        let mut file = File::create(&asset_file_path).map_err(CliActionError::IoError)?;
        file.write_all(&file_content)
            .map_err(CliActionError::IoError)?;

        // Update progress bar if present
        if let Some(ref pb) = progress_bar {
            pb.inc(1);
        }
    }

    // Finish progress bar if present
    if let Some(pb) = progress_bar {
        pb.finish_with_message("Assets downloaded, creating ZIP...");
    }

    // Create ZIP file with all downloaded assets
    let zip_file = File::create(&output_file_path).map_err(CliActionError::IoError)?;
    let mut zip_writer = ZipWriter::new(zip_file);

    // Walk through the temp directory and add files to the ZIP
    for entry in std::fs::read_dir(&temp_dir)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?
    {
        let entry = entry.map_err(CliActionError::IoError)?;
        let path = entry.path();

        if path.is_file() {
            let file_name = path
                .file_name()
                .ok_or_else(|| {
                    CliError::ActionError(CliActionError::IoError(std::io::Error::other(
                        "Could not get file name",
                    )))
                })?
                .to_str()
                .ok_or_else(|| {
                    CliError::ActionError(CliActionError::IoError(std::io::Error::other(
                        "Invalid file name",
                    )))
                })?;

            let options: FileOptions<()> = FileOptions::default();
            zip_writer
                .start_file(file_name, options)
                .map_err(|e| CliError::ActionError(CliActionError::ZipError(e)))?;
            let file_content = std::fs::read(&path)
                .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
            zip_writer
                .write_all(&file_content)
                .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| CliError::ActionError(CliActionError::ZipError(e)))?;

    // Clean up temporary directory
    std::fs::remove_dir_all(&temp_dir)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;

    Ok(())
}

/// Delete specific metadata fields from an asset.
///
/// This function handles the "asset metadata delete" command, which removes
/// specified metadata fields from a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the metadata was deleted successfully
/// * `Err(CliError)` - If an error occurred during the deletion
pub async fn delete_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Execute \"asset metadata delete\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get metadata name parameter from command line
    let metadata_names = sub_matches
        .get_many::<String>("field_name")
        .ok_or(CliError::MissingRequiredArgument("field_name".to_string()))?
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

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

    // Delete the specified metadata fields using the dedicated API endpoint
    let metadata_keys: Vec<&str> = metadata_names.iter().map(|s| s.as_ref()).collect();
    api.delete_asset_metadata(
        &tenant.uuid.to_string(),
        &asset.uuid().to_string(),
        metadata_keys,
    )
    .await?;

    Ok(())
}

/// Apply metadata inference from a reference asset to geometrically similar assets
///
/// This function finds geometrically similar assets to a reference asset and copies specified metadata fields
/// from the reference to the similar assets.
///
/// # Arguments
/// * `sub_matches` - The command line arguments
///
/// # Returns
/// * `Ok(())` - If the operation completed successfully
/// * `Err(CliError)` - If an error occurred during the operation
pub async fn metadata_inference(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing metadata inference command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get the reference asset path
    let asset_path = sub_matches
        .get_one::<String>("path")
        .ok_or_else(|| CliError::MissingRequiredArgument("path".to_string()))?;

    // Get the metadata field names to copy
    let metadata_names: Vec<String> = sub_matches
        .get_many::<String>("inference_name")
        .ok_or_else(|| CliError::MissingRequiredArgument("name".to_string()))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get format parameters
    let format_str = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .map(|s| s.as_str())
        .unwrap_or("json");

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    };

    let format =
        crate::format::OutputFormat::from_string_with_options(format_str, format_options.clone())
            .map_err(|e| {
            CliActionError::FormattingError(crate::format::FormattingError::FormatFailure {
                cause: Box::new(e),
            })
        })?;

    // Get the reference asset
    let reference_asset = api.get_asset_by_path(&tenant.uuid, asset_path).await?;

    // Check if the reference asset has any of the requested metadata fields BEFORE performing expensive geometric search
    let reference_metadata = reference_asset.metadata();
    let mut available_fields = Vec::new();
    if let Some(asset_metadata) = reference_metadata {
        for field_name in &metadata_names {
            if asset_metadata.get(field_name).is_some() {
                available_fields.push(field_name.clone());
            }
        }
    }

    // Fail fast if no requested fields exist in the reference asset
    if available_fields.is_empty() {
        return Err(CliError::from(CliActionError::MissingRequiredArgument(
            format!(
                "Reference asset '{}' has no metadata fields matching: {:?}",
                asset_path, metadata_names
            ),
        )));
    }

    // Only perform expensive geometric search if we know we have fields to copy
    let search_results = api
        .geometric_search(&tenant.uuid, &reference_asset.uuid(), threshold)
        .await?;

    let mut assets_updated = Vec::new();

    // Extract the parent folder path from the reference asset
    let reference_parent_folder_path = {
        let path_str = reference_asset.path().to_string();
        let path_parts: Vec<&str> = path_str.split('/').collect();
        if path_parts.len() > 1 {
            // Join all parts except the last one (filename) to get the parent folder path
            path_parts[..path_parts.len() - 1].join("/")
        } else {
            // If there's only one part, the parent is root
            "".to_string()
        }
    };

    for match_result in search_results.matches {
        // If exclusive flag is set, only process assets in the same parent folder
        if exclusive {
            let candidate_parent_folder_path = {
                let path_str = match_result.asset.path.clone();
                let path_parts: Vec<&str> = path_str.split('/').collect();
                if path_parts.len() > 1 {
                    path_parts[..path_parts.len() - 1].join("/")
                } else {
                    "".to_string()
                }
            };

            // Skip if the candidate asset is not in the same parent folder as the reference
            if candidate_parent_folder_path != reference_parent_folder_path {
                continue;
            }
        }

        // Create a new metadata map with only the specified fields from the reference asset
        let mut new_metadata_map = std::collections::HashMap::new();

        if let Some(asset_metadata) = reference_metadata {
            for field_name in &available_fields {
                if let Some(value) = asset_metadata.get(field_name) {
                    new_metadata_map
                        .insert(field_name.clone(), serde_json::Value::String(value.clone()));
                }
            }
        }

        // Update the similar asset with the copied metadata with automatic registration of new keys
        if !new_metadata_map.is_empty() {
            api.update_asset_metadata_with_registration(
                &tenant.uuid,
                &match_result.asset.uuid,
                &new_metadata_map,
            )
            .await?;

            // Track which assets were updated
            assets_updated.push((match_result.asset.path.clone(), new_metadata_map.clone()));
        }
    }

    // Create a response structure to output the results
    let response = serde_json::json!({
        "reference_asset_path": asset_path,
        "reference_asset_uuid": reference_asset.uuid(),
        "threshold": threshold,
        "fields_copied": metadata_names,
        "assets_updated": assets_updated.len(),
        "updated_assets": assets_updated
    });

    match format {
        crate::format::OutputFormat::Json(options) => {
            if options.pretty {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("{}", serde_json::to_string(&response)?);
            }
        }
        crate::format::OutputFormat::Csv(options) => {
            // For CSV output, create a simple table
            let mut wtr = csv::Writer::from_writer(vec![]);

            if options.with_headers {
                if let Err(e) = wtr.serialize(&[
                    "REFERENCE_ASSET_PATH",
                    "CANDIDATE_ASSET_PATH",
                    "FIELD_NAME",
                    "FIELD_VALUE",
                    "THRESHOLD",
                ]) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for (candidate_asset_path, metadata_map) in &assets_updated {
                for (field_name, field_value) in metadata_map {
                    if let Err(e) = wtr.serialize([
                        asset_path,                         // REFERENCE_ASSET_PATH - the reference asset path
                        candidate_asset_path, // CANDIDATE_ASSET_PATH - the asset that received the metadata
                        field_name,           // FIELD_NAME
                        field_value.as_str().unwrap_or(""), // FIELD_VALUE
                        &threshold.to_string(), // THRESHOLD
                    ]) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            let csv_string = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };
            print!("{}", csv_string);
        }
        _ => {
            // Default to JSON
            if format_options.pretty {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("{}", serde_json::to_string(&response)?);
            }
        }
    }

    Ok(())
}

pub async fn text_match(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing text match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    // Get the text query parameter
    let text_query =
        sub_matches
            .get_one::<String>("text")
            .ok_or(CliError::MissingRequiredArgument(
                "text query is required".to_string(),
            ))?;

    // Get the fuzzy flag - if not specified, default to false (meaning exact search with quoted text)
    let fuzzy = sub_matches.get_flag(PARAMETER_FUZZY);

    // If fuzzy is false (default), wrap the text query in quotes for exact search
    let search_query = if fuzzy {
        text_query.clone()
    } else {
        format!("\"{}\"", text_query)
    };

    // Get format parameters directly from sub_matches since text match commands have format flags
    let format_str = if let Some(format_val) =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
    {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string()
        }
    };

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Extract tenant info before calling text search
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Perform text search
    let mut search_results = ctx.api().text_search(&tenant_uuid, &search_query).await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate asset URLs for each match (not comparison URLs since text search doesn't compare two assets)
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let asset_url = format!(
            "{}/tenants/{}/asset/{}",
            base_url,    // Use configurable UI base URL without trailing slash
            tenant_name, // Use tenant short name in path
            match_result.asset.uuid
        );
        match_result.comparison_url = Some(asset_url); // Store asset URL in comparison_url field for text search
    }

    // Create enhanced response that includes the search query information
    let enhanced_response = crate::model::EnhancedTextSearchResponse {
        search_query: text_query.clone(), // Use the original user input for display
        matches: search_results.matches,
    };

    // Format the response based on the output format
    match format {
        crate::format::OutputFormat::Json(options) => {
            if options.pretty {
                println!("{}", serde_json::to_string_pretty(&enhanced_response)?);
            } else {
                println!("{}", serde_json::to_string(&enhanced_response)?);
            }
        }
        crate::format::OutputFormat::Csv(options) => {
            let mut wtr = csv::Writer::from_writer(vec![]);

            if options.with_headers {
                if options.with_metadata {
                    // Include metadata columns in the header
                    let mut base_headers = crate::model::EnhancedTextSearchResponse::csv_header();

                    // Get unique metadata keys from all assets in the response
                    let mut all_metadata_keys = std::collections::HashSet::new();
                    for match_result in &enhanced_response.matches {
                        for key in match_result.asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Extend headers with metadata columns
                    for key in &sorted_keys {
                        base_headers.push(format!("ASSET_{}", key.to_uppercase()));
                    }

                    if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                } else {
                    let headers = crate::model::EnhancedTextSearchResponse::csv_header();
                    if let Err(e) = wtr.serialize(headers.as_slice()) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                }
            }

            for match_result in &enhanced_response.matches {
                if options.with_metadata {
                    // Include metadata values in the output
                    let base_values = vec![
                        match_result
                            .asset
                            .path
                            .split('/')
                            .last()
                            .unwrap_or(&match_result.asset.path)
                            .to_string(), // ASSET_NAME
                        match_result.asset.path.clone(), // ASSET_PATH
                        match_result.asset.asset_type.clone(), // TYPE
                        match_result.asset.state.clone(), // STATE
                        match_result.asset.is_assembly.to_string(), // IS_ASSEMBLY
                        format!("{}", match_result.relevance_score.unwrap_or(0.0)), // RELEVANCE_SCORE
                        match_result.asset.uuid.to_string(),                        // ASSET_UUID
                        match_result.comparison_url.clone().unwrap_or_default(),    // ASSET_URL
                    ];

                    // Get unique metadata keys from all assets in the response
                    let mut all_metadata_keys = std::collections::HashSet::new();
                    for mr in &enhanced_response.matches {
                        for key in mr.asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Add metadata values for each key
                    let mut extended_values = base_values.clone();
                    for key in &sorted_keys {
                        let value = match_result
                            .asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        extended_values.push(value);
                    }

                    if let Err(e) = wtr.serialize(extended_values.as_slice()) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                } else {
                    let records = match_result.as_csv_records();
                    for record in records {
                        if let Err(e) = wtr.serialize(record.as_slice()) {
                            return Err(CliError::from(CliActionError::FormattingError(
                                crate::format::FormattingError::CsvError(e),
                            )));
                        }
                    }
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            let output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };
            print!("{}", output);
        }
        _ => {
            // Default to JSON
            println!("{}", serde_json::to_string_pretty(&enhanced_response)?);
        }
    }

    Ok(())
}

/// Execute the folder dependencies command to get dependencies for all assembly assets in one or more folders
pub async fn print_folder_dependencies(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"folder dependencies\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    // Get folder paths from the command line arguments
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();

    if folder_paths.is_empty() {
        return Err(CliError::MissingRequiredArgument(
            "At least one folder path must be provided".to_string(),
        ));
    }

    let tenant_uuid = *ctx.tenant_uuid();

    // Check if progress should be displayed
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);

    // Create progress bars if requested
    let multi_progress = if show_progress {
        let mp = indicatif::MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(indicatif::ProgressBar::new(folder_paths.len() as u64));
        overall_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Processing folders")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Collect all dependencies from all specified folders
    let mut all_dependencies = Vec::new();
    let mut all_assembly_trees = Vec::new();

    for folder_path in folder_paths.iter() {
        // Update overall progress if enabled
        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.set_message(format!("Processing folder: {}", folder_path));
        }

        // List all assets in the folder
        let assets_response = ctx
            .api()
            .list_assets_by_parent_folder_path(&tenant_uuid, folder_path)
            .await?;

        // Count total assemblies in this folder for progress tracking
        let assemblies: Vec<_> = assets_response
            .get_all_assets()
            .into_iter()
            .filter(|asset| asset.is_assembly())
            .collect();

        // Create individual progress bar for this folder if progress is enabled
        let folder_progress = if let Some((ref mp, _)) = multi_progress {
            let pb = mp.add(indicatif::ProgressBar::new(assemblies.len() as u64));
            pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template(&format!(
                        "{{spinner:.yellow}} Processing assets in {}: {{pos}}/{{len}} {{msg}}",
                        folder_path
                    ))
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };

        // Process each asset in the folder that is an assembly (has dependencies)
        for asset in assemblies {
            if let Some(ref pb) = folder_progress {
                pb.set_message(format!("Getting dependencies for: {}", asset.name()));
            }

            trace!(
                "Processing assembly: {} (path: {})",
                asset.name(),
                asset.path()
            );

            // Get the full assembly tree with all recursive dependencies for this asset
            let assembly_tree = ctx
                .api()
                .get_asset_dependencies_by_path(&tenant_uuid, asset.path().as_str())
                .await?;

            // For tree and JSON formats, we'll collect the assembly trees to preserve hierarchy
            let format_is_tree = matches!(format, crate::format::OutputFormat::Tree(_));
            let format_is_json = matches!(format, crate::format::OutputFormat::Json(_));
            if format_is_tree || format_is_json {
                all_assembly_trees.push(assembly_tree);
            } else {
                // For other formats (CSV), extract all dependencies from the tree structure
                let mut asset_dependencies = extract_all_dependencies_from_tree(&assembly_tree);

                // Update each dependency to include the original asset path information (for ASSET_PATH column)
                // The assembly_path should remain as the relative path within the assembly hierarchy
                for dep in &mut asset_dependencies {
                    // The assembly_path should already contain the relative path within the assembly hierarchy
                    // from the extract_all_dependencies_from_tree function, so we don't modify it here
                    // It represents the path from the root of this assembly to the dependency

                    // Set the original asset path for proper CSV output
                    dep.original_asset_path = Some(asset.path().to_string());
                }

                // Add all dependencies from this asset's tree to the combined list
                all_dependencies.extend(asset_dependencies);
            }

            // Update folder progress if enabled
            if let Some(ref pb) = folder_progress {
                pb.inc(1);
            }
        }

        // Finish folder progress bar if enabled
        if let Some(pb) = folder_progress {
            pb.finish_and_clear();
        }

        // Update overall progress if enabled
        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    // Finish overall progress bar if enabled
    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!("Processed {} folders", folder_paths.len()));
    }

    // Output the results based on the requested format
    let format_is_tree = matches!(format, crate::format::OutputFormat::Tree(_));
    let format_is_json = matches!(format, crate::format::OutputFormat::Json(_));
    if format_is_tree || format_is_json {
        // For tree and JSON formats, if we have multiple assembly trees, we need to handle them appropriately
        if all_assembly_trees.len() == 1 {
            // If there's only one tree, just output it directly
            println!("{}", all_assembly_trees[0].format(format)?);
        } else if all_assembly_trees.is_empty() {
            // If no assembly trees were found, output an empty result
            if matches!(format, crate::format::OutputFormat::Json(_)) {
                println!("[]"); // Output empty array for JSON format
            } else {
                println!("No assembly assets found in the specified folders.");
            }
        } else {
            // If there are multiple trees, output them separately with separators
            for (i, tree) in all_assembly_trees.iter().enumerate() {
                if i > 0 {
                    println!("---"); // Separator between different folder results
                }
                println!("{}", tree.format(format.clone())?);
            }
        }
    } else {
        // For CSV format, create an AssetDependencyList with all collected dependencies
        // Use a more appropriate path that indicates this is from multiple assets in the specified folders
        let dependency_list = crate::model::AssetDependencyList {
            path: "MULTIPLE_ASSETS".to_string(), // Indicate this represents multiple assets
            dependencies: all_dependencies,
        };

        println!("{}", dependency_list.format(format)?);
    }

    Ok(())
}

async fn download_asset_with_retry(
    api: &mut crate::physna_v3::PhysnaApiClient,
    tenant_id: &str,
    asset_id: &str,
) -> Result<Vec<u8>, CliError> {
    use rand::Rng;

    // First attempt
    match api.download_asset(tenant_id, asset_id, None).await {
        Ok(content) => Ok(content),
        Err(e) => {
            // If the first attempt fails, wait for a random delay between 3-5 seconds and retry once
            tracing::warn!(
                "Asset download failed (attempt 1), retrying after delay: {}",
                e
            );

            // Generate random delay between 3 and 5 seconds
            let mut rng = rand::thread_rng();
            let delay_seconds = rng.gen_range(3..=5);

            tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

            // Second and final attempt
            match api.download_asset(tenant_id, asset_id, None).await {
                Ok(content) => Ok(content),
                Err(final_e) => {
                    tracing::error!("Asset download failed after retry: {}", final_e);
                    Err(CliError::PhysnaExtendedApiError(final_e))
                }
            }
        }
    }
}

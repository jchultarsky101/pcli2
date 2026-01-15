use std::{path::PathBuf, str::FromStr};
use clap::ArgMatches;
use uuid::Uuid;
use crate::actions::CliActionError;
use crate::{actions::folders::resolve_folder_uuid_by_path, commands::params::{PARAMETER_FILE, PARAMETER_FILES, PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_PATH, PARAMETER_UUID}, configuration::Configuration, error::CliError, error_utils, format::{OutputFormatter, CsvRecordProducer}, metadata::convert_single_metadata_to_json_value, model::{AssetList, Folder, normalize_path}, param_utils::{get_format_parameter_value, get_tenant}, physna_v3::{PhysnaApiClient, TryDefault}};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tracing::{debug, trace};

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
        
        let assets = api.list_assets_by_parent_folder_path(&tenant.uuid, path.as_str()).await?;

        println!("{}", assets.format(format)?);
    };
    

	Ok(())
}

pub async fn print_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset get\" command...");
    
    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let format = get_format_parameter_value(sub_matches).await;
    
    let asset_uuid_param = sub_matches.get_one::<String>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
    
    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        let uuid = Uuid::from_str(uuid).unwrap(); 
        api.get_asset_by_uuid(&tenant.uuid, &uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset cache or fetch assets from API
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    println!("{}", asset.format(format)?);        

	Ok(())
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
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
    };
            
    // Check if the folder exists
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder: Folder = folder.into();
    match folder_path_param {
        Some(path) => folder.set_path(path.to_owned()),
        None => (),
    }

    let file_path = sub_matches.get_one::<PathBuf>(PARAMETER_FILE).ok_or(CliError::MissingRequiredArgument("file".to_string()))?;
                
    // Extract filename from path for use in asset path construction
    let file_name = file_path
        .file_name()
        .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file path".to_string()))?
        .to_str()
        .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file name".to_string()))?
        .to_string();

    
    // Construct the full asset path by combining folder path with filename
    let asset_path = match folder_path_param {
        Some(folder_path) => {
            if folder_path.is_empty() {
                file_name.clone()
            } else {
                format!("{}/{}", folder_path, file_name)
            }
        },
        None => file_name.clone(),
    };
    
    debug!("Creating asset with path: {}", asset_path);

    let asset = api.create_asset(&tenant.uuid, &file_path, &asset_path, &folder_uuid).await?;
    println!("{}", asset.format(format)?);

    Ok(())
}

pub async fn create_asset_batch(sub_matches: &ArgMatches) -> Result<(), CliError> {

     trace!("Executing \"create asset batch\" command...");
    
    let glob_pattern = sub_matches.get_one::<String>(PARAMETER_FILES)
        .ok_or(CliError::MissingRequiredArgument("files".to_string()))?
        .clone();
    let concurrent_param = sub_matches.get_one::<usize>("concurrent")
        .unwrap_or(&5);
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
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
    };
            
    // Check if the folder exists
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder: Folder = folder.into();
    match folder_path_param {
        Some(path) => folder.set_path(path.to_owned()),
        None => (),
    }

    let assets = api.create_assets_batch(&tenant.uuid, &glob_pattern, Some(folder.path().as_str()), Some(&folder_uuid), concurrent, show_progress).await?;
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

    let csv_file_path = sub_matches.get_one::<std::path::PathBuf>("csv-file")
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
    let mut asset_metadata_map: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();

    for result in reader.records() {
        let record: csv::StringRecord = result.map_err(|e| CliError::FormattingError(crate::format::FormattingError::CsvError(e)))?;

        if record.len() >= 3 {
            let asset_path: &str = record[0].trim();
            let metadata_name: &str = record[1].trim();
            let metadata_value: &str = record[2].trim();

            // Use the same conversion logic as individual metadata command (default to text type)
            let json_value = crate::metadata::convert_single_metadata_to_json_value(
                metadata_name,  // name parameter (not used in function)
                metadata_value,
                "text"  // default to text type since CSV doesn't specify type
            );

            // Group metadata by asset path (strip leading slash if present for consistency with asset paths in system)
            let clean_asset_path = asset_path.strip_prefix('/').unwrap_or(asset_path);
            asset_metadata_map.entry(clean_asset_path.to_string())
                .or_insert_with(HashMap::new)
                .insert(metadata_name.to_string(), json_value);
        }
    }

    // Process each asset with its metadata
    let total_assets = asset_metadata_map.len();
    let mut current_asset = 0;

    for (asset_path, metadata) in &asset_metadata_map {
        if show_progress {
            current_asset += 1;
            eprint!("\rProcessing asset {}/{}: {}", current_asset, total_assets, asset_path);
        }

        // Get the asset by the normalized path
        match api.get_asset_by_path(&tenant.uuid, asset_path).await {
            Ok(asset) => {
                // Update the asset's metadata with automatic registration of new keys
                if let Err(e) = api.update_asset_metadata_with_registration(&tenant.uuid, &asset.uuid(), metadata).await {
                    error_utils::report_error_with_remediation(
                        &format!("Failed to update metadata for asset '{}': {}", asset_path, e),
                        &[
                            "Verify metadata field names and values are valid",
                            "Check that you have sufficient permissions to modify this asset",
                            "Verify your network connectivity",
                            "Confirm the asset hasn't been deleted or modified recently"
                        ]
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
    let metadata_name = sub_matches.get_one::<String>("name")
        .ok_or(CliError::MissingRequiredArgument("name".to_string()))?;
    let metadata_value = sub_matches.get_one::<String>("value")
        .ok_or(CliError::MissingRequiredArgument("value".to_string()))?;
    let metadata_type = sub_matches.get_one::<String>("type")
        .map(|s| s.as_str())
        .unwrap_or("text");

    // Convert the single metadata entry to JSON value using shared function
    let json_value = convert_single_metadata_to_json_value(
        metadata_name,
        metadata_value,
        metadata_type
    );

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
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Update the asset's metadata with automatic registration of new keys
    api.update_asset_metadata_with_registration(&tenant.uuid, &asset.uuid(), &metadata).await?;

    // No output on success (per requirements)

    Ok(())
}

pub async fn print_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    
    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let format = get_format_parameter_value(sub_matches).await;
    
    let asset_uuid_param = sub_matches.get_one::<String>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);
    
    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        let uuid = Uuid::from_str(uuid).unwrap(); 
        api.get_asset_by_uuid(&tenant.uuid, &uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset cache or fetch assets from API
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };
    
    match asset.metadata() {
        Some(metadata) => println!("{}", metadata.format(format)?),
        None => ()
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

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Delete the asset
    api.delete_asset(&tenant.uuid.to_string(), &asset.uuid().to_string()).await?;

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

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Get the output file path
    let output_file_path = if let Some(output_path) = sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_FILE) {
        output_path.clone()
    } else {
        // Use the asset name as the default output file name
        let asset_name = asset.name();
        let mut path = std::path::PathBuf::new();
        path.push(asset_name);
        path
    };

    // Download the asset file
    let file_content = api.download_asset(
        &tenant.uuid.to_string(),
        &asset.uuid().to_string()
    ).await?;

    // Write the file content to the output file
    std::fs::write(&output_file_path, file_content).map_err(|e| CliActionError::IoError(e))?;

    Ok(())
}

pub async fn geometric_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing geometric match command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get threshold parameter
    let threshold = sub_matches.get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters directly from sub_matches since geometric match commands have all format flags
    let format_str = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT).unwrap();

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(|e| CliActionError::FormattingError(e))?;

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Perform geometric search
    let mut search_results = api.geometric_search(&tenant.uuid, &asset.uuid(), threshold).await?;

    // Load configuration to get the UI base URL
    let configuration = crate::configuration::Configuration::load_or_create_default()
        .map_err(|e| CliError::ConfigurationError(
            crate::configuration::ConfigurationError::FailedToLoadData {
                cause: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
            }
        ))?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let comparison_url = format!(
            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
            ui_base_url, // Use configurable UI base URL
            tenant.name, // Use tenant short name in path
            asset.uuid(),
            match_result.asset.uuid,
            tenant.uuid, // Use tenant UUID in query params
            tenant.uuid, // Use tenant UUID in query params
            match_result.match_percentage
        );
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
        tenant_id: tenant.uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None, // No owner ID
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

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get threshold parameter
    let threshold = sub_matches.get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters directly from sub_matches since part match commands have all format flags
    let format_str = if let Some(format_val) = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT) {
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
        .map_err(|e| CliActionError::FormattingError(e))?;

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Perform part search
    let mut search_results = api.part_search(&tenant.uuid, &asset.uuid(), threshold).await?;

    // Load configuration to get the UI base URL
    let configuration = crate::configuration::Configuration::load_or_create_default()
        .map_err(|e| CliError::ConfigurationError(
            crate::configuration::ConfigurationError::FailedToLoadData {
                cause: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
            }
        ))?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let comparison_url = format!(
            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
            ui_base_url, // Use configurable UI base URL
            tenant.name, // Use tenant short name in path
            asset.uuid(),
            match_result.asset.uuid,
            tenant.uuid, // Use tenant UUID in query params
            tenant.uuid, // Use tenant UUID in query params
            match_result.forward_match_percentage.unwrap_or(0.0),
            match_result.reverse_match_percentage.unwrap_or(0.0)
        );
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
        tenant_id: tenant.uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None, // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedPartSearchResponse {
        reference_asset: reference_asset_response,
        matches: search_results.matches,
    };

    // Format the response considering the metadata flag
    println!("{}", enhanced_response.format_with_metadata_flag(format, with_metadata)?);

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
        .ok_or(CliError::MissingRequiredArgument("folder-path".to_string()))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let threshold = sub_matches.get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters
    let format_str = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT).unwrap_or(&"json".to_string()).clone();

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(crate::commands::params::PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(|e| CliActionError::FormattingError(e))?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if val < 1 || val > 10 {
                return Err(CliError::MissingRequiredArgument(format!("Invalid value for '--concurrent': must be between 1 and 10, got {}", val)));
            }
            val
        },
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api.list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str()).await?;

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
                "Ensure you have permissions to access the specified folder(s)"
            ]
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
    type TaskResult = Result<Vec<crate::model::EnhancedGeometricSearchResponse>, Box<dyn std::error::Error + Send + Sync>>;
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
                        .template(&format!("{{spinner:.green}} Processing: {} {{msg}}", asset_clone.name()))
                        .unwrap()
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting geometric search...");
            }

            let result = match api_clone.geometric_search(&tenant_uuid, &asset_uuid, threshold).await {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Processing {} matches...", search_results.matches.len()));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration = crate::configuration::Configuration::load_or_create_default()
                            .map_err(|e| CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
                                }
                            ))?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let comparison_url = format!(
                            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                            ui_base_url, // Use configurable UI base URL
                            tenant_clone.name, // Use tenant short name in path
                            asset_uuid,
                            match_result.asset.uuid,
                            tenant_uuid, // Use tenant UUID in query params
                            tenant_uuid, // Use tenant UUID in query params
                            match_result.match_percentage
                        );
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        let candidate_in_specified_folders = folder_paths_clone.iter().any(|folder_path| {
                            match_result.asset.path.starts_with(folder_path)
                        });

                        if exclusive && !candidate_in_specified_folders {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
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
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(), // Placeholder for creation time
                            updated_at: "".to_string(), // Placeholder for update time
                            state: "active".to_string(), // Default state
                            is_assembly: false, // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None, // No owner ID
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
                    error_utils::report_warning(&format!("🔍 Failed to perform geometric search for asset {}: {}", asset_clone.name(), e));
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
                        let (ref_uuid, cand_uuid) = if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
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
                        "Retry the operation"
                    ]
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation"
                    ]
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!("Processed {} assets. Found {} unique matches.", all_assets.len(), all_matches.len()));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::GeometricMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result
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
                    flattened_matches.push(crate::model::GeometricMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result
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
                let mut base_headers = crate::model::GeometricMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
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
                        let ref_value = match_pair.reference_asset.metadata.get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair.candidate_asset.metadata.get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvIntoInnerError(e))));
                }
            };
            output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::Utf8Error(e))));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::GeometricMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result
                    ));
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
        .ok_or(CliError::MissingRequiredArgument("folder-path".to_string()))?
        .cloned()
        .collect();

    // Get threshold parameter
    let threshold = sub_matches.get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters
    let format_str = if let Some(format_val) = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT) {
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
        .map_err(|e| CliActionError::FormattingError(e))?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if val < 1 || val > 10 {
                return Err(CliError::MissingRequiredArgument(format!("Invalid value for '--concurrent': must be between 1 and 10, got {}", val)));
            }
            val
        },
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api.list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str()).await?;

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
                "Ensure you have permissions to access the specified folder(s)"
            ]
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
    type TaskResult = Result<Vec<crate::model::EnhancedPartSearchResponse>, Box<dyn std::error::Error + Send + Sync>>;
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
                        .template(&format!("{{spinner:.green}} Processing: {} {{msg}}", asset_clone.name()))
                        .unwrap()
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting part search...");
            }

            let result = match api_clone.part_search(&tenant_uuid, &asset_uuid, threshold).await {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Processing {} matches...", search_results.matches.len()));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration = crate::configuration::Configuration::load_or_create_default()
                            .map_err(|e| CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
                                }
                            ))?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let comparison_url = format!(
                            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                            ui_base_url, // Use configurable UI base URL
                            tenant_clone.name, // Use tenant short name in path
                            asset_uuid,
                            match_result.asset.uuid,
                            tenant_uuid, // Use tenant UUID in query params
                            tenant_uuid, // Use tenant UUID in query params
                            match_result.forward_match_percentage.unwrap_or(0.0),
                            match_result.reverse_match_percentage.unwrap_or(0.0)
                        );
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        let candidate_in_specified_folders = folder_paths_clone.iter().any(|folder_path| {
                            match_result.asset.path.starts_with(folder_path)
                        });

                        if exclusive && !candidate_in_specified_folders {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
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
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(), // Placeholder for creation time
                            updated_at: "".to_string(), // Placeholder for update time
                            state: "active".to_string(), // Default state
                            is_assembly: false, // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None, // No owner ID
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
                    error_utils::report_warning(&format!("🔍 Failed to perform part search for asset {}: {}", asset_clone.name(), e));
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
                        let (ref_uuid, cand_uuid) = if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
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
                        "Retry the operation"
                    ]
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation"
                    ]
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!("Processed {} assets. Found {} unique matches.", all_assets.len(), all_matches.len()));
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
                        match_result
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
                        match_result
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
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair.forward_match_percentage.map_or_else(|| "0.0".to_string(), |val| format!("{}", val)),
                    match_pair.reverse_match_percentage.map_or_else(|| "0.0".to_string(), |val| format!("{}", val)),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair.reference_asset.metadata.get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair.candidate_asset.metadata.get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvIntoInnerError(e))));
                }
            };
            output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::Utf8Error(e))));
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
                        match_result
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

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_PATH);

    // Get format parameters directly from sub_matches since visual match commands have all format flags
    let format_str = if let Some(format_val) = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT) {
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
        .map_err(|e| CliActionError::FormattingError(e))?;

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Perform visual search
    let mut search_results = api.visual_search(&tenant.uuid, &asset.uuid()).await?;

    // Load configuration to get the UI base URL
    let configuration = crate::configuration::Configuration::load_or_create_default()
        .map_err(|e| CliError::ConfigurationError(
            crate::configuration::ConfigurationError::FailedToLoadData {
                cause: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
            }
        ))?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let comparison_url = format!(
            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
            ui_base_url, // Use configurable UI base URL
            tenant.name, // Use tenant short name in path
            asset.uuid(),
            match_result.asset.uuid,
            tenant.uuid, // Use tenant UUID in query params
            tenant.uuid, // Use tenant UUID in query params
        );
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
        tenant_id: tenant.uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None, // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    // For visual search, we need to ensure match percentages are handled properly
    // since visual search doesn't have similarity scores
    let adjusted_matches: Vec<crate::model::PartMatch> = search_results.matches
        .into_iter()
        .map(|mut match_item| {
            // Set match percentages to None for visual search since they don't apply
            match_item.forward_match_percentage = None;
            match_item.reverse_match_percentage = None;
            match_item
        })
        .collect();

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedPartSearchResponse {
        reference_asset: reference_asset_response,
        matches: adjusted_matches,
    };

    // Format the response considering the metadata flag
    println!("{}", enhanced_response.format_with_metadata_flag(format, with_metadata)?);

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
        .ok_or(CliError::MissingRequiredArgument("folder-path".to_string()))?
        .cloned()
        .collect();

    // Get format parameters
    let format_str = if let Some(format_val) = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT) {
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
        .map_err(|e| CliActionError::FormattingError(e))?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if val < 1 || val > 10 {
                return Err(CliError::MissingRequiredArgument(format!("Invalid value for '--concurrent': must be between 1 and 10, got {}", val)));
            }
            val
        },
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api.list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str()).await?;

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
                "Ensure you have permissions to access the specified folder(s)"
            ]
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
    type TaskResult = Result<Vec<crate::model::EnhancedPartSearchResponse>, Box<dyn std::error::Error + Send + Sync>>;
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
                        .template(&format!("{{spinner:.green}} Processing: {} {{msg}}", asset_clone.name()))
                        .unwrap()
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
                        pb.set_message(format!("Processing {} matches...", search_results.matches.len()));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration = crate::configuration::Configuration::load_or_create_default()
                            .map_err(|e| CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e) as Box<dyn std::error::Error + Send + Sync>,
                                }
                            ))?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let comparison_url = format!(
                            "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                            ui_base_url, // Use configurable UI base URL
                            tenant_clone.name, // Use tenant short name in path
                            asset_uuid,
                            match_result.asset.uuid,
                            tenant_uuid, // Use tenant UUID in query params
                            tenant_uuid, // Use tenant UUID in query params
                        );
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        let candidate_in_specified_folders = folder_paths_clone.iter().any(|folder_path| {
                            match_result.asset.path.starts_with(folder_path)
                        });

                        if exclusive && !candidate_in_specified_folders {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
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
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(), // Placeholder for creation time
                            updated_at: "".to_string(), // Placeholder for update time
                            state: "active".to_string(), // Default state
                            is_assembly: false, // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None, // No owner ID
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
                    error_utils::report_warning(&format!("🔍 Failed to perform visual search for asset {}: {}", asset_clone.name(), e));
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
                        let (ref_uuid, cand_uuid) = if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
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
                        "Retry the operation"
                    ]
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation"
                    ]
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!("Processed {} assets. Found {} unique matches.", all_assets.len(), all_matches.len()));
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
                        match_result
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
                        match_result
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
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    // For visual match, we don't have match percentages, so we'll use empty strings
                    "".to_string(), // Forward match percentage placeholder
                    "".to_string(), // Reverse match percentage placeholder
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair.reference_asset.metadata.get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair.candidate_asset.metadata.get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvIntoInnerError(e))));
                }
            };
            output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::Utf8Error(e))));
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
                        match_result
                    ));
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

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
    let metadata_names = sub_matches.get_many::<String>("field_name")
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
        return Err(CliError::MissingRequiredArgument("Either asset UUID or path must be provided".to_string()));
    };

    // Delete the specified metadata fields using the dedicated API endpoint
    let metadata_keys: Vec<&str> = metadata_names.iter().map(|s| s.as_ref()).collect();
    api.delete_asset_metadata(&tenant.uuid.to_string(), &asset.uuid().to_string(), metadata_keys).await?;

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
    let asset_path = sub_matches.get_one::<String>("path")
        .ok_or_else(|| CliError::MissingRequiredArgument("path".to_string()))?;

    // Get the metadata field names to copy
    let metadata_names: Vec<String> = sub_matches
        .get_many::<String>("inference_name")
        .ok_or_else(|| CliError::MissingRequiredArgument("name".to_string()))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let threshold = sub_matches.get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get format parameters
    let format_str = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .map(|s| s.as_str())
        .unwrap_or("json");

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(format_str, format_options.clone())
        .map_err(|e| CliActionError::FormattingError(crate::format::FormattingError::FormatFailure { cause: Box::new(e) }))?;

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
        return Err(CliError::from(CliActionError::MissingRequiredArgument(format!(
            "Reference asset '{}' has no metadata fields matching: {:?}", asset_path, metadata_names
        ))));
    }

    // Only perform expensive geometric search if we know we have fields to copy
    let search_results = api.geometric_search(&tenant.uuid, &reference_asset.uuid(), threshold).await?;

    let mut assets_updated = Vec::new();

    // Extract the parent folder path from the reference asset
    let reference_parent_folder_path = {
        let path_str = reference_asset.path().to_string();
        let path_parts: Vec<&str> = path_str.split('/').collect();
        if path_parts.len() > 1 {
            // Join all parts except the last one (filename) to get the parent folder path
            path_parts[..path_parts.len()-1].join("/")
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
                    path_parts[..path_parts.len()-1].join("/")
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
                    new_metadata_map.insert(field_name.clone(), serde_json::Value::String(value.clone()));
                }
            }
        }

        // Update the similar asset with the copied metadata with automatic registration of new keys
        if !new_metadata_map.is_empty() {
            api.update_asset_metadata_with_registration(&tenant.uuid, &match_result.asset.uuid, &new_metadata_map).await?;

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
                if let Err(e) = wtr.serialize(&["REFERENCE_ASSET_PATH", "CANDIDATE_ASSET_PATH", "FIELD_NAME", "FIELD_VALUE", "THRESHOLD"]) {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                }
            }

            for (candidate_asset_path, metadata_map) in &assets_updated {
                for (field_name, field_value) in metadata_map {
                    if let Err(e) = wtr.serialize(&[
                        asset_path,  // REFERENCE_ASSET_PATH - the reference asset path
                        candidate_asset_path,  // CANDIDATE_ASSET_PATH - the asset that received the metadata
                        field_name,  // FIELD_NAME
                        field_value.as_str().unwrap_or(""), // FIELD_VALUE
                        &threshold.to_string() // THRESHOLD
                    ]) {
                        return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvError(e))));
                    }
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::CsvIntoInnerError(e))));
                }
            };
            let csv_string = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(crate::format::FormattingError::Utf8Error(e))));
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
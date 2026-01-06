use std::{path::PathBuf, str::FromStr};
use clap::ArgMatches;
use uuid::Uuid;
use crate::actions::CliActionError;
use crate::{actions::folders::resolve_folder_uuid_by_path, commands::params::{PARAMETER_FILE, PARAMETER_FILES, PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_PATH, PARAMETER_UUID}, configuration::Configuration, error::CliError, format::OutputFormatter, metadata::convert_single_metadata_to_json_value, model::{AssetList, Folder, normalize_path}, param_utils::{get_format_parameter_value, get_tenant}, physna_v3::{PhysnaApiClient, TryDefault}};
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

    // Update the asset's metadata
    api.update_asset_metadata(&tenant.uuid, &asset.uuid(), &metadata).await?;

    // Get and display the updated asset details
    let updated_asset = api.get_asset_by_uuid(&tenant.uuid, &asset.uuid()).await?;
    let format = get_format_parameter_value(sub_matches).await;
    println!("{}", updated_asset.format(format)?);

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

    let format = crate::format::OutputFormat::from_string_with_options(format_str, format_options)
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
    let search_results = api.geometric_search(&tenant.uuid, &asset.uuid(), threshold).await?;

    // Create a basic AssetResponse from the asset for the reference
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
        metadata: std::collections::HashMap::new(), // Empty metadata
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

pub async fn geometric_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing geometric match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let _tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let _format = get_format_parameter_value(sub_matches).await;

    // Get folder paths
    let _folder_paths: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_PATH)
        .ok_or(CliError::MissingRequiredArgument("path".to_string()))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let _threshold = sub_matches.get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get exclusive flag
    let _exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let _concurrent = sub_matches.get_one::<usize>("concurrent").copied().unwrap_or(5);
    let _show_progress = sub_matches.get_flag("progress");

    // For now, this is a placeholder implementation since the API doesn't have a direct method for folder-based geometric search
    // In a real implementation, this would iterate through all assets in the specified folders and perform geometric searches
    eprintln!("Geometric match folder functionality not yet fully implemented");
    Err(CliError::MissingRequiredArgument("Geometric match folder functionality not yet implemented".to_string()))
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
    let metadata_names = sub_matches.get_many::<String>("name")
        .ok_or(CliError::MissingRequiredArgument("name".to_string()))?
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

    // Delete the specified metadata fields
    // Note: The API doesn't have a direct method to delete specific metadata, so we'll need to
    // update the asset with metadata excluding the specified keys
    let existing_asset = api.get_asset_by_uuid(&tenant.uuid, &asset.uuid()).await?;
    let existing_metadata = existing_asset.metadata();

    // Create a new metadata map without the specified keys
    let mut new_metadata_map = std::collections::HashMap::new();
    if let Some(asset_metadata) = existing_metadata {
        for key in asset_metadata.keys() {
            if !metadata_names.contains(&key.as_str()) {
                if let Some(value) = asset_metadata.get(key) {
                    // Convert AssetMetadata's String values back to serde_json::Value
                    // We need to recreate the metadata map with JSON values for the API call
                    new_metadata_map.insert(key.clone(), serde_json::Value::String(value.clone()));
                }
            }
        }
    }

    // Update the asset with the modified metadata
    api.update_asset_metadata(&tenant.uuid, &asset.uuid(), &new_metadata_map).await?;

    Ok(())
}
use std::{path::PathBuf, str::FromStr};
use clap::ArgMatches;
use uuid::Uuid;
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


pub async fn print_asset_batch(sub_matches: &ArgMatches) -> Result<(), CliError> {

    trace!("Execute \"asset batch\" command...");
    
    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let format = get_format_parameter_value(sub_matches).await;
    
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
    let json_value = convert_single_metadata_to_json_value(
        metadata_name, 
        metadata_value, 
        metadata_type
    );
    
    // Create a HashMap with the single metadata entry
    // This hashmap represents the desired metadata fields
    let mut metadata: std::collections::HashMap<String, serde_json::Value> = 
        std::collections::HashMap::new();
    metadata.insert(metadata_name.clone(), json_value);
    
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

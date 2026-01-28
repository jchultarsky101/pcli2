/// Upload all assets from a local directory to a Physna folder.
///
/// This function handles the "folder upload" command, uploading all asset files
/// from a specified local directory to a Physna folder.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the folder was uploaded successfully
/// * `Err(CliError)` - If an error occurred during upload
pub async fn upload_folder(sub_matches: &clap::ArgMatches) -> Result<(), crate::error::CliError> {
    use std::path::Path;
    use uuid::Uuid;
    use crate::{
        commands::params::{PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID},
        configuration::Configuration,
        error::CliError,
        param_utils::{get_tenant},
        physna_v3::{PhysnaApiClient, TryDefault},
        model::normalize_path
    };

    trace!("Executing \"folder upload\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get the local directory path from command line
    let local_dir_path = sub_matches.get_one::<std::path::PathBuf>("local-path")
        .ok_or_else(|| CliError::MissingRequiredArgument("Local directory path is required".to_string()))?;

    // Check if the local path exists and is a directory
    if !local_dir_path.exists() {
        return Err(CliError::ActionError(crate::actions::CliActionError::IoError(
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("Local path does not exist: {:?}", local_dir_path))
        )));
    }

    if !local_dir_path.is_dir() {
        return Err(CliError::ActionError(crate::actions::CliActionError::IoError(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Local path is not a directory: {:?}", local_dir_path))
        )));
    }

    // Get folder UUID or path from command line
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Resolve the folder UUID - first try to get existing folder, then create if needed
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        // Try to resolve the folder UUID by path
        match super::folders::resolve_folder_uuid_by_path(&mut api, &tenant, path).await {
            Ok(uuid) => uuid,
            Err(CliError::FolderNotFound(_)) => {
                // Folder doesn't exist, create it
                trace!("Folder does not exist, creating new folder with path: {}", path);
                
                // Extract folder name from the path
                let folder_name = Path::new(path)
                    .file_name()
                    .ok_or_else(|| CliError::ActionError(crate::actions::CliActionError::IoError(
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid folder path")
                    )))?
                    .to_str()
                    .ok_or_else(|| CliError::ActionError(crate::actions::CliActionError::IoError(
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid folder name encoding")
                    )))?
                    .to_string();
                
                // Find parent folder UUID if path has multiple segments
                let parent_folder_path = if path.contains("/") {
                    let parent_path = Path::new(path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .ok_or_else(|| CliError::ActionError(crate::actions::CliActionError::IoError(
                            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid parent folder path")
                        )))?;
                    
                    if !parent_path.is_empty() && normalize_path(parent_path) != "/" {
                        Some(parent_path.to_string())
                    } else {
                        None // Root folder
                    }
                } else {
                    None
                };
                
                let parent_folder_uuid = if let Some(parent_path) = parent_folder_path {
                    Some(super::folders::resolve_folder_uuid_by_path(&mut api, &tenant, &parent_path).await?)
                } else {
                    None
                };
                
                // Create the new folder
                let new_folder_uuid = api.create_folder(&tenant.uuid, &folder_name, parent_folder_uuid).await?;
                trace!("Created new folder with UUID: {}", new_folder_uuid);
                new_folder_uuid
            },
            Err(e) => return Err(e),
        }
    } else {
        // Neither folder UUID nor path provided
        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
    };

    // Get the skip-existing flag
    let skip_existing = sub_matches.get_flag("skip-existing");

    // Read all files in the local directory
    let entries = std::fs::read_dir(local_dir_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Upload each file in the directory
    for entry in entries {
        let entry = entry
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;
        let file_path = entry.path();

        // Skip if it's a directory
        if file_path.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name_str = file_name.to_str()
            .ok_or_else(|| CliError::ActionError(crate::actions::CliActionError::IoError(
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file name encoding")
            )))?;

        trace!("Checking if asset exists: {}", file_name_str);

        // Check if an asset with the same name already exists in the folder
        let assets_response = api.list_assets_by_parent_folder_uuid(&tenant.uuid, Some(&folder_uuid)).await?;
        let asset_list: crate::model::AssetList = assets_response.into();
        
        let asset_exists = asset_list.get_all_assets().iter()
            .any(|asset| asset.name() == file_name_str);

        if asset_exists {
            if skip_existing {
                println!("Skipping existing asset: {}", file_name_str);
                continue;
            } else {
                return Err(CliError::ActionError(crate::actions::CliActionError::BusinessLogicError(
                    format!("Asset already exists: {}. Use --skip-existing to skip existing assets.", file_name_str)
                )));
            }
        }

        // Upload the file
        trace!("Uploading asset: {} to folder UUID: {}", file_name_str, folder_uuid);
        
        // Read the file content
        let file_content = std::fs::read(&file_path)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;
        
        // Create a temporary file to pass to the API
        let temp_file = std::env::temp_dir().join(file_name_str);
        std::fs::write(&temp_file, &file_content)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;
        
        // Upload the asset to the specified folder
        let upload_result = api.create_asset(&tenant.uuid, &temp_file, Some(&folder_uuid)).await;
        
        // Clean up the temporary file
        let _ = std::fs::remove_file(&temp_file);
        
        match upload_result {
            Ok(_) => {
                println!("Successfully uploaded: {}", file_name_str);
            },
            Err(e) => {
                return Err(CliError::PhysnaExtendedApiError(e));
            }
        }
    }

    Ok(())
}
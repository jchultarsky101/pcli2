use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;
use crate::{commands::params::{PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_NAME, PARAMETER_PARENT_FOLDER_PATH, PARAMETER_PARENT_FOLDER_UUID}, configuration::Configuration, error::CliError, folder_hierarchy::FolderHierarchy, format::{OutputFormat, OutputFormatter}, model::{Folder, Tenant, normalize_path}, param_utils::{get_format_parameter_value, get_tenant}, physna_v3::{PhysnaApiClient, TryDefault}};


pub async fn resolve_folder_uuid_by_path(api: &mut PhysnaApiClient, tenant: &Tenant, path: &str) -> Result<Uuid, CliError> {
    trace!("Resolving the UUID for folder path {}...", path);

    // Root path should be handled separately by the calling function, so this function is only for non-root paths
    match api.get_folder_uuid_by_path(&tenant.uuid, path).await {
        Ok(Some(folder_uuid)) => Ok(folder_uuid),
        Ok(None) => Err(CliError::FolderNotFound(path.to_string())),
        Err(api_error) => {
            // Propagate API errors (like authentication errors) instead of converting them to FolderNotFound
            Err(CliError::PhysnaExtendedApiError(api_error))
        }
    }
}

pub async fn list_folders(sub_matches: &ArgMatches) -> Result<(), CliError> {

    trace!("Listing folders...");
    
    let format = get_format_parameter_value(sub_matches).await;
    let configuration = Configuration::load_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let path = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    let path = normalize_path(path.cloned().unwrap_or_default());
    trace!("Path requested: \"{}\"", &path);

    // It is not efficient, but the only option is to read the full directory hieratchy from the API
    let hierarchy = FolderHierarchy::build_from_api(&mut api, &tenant.uuid).await?;

    
    // If tree format is requested, display the hierarchical tree structure
    match format {
        OutputFormat::Tree(_) => {
                let hierarchy = if path.eq("/") {
                        hierarchy
                    } else {
                        hierarchy.filter_by_path(path.as_str()).ok_or(CliError::FolderNotFound(path))?
                    };
                hierarchy.print_tree();
            }
        _ => {
                // Convert to folder list with only direct children for non-tree formats
                let folder_list = if path.eq("/") {
                        hierarchy.to_direct_children_list()
                    } else {
                        // Use get_children_by_path to get only direct children, not all descendants
                        hierarchy.get_children_by_path(path.as_str()).ok_or(CliError::FolderNotFound(path))?
                    };

                println!("{}", folder_list.format(format)?);
            }
    }

	Ok(())
}

pub async fn print_folder_details(sub_matches: &ArgMatches) -> Result<(), CliError> {
    
    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    
    // Must provide either UUID or path
    if folder_uuid_param.is_none() && folder_path_param.is_none() {
        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
    }

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path doesn't have a specific UUID, so this should be handled differently
            // For get operations, we might need to list root contents instead
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
    };
            
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder: Folder = folder.into();
    match folder_path_param {
        Some(path) => folder.set_path(path.to_owned()),
        None => (),
    }
    println!("{}", folder.format(format)?);

    Ok(())
}

pub async fn rename_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    let new_name = sub_matches.get_one::<String>(PARAMETER_NAME)
        .ok_or(CliError::MissingRequiredArgument(PARAMETER_NAME.to_string()))?
        .clone();

    // Validate that only one folder parameter is provided (mutual exclusivity handled by clap group)
    if folder_uuid_param.is_some() && folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --folder-uuid or --folder-path can be specified, not both".to_string()));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve folder ID from either ID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path doesn't have a specific UUID, so this operation is not valid
            return Err(CliError::MissingRequiredArgument("Cannot rename the root folder".to_string()));
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        return Err(CliError::MissingRequiredArgument(format!("Missing folder identifier")));
    };

    // Attempt to rename the folder
    if let Err(e) = api.rename_folder(&tenant.uuid.to_string(), &folder_uuid.to_string(), &new_name).await {
        // If we got here, the folder was successfully found/resolved, but the rename operation failed
        // This could be due to permissions, API endpoint issues, etc.
        return Err(CliError::FolderRenameFailed(folder_uuid.to_string(), e.to_string()));
    }

    Ok(())
}

pub async fn move_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    let parent_folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_PARENT_FOLDER_UUID);
    let parent_folder_path_param = sub_matches.get_one::<String>(PARAMETER_PARENT_FOLDER_PATH);

    // Validate that only one folder parameter is provided (mutual exclusivity handled by clap group)
    if folder_uuid_param.is_some() && folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --folder-uuid or --folder-path can be specified, not both".to_string()));
    }

    // Validate that only one parent folder parameter is provided (mutual exclusivity handled by clap group)
    if parent_folder_uuid_param.is_some() && parent_folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --parent-folder-uuid or --parent-folder-path can be specified, not both".to_string()));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve folder ID from either ID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        return Err(CliError::MissingRequiredArgument(format!("Missing folder identifier")));
    };

    // Resolve parent folder ID from either ID parameter or path
    let parent_folder_uuid: Option<Uuid> = if let Some(uuid) = parent_folder_uuid_param {
        Some(uuid.clone())
    } else if let Some(path) = parent_folder_path_param {
        // Use get_folder_uuid_by_path to get the actual UUID, then handle root case separately
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path means no parent (None)
            None
        } else {
            Some(resolve_folder_uuid_by_path(&mut api, &tenant, path).await?)
        }
    } else {
        // If no parent is specified, move to root (None)
        None
    };

    api.move_folder(&tenant.uuid.to_string(), &folder_uuid.to_string(), parent_folder_uuid).await?;

    Ok(())
}

pub async fn create_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    
    let name = sub_matches.get_one::<String>(PARAMETER_NAME)
        .ok_or(CliError::MissingRequiredArgument(PARAMETER_NAME.to_string()))?
        .clone();
    let parent_folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_PARENT_FOLDER_UUID);
    let parent_folder_path_param = sub_matches.get_one::<String>(PARAMETER_PARENT_FOLDER_PATH);

    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
    if parent_folder_uuid_param.is_some() && parent_folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --parent-folder-uuid or --parent-folder-path can be specified, not both".to_string()));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve parent folder ID from either ID parameter or path
    let parent_folder_uuid = if let Some(uuid) = parent_folder_uuid_param {
        Some(uuid.clone())
    } else if let Some(path) = parent_folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path means no parent (None)
            None
        } else {
            Some(resolve_folder_uuid_by_path(&mut api, &tenant, path).await?)
        }
    } else {
        None
    };

    api.create_folder(&tenant.uuid, name.as_str(), parent_folder_uuid).await?;
    
    Ok(())
}

pub async fn delete_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {

    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    let force_flag = sub_matches.get_flag("force");

    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
    if folder_uuid_param.is_some() && folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --folder-uuid or --folder-path can be specified, not both".to_string()));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve parent folder ID from either ID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path doesn't have a specific UUID, so this operation is not valid
            return Err(CliError::MissingRequiredArgument("Cannot delete the root folder".to_string()));
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        return Err(CliError::MissingRequiredArgument(format!("Missing folder path")));
    };

    if force_flag {
        // Recursively delete all contents before deleting the folder
        delete_folder_contents(&mut api, &tenant, &folder_uuid).await?;
    }

    api.delete_folder(&tenant.uuid, &folder_uuid).await?;

    Ok(())
}

use std::collections::VecDeque;

/// Delete all assets and subfolders in a folder using iterative approach to avoid recursion
async fn delete_folder_contents(api: &mut PhysnaApiClient, tenant: &crate::model::Tenant, folder_uuid: &Uuid) -> Result<(), CliError> {
    // Use BFS to collect all folders in the hierarchy
    let mut all_folders = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(*folder_uuid);

    while let Some(current_folder_uuid) = queue.pop_front() {
        all_folders.push(current_folder_uuid);

        // Get subfolders of current folder
        let folders_response = api.list_folders_in_parent(&tenant.uuid, Some(&current_folder_uuid.to_string()), None, None).await?;
        for folder in folders_response.folders {
            queue.push_back(folder.uuid);
        }
    }

    // Process folders in reverse order (deepest first, then parent)
    for folder_uuid in all_folders.iter().rev() {
        // List and delete all assets in this folder
        let assets = api.list_assets_by_parent_folder_uuid(&tenant.uuid, Some(folder_uuid)).await?;
        for asset in assets.get_all_assets() {
            api.delete_asset(&tenant.uuid.to_string(), &asset.uuid().to_string()).await?;
        }
    }

    // Now delete the folders in reverse order (children first, then parents)
    // Skip the original folder since that will be deleted by the caller
    for folder_uuid in all_folders.iter().skip(1).rev() {
        api.delete_folder(&tenant.uuid, folder_uuid).await?;
    }

    Ok(())
}

pub async fn resolve_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Resolving folder path to UUID...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let folder_path = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument("folder-path is required".to_string()))?;

    trace!("Resolving path: {}", folder_path);

    // Special handling for root path "/"
    if crate::model::normalize_path(folder_path) == "/" {
        // The root path "/" doesn't correspond to a specific folder UUID
        // It represents the root level which contains multiple folders
        // We should return a special indication rather than an error
        println!("ROOT");
        return Ok(());
    }

    match api.get_folder_uuid_by_path(&tenant.uuid, folder_path).await? {
        Some(uuid) => {
            println!("{}", uuid);
            Ok(())
        },
        None => {
            Err(CliError::FolderNotFound(folder_path.clone()))
        }
    }
}

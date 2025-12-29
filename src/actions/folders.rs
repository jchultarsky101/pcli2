use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;
use crate::{commands::params::{PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_NAME, PARAMETER_PATH, PARAMETER_UUID}, configuration::Configuration, error::CliError, folder_hierarchy::FolderHierarchy, format::{OutputFormat, OutputFormatter}, model::{Folder, Tenant, normalize_path}, param_utils::{get_format_parameter_value, get_tenant}, physna_v3::{PhysnaApiClient, TryDefault}};


pub async fn resolve_folder_uuid_by_path(api: &mut PhysnaApiClient, tenant: &Tenant, path: &str) -> Result<Uuid, CliError> {
    trace!("Resolving the UUID for folder path {}...", path);
    if let Some(folder_uuid) = api.get_folder_uuid_by_path(&tenant.uuid, path).await? {
        Ok(folder_uuid)
    } else {
        Err(CliError::FolderNotFound(path.to_string()))
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
                // Convert to folder list with only direct children if not recursive
                let folder_list = if path.eq("/") {
                        hierarchy.to_direct_children_list()
                    } else {
                        hierarchy.filter_by_path(path.as_str()).ok_or(CliError::FolderNotFound(path))?.to_folder_list()
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
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
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

pub async fn create_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    
    let name = sub_matches.get_one::<String>(PARAMETER_NAME)
        .ok_or(CliError::MissingRequiredArgument(PARAMETER_NAME.to_string()))?
        .clone();
    let parent_folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let parent_folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
    if parent_folder_uuid_param.is_some() && parent_folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --parent-folder-id or --path can be specified, not both".to_string()));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve parent folder ID from either ID parameter or path
    let parent_folder_uuid = if let Some(uuid) = parent_folder_uuid_param {
        Some(uuid.clone())
    } else if let Some(path) = parent_folder_path_param {
        Some(resolve_folder_uuid_by_path(&mut api, &tenant, path).await?)
    } else {
        None
    };

    api.create_folder(&tenant.uuid, name.as_str(), parent_folder_uuid).await?;
    
    Ok(())
}

pub async fn delete_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
    if folder_uuid_param.is_some() && folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument("Only one of --uuid or --path can be specified, not both".to_string()));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve parent folder ID from either ID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        uuid.clone()
    } else if let Some(path) = folder_path_param {
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        return Err(CliError::MissingRequiredArgument(format!("Missing folder path")));        
    };

    api.delete_folder(&tenant.uuid, &folder_uuid).await?;
    
    Ok(())
}

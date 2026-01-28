use clap::ArgMatches;
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use tracing::trace;
use uuid::Uuid;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use zip::{ZipWriter, write::FileOptions};
use tokio::sync::Semaphore;
use std::sync::Arc;
use tokio::time::sleep;
use std::time::Duration;
use crate::physna_v3::ApiError;

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

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Extract tenant before calling resolve_folder to avoid borrowing conflicts
    let tenant = ctx.tenant().clone();

    // Resolve folder using the helper function
    let mut folder: Folder = crate::actions::utils::resolve_folder(
        ctx.api(),
        &tenant,
        folder_uuid_param,
        folder_path_param
    ).await?;

    // Set path if provided in parameters
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned());
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

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    // Extract tenant before calling resolve_folder to avoid borrowing conflicts
    let tenant = ctx.tenant().clone();

    // Resolve folder using the helper function
    let folder: Folder = crate::actions::utils::resolve_folder(
        ctx.api(),
        &tenant,
        folder_uuid_param,
        folder_path_param
    ).await?;

    // Check if trying to rename the root folder
    if folder_path_param.map_or(false, |p| crate::model::normalize_path(p) == "/") {
        return Err(CliError::MissingRequiredArgument("Cannot rename the root folder".to_string()));
    }

    // Extract tenant UUID before calling rename_folder to avoid borrowing conflicts
    let tenant_uuid = tenant.uuid;

    // Attempt to rename the folder
    if let Err(e) = ctx.api().rename_folder(&tenant_uuid.to_string(), &folder.uuid().to_string(), &new_name).await {
        // If we got here, the folder was successfully found/resolved, but the rename operation failed
        // This could be due to permissions, API endpoint issues, etc.
        return Err(CliError::FolderRenameFailed(folder.uuid().to_string(), e.to_string()));
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

    match api.delete_folder(&tenant.uuid, &folder_uuid, force_flag).await {
        Ok(()) => Ok(()),
        Err(api_error) => {
            // Check if this is a 404 error on a folder deletion, which likely means the folder is not empty
            if api_error.to_string().contains("404 Not Found") && !force_flag {
                // The folder exists (we resolved the UUID successfully) but can't be deleted because it's not empty
                return Err(CliError::ActionError(crate::actions::CliActionError::BusinessLogicError(
                    format!("Folder is not empty. Use --force flag to delete the folder and all its contents recursively.")
                )));
            }
            Err(CliError::PhysnaExtendedApiError(api_error))
        }
    }
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

/// Download all assets in a folder and its subfolders as a ZIP archive.
///
/// This function handles the "folder download" command, retrieving all assets
/// in a specified folder and all its subfolders from the Physna API and packaging them into a ZIP file,
/// preserving the folder structure.
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
    trace!("Executing \"folder download\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder UUID or path from command line
    let folder_uuid_param = sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        // Resolve folder UUID by path
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument("Either folder UUID or path must be provided".to_string()));
    };

    // Get the output file path
    let output_file_path = if let Some(output_path) = sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_OUTPUT) {
        output_path.clone()
    } else {
        // Use the folder name as the default output file name
        // Determine the folder name from the provided path or get it from the folder details
        let folder_name = if let Some(path) = folder_path_param {
            // If the folder was specified by path, extract the folder name from the path
            // Special handling for root folder "/"
            if path.trim() == "/" {
                // Use tenant name for root folder
                tenant.name.clone()
            } else {
                let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                if path_segments.is_empty() {
                    "untitled".to_string()
                } else {
                    path_segments.last().unwrap().to_string()
                }
            }
        } else {
            // If the folder was specified by UUID, get the folder details to determine the name
            let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
            let folder: crate::model::Folder = folder.into();

            let folder_path = folder.path();
            // Special handling for root folder
            if folder_path.trim() == "/" {
                // Use tenant name for root folder
                tenant.name.clone()
            } else {
                let path_segments: Vec<&str> = folder_path.split('/').filter(|s| !s.is_empty()).collect();
                if path_segments.is_empty() {
                    "untitled".to_string()
                } else {
                    path_segments.last().unwrap().to_string()
                }
            }
        };

        let mut path = std::path::PathBuf::new();
        path.push(format!("{}.zip", folder_name));
        path
    };

    // Create a temporary directory to store downloaded files
    let temp_dir = std::env::temp_dir().join(format!("pcli2_temp_{}", folder_uuid));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Use BFS to collect all folders in the hierarchy and their assets
    let mut all_assets_with_paths = Vec::new();
    let mut folder_queue = std::collections::VecDeque::new();

    // Get the root folder details to determine its path
    let root_folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let root_folder: crate::model::Folder = root_folder.into();
    let root_folder_path = root_folder.path();

    // Start BFS with the specified folder
    folder_queue.push_back((folder_uuid, root_folder_path.clone()));

    while let Some((current_folder_uuid, current_folder_path)) = folder_queue.pop_front() {
        // Get all assets in the current folder
        let assets_response = api.list_assets_by_parent_folder_uuid(&tenant.uuid, Some(&current_folder_uuid)).await?;
        let asset_list: crate::model::AssetList = assets_response.into();

        // Add assets with their relative paths
        for asset in asset_list.get_all_assets() {
            // Only include assets with "finished" state in the download queue
            if asset.normalized_processing_status() != "finished" {
                continue;
            }

            // Calculate the relative path from the root folder
            let mut asset_name_for_path = asset.name().to_string();

            // If the asset is an assembly, change the extension to .zip since assemblies download as ZIP files
            if asset.is_assembly() {
                let path = std::path::Path::new(&asset_name_for_path);
                let stem = path.file_stem().unwrap_or(std::ffi::OsStr::new(&asset_name_for_path));
                if let Some(stem_str) = stem.to_str() {
                    asset_name_for_path = format!("{}.zip", stem_str);
                }
            }

            let relative_path = if current_folder_path == root_folder_path {
                // If it's the root folder, just use the asset name (with .zip extension if assembly)
                asset_name_for_path
            } else {
                // Otherwise, create a subfolder path by removing the root folder path prefix
                // For example, if root is "/Julian/sub1" and current is "/Julian/sub1/sub2",
                // the relative path becomes "sub2/asset_name"
                let relative_folder_path = current_folder_path.strip_prefix(&root_folder_path)
                    .unwrap_or(&current_folder_path)  // fallback if strip_prefix fails
                    .trim_start_matches('/')  // remove leading slash
                    .trim_end_matches('/');   // remove trailing slash

                if relative_folder_path.is_empty() {
                    asset_name_for_path
                } else {
                    format!("{}/{}", relative_folder_path, asset_name_for_path)
                }
            };

            // Use the asset's original path as the physna_path
            let physna_path = asset.path().clone();

            all_assets_with_paths.push((asset.clone(), relative_path, physna_path));
        }

        // Get subfolders of current folder to process next
        // Use get_folder_contents to get only direct children of the current folder
        let subfolders_response = api.get_folder_contents(&tenant.uuid, Some(&current_folder_uuid), "folders", Some(1), Some(1000)).await?;
        for folder in subfolders_response.folders() {
            // Get the full folder details to get the name
            let folder_detail = api.get_folder(&tenant.uuid, &folder.uuid()).await?;
            let folder_detail: crate::model::Folder = folder_detail.into();

            // Get the folder path by appending the folder name to the current path
            let folder_path = if current_folder_path.ends_with('/') {
                format!("{}{}", current_folder_path, folder_detail.name())
            } else {
                format!("{}/{}", current_folder_path, folder_detail.name())
            };

            // Add to queue to process this subfolder
            folder_queue.push_back((*folder.uuid(), folder_path));
        }
    }

    if all_assets_with_paths.is_empty() {
        crate::error_utils::report_error_with_remediation(
            &format!("No assets found in folder with UUID: {} or its subfolders", folder_uuid),
            &[
                "Verify the folder UUID or path is correct",
                "Check that the folder or its subfolders contain assets",
                "Ensure you have permissions to access the folder"
            ]
        );
        return Ok(());
    }

    // Get the new parameters
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);
    let concurrent_param = sub_matches.get_one::<usize>(crate::commands::params::PARAMETER_CONCURRENT).copied().unwrap_or(1);
    let continue_on_error = sub_matches.get_flag(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR);
    let delay_param = sub_matches.get_one::<usize>(crate::commands::params::PARAMETER_DELAY).copied().unwrap_or(0);

    // Validate concurrent parameter
    if concurrent_param < 1 || concurrent_param > 10 {
        return Err(CliError::MissingRequiredArgument(format!("Invalid value for '--concurrent': must be between 1 and 10, got {}", concurrent_param)));
    }

    // Validate delay parameter
    if delay_param > 180 {
        return Err(CliError::MissingRequiredArgument(format!("Invalid value for '--delay': must be between 0 and 180, got {}", delay_param)));
    }

    // Use a semaphore to limit concurrent operations
    let semaphore = Arc::new(Semaphore::new(concurrent_param));

    // Create progress bars if requested
    let (progress_bar, multi_progress) = if show_progress {
        let mp = MultiProgress::new();
        let pb = mp.add(ProgressBar::new(all_assets_with_paths.len() as u64));
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Overall progress")
            .unwrap()
            .progress_chars("#>-"));
        (Some(pb), Some(mp))
    } else {
        (None, None)
    };

    // Track errors if continue-on-error is enabled
    let mut error_count = 0;
    let mut success_count = 0;

    // Download each asset to the appropriate subdirectory in the temp directory
    let mut tasks = Vec::new();

    for (asset, relative_path, physna_path) in all_assets_with_paths {
        let tenant_id = tenant.uuid.to_string();
        let asset_id = asset.uuid().to_string();
        let asset_name = asset.name().to_string();
        let asset_file_path = temp_dir.join(&relative_path);
        let semaphore = semaphore.clone();
        let progress_bar_clone = progress_bar.clone();
        let multi_progress_clone = multi_progress.clone();
        let delay_duration = Duration::from_secs(delay_param as u64);
        let continue_on_error_clone = continue_on_error;
        let concurrent_param_clone = concurrent_param;

        // Spawn a task for each download
        let task = tokio::spawn(async move {
            // Acquire a permit from the semaphore to limit concurrency
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this download if concurrent > 1 and progress is enabled
            let individual_pb = if concurrent_param_clone > 1 && progress_bar_clone.is_some() {
                if let Some(ref mp) = multi_progress_clone {
                    let individual_pb = mp.add(ProgressBar::new_spinner()); // We'll update this later with actual size if known
                    individual_pb.set_style(ProgressStyle::default_bar()
                        .template(&format!("{{spinner:.yellow}} {{msg}}"))
                        .unwrap());
                    individual_pb.set_message(format!("Downloading: {}", asset_name));
                    Some(individual_pb)
                } else {
                    None
                }
            } else {
                None
            };

            // Add delay if specified
            if delay_param > 0 {
                sleep(delay_duration).await;
            }

            // Create a new API client for this task
            let mut api_task = match PhysnaApiClient::try_default() {
                Ok(client) => client,
                Err(e) => {
                    if continue_on_error_clone {
                        return Ok(Err((asset_name, physna_path, ApiError::from(e), true))); // true indicates it's a recoverable error
                    } else {
                        return Err(CliError::PhysnaExtendedApiError(ApiError::from(e)));
                    }
                }
            };

            // Attempt to download the asset
            let result = api_task.download_asset(&tenant_id, &asset_id).await;

            match result {
                Ok(file_content) => {
                    // Update individual progress bar
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Downloaded: {}", asset_name));
                        ipb.finish_and_clear(); // Clear the spinner for this individual download
                    }

                    // Create parent directories if they don't exist
                    if let Some(parent) = asset_file_path.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            if continue_on_error_clone {
                                return Ok(Err((asset_name, physna_path, ApiError::IoError(e), true)));
                            } else {
                                return Err(CliError::ActionError(crate::actions::CliActionError::IoError(e)));
                            }
                        }
                    }

                    let file_result = File::create(&asset_file_path);
                    match file_result {
                        Ok(mut file) => {
                            match file.write_all(&file_content) {
                                Ok(_) => {},
                                Err(e) => {
                                    if continue_on_error_clone {
                                        return Ok(Err((asset_name, physna_path, ApiError::IoError(e), true)));
                                    } else {
                                        return Err(CliError::ActionError(crate::actions::CliActionError::IoError(e)));
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            if continue_on_error_clone {
                                return Ok(Err((asset_name, physna_path, ApiError::IoError(e), true)));
                            } else {
                                return Err(CliError::ActionError(crate::actions::CliActionError::IoError(e)));
                            }
                        }
                    }

                    // Update overall progress bar if present
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }

                    Ok(Ok(asset_name))
                },
                Err(e) => {
                    // Update individual progress bar for error
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Failed: {} - {}", asset_name, e));
                        ipb.finish_and_clear(); // Clear the spinner for this individual download
                    }

                    // Log the detailed error for debugging with asset UUID and Physna path
                    tracing::error!("Failed to download asset '{}' (Asset UUID: {}, Physna path: {}): {}", asset_name, asset_id, physna_path, e);
                    tracing::debug!("Error details for asset '{}': error type = {:?}", asset_name, e);

                    // If continue-on-error is enabled, return the error as a warning instead of failing
                    if continue_on_error_clone {
                        Ok(Err((asset_name, physna_path, e, true))) // true indicates it's a recoverable error
                    } else {
                        Err(CliError::PhysnaExtendedApiError(e))
                    }
                }
            }
        });

        tasks.push(task);
    }


    // Wait for all tasks to complete
    for task in tasks {
        match task.await {
            Ok(task_result) => {
                match task_result {
                    Ok(asset_result) => {
                        match asset_result {
                            Ok(_asset_name) => {
                                success_count += 1;
                            },
                            Err((asset_name, physna_path, error, is_recoverable)) => {
                                if is_recoverable {
                                    error_count += 1;
                                    eprintln!("⚠️  Warning: Failed to download asset '{}' (Physna path: {}): {}", asset_name, physna_path, error);
                                } else {
                                    return Err(CliError::PhysnaExtendedApiError(error));
                                }
                            }
                        }
                    },
                    Err(cli_error) => {
                        if continue_on_error {
                            error_count += 1;
                            eprintln!("⚠️  Warning: Failed to download asset due to CLI error: {}", cli_error);
                        } else {
                            return Err(cli_error);
                        }
                    }
                }
            },
            Err(join_error) => {
                if continue_on_error {
                    error_count += 1;
                    eprintln!("⚠️  Warning: Task failed to execute: {}", join_error);
                } else {
                    return Err(CliError::ActionError(crate::actions::CliActionError::IoError(
                        std::io::Error::new(std::io::ErrorKind::Other, join_error.to_string())
                    )));
                }
            }
        }
    }

    // Finish progress bar if present
    if let Some(pb) = progress_bar {
        pb.finish_with_message("Assets downloaded, creating ZIP...");
    }

    // Report summary if continue-on-error was used
    if continue_on_error && (error_count > 0 || success_count > 0) {
        println!("✅ Successfully downloaded: {} assets", success_count);
        if error_count > 0 {
            eprintln!("❌ Failed to download: {} assets", error_count);
        }
    }

    // Create ZIP file with all downloaded assets, preserving folder structure
    let zip_file = File::create(&output_file_path).map_err(|e| crate::actions::CliActionError::IoError(e))?;

    // Create a progress bar for the zipping process if the original progress bar was shown
    let zip_progress_bar = if show_progress {
        let zip_pb = ProgressBar::new(100); // Generic progress bar for zipping
        zip_pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Creating ZIP archive...")
            .unwrap()
            .progress_chars("#>-"));
        zip_pb.set_message("Starting ZIP creation...");
        Some(zip_pb)
    } else {
        None
    };

    let mut zip_writer = ZipWriter::new(zip_file);

    // Walk through the temp directory recursively and add files to the ZIP
    add_files_to_zip_recursive_with_progress(&mut zip_writer, &temp_dir, &temp_dir, zip_progress_bar.as_ref())?;

    zip_writer.finish()
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;

    // Clean up temporary directory
    std::fs::remove_dir_all(&temp_dir)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    Ok(())
}


// Helper function to recursively add files to the ZIP archive while preserving folder structure with progress indication
fn add_files_to_zip_recursive_with_progress(
    zip_writer: &mut ZipWriter<File>,
    base_path: &std::path::Path,
    current_path: &std::path::Path,
    progress_bar: Option<&ProgressBar>,
) -> Result<(), CliError> {
    // Count total files first to provide progress indication
    let total_files = count_files_recursive(current_path)?;
    
    if let Some(pb) = progress_bar {
        pb.set_length(total_files as u64);
        pb.set_message("Counted files, starting ZIP creation...");
    }

    // Now actually add files to the ZIP with progress updates
    add_files_to_zip_recursive_with_progress_impl(zip_writer, base_path, current_path, progress_bar, &total_files)?;

    if let Some(pb) = progress_bar {
        pb.finish_with_message("ZIP archive created successfully!");
    }

    Ok(())
}

// Helper function to count total files recursively
fn count_files_recursive(current_path: &std::path::Path) -> Result<usize, CliError> {
    let mut count = 0;
    for entry in std::fs::read_dir(current_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))? {
        let entry = entry.map_err(|e| crate::actions::CliActionError::IoError(e))?;
        let path = entry.path();

        if path.is_file() {
            count += 1;
        } else if path.is_dir() {
            count += count_files_recursive(&path)?;
        }
    }
    Ok(count)
}

// Actual implementation of adding files to ZIP with progress
fn add_files_to_zip_recursive_with_progress_impl(
    zip_writer: &mut ZipWriter<File>,
    base_path: &std::path::Path,
    current_path: &std::path::Path,
    progress_bar: Option<&ProgressBar>,
    total_files: &usize,
) -> Result<(), CliError> {
    for entry in std::fs::read_dir(current_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))? {
        let entry = entry.map_err(|e| crate::actions::CliActionError::IoError(e))?;
        let path = entry.path();

        if path.is_file() {
            // Calculate the relative path from the base path
            let relative_path = path.strip_prefix(base_path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to strip prefix: {}", e)
                ))))?;

            let file_name = relative_path.to_str()
                .ok_or_else(|| CliError::ActionError(crate::actions::CliActionError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Invalid file path"
                ))))?;

            let options: FileOptions<()> = FileOptions::default();
            zip_writer.start_file(file_name, options)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;

            let file_content = std::fs::read(&path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            zip_writer.write_all(&file_content)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            // Update progress bar
            if let Some(pb) = progress_bar {
                pb.inc(1);
                pb.set_message(format!("Added file: {}", file_name));
            }
        } else if path.is_dir() {
            // Recursively process subdirectories
            add_files_to_zip_recursive_with_progress_impl(zip_writer, base_path, &path, progress_bar, total_files)?;
        }
    }

    Ok(())
}
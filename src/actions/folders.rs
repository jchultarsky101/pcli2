use crate::physna_v3::ApiError;
use clap::ArgMatches;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::trace;
use uuid::Uuid;

use crate::{
    commands::params::{
        PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_NAME, PARAMETER_PARENT_FOLDER_PATH,
        PARAMETER_PARENT_FOLDER_UUID,
    },
    configuration::Configuration,
    error::CliError,
    folder_hierarchy::FolderHierarchy,
    format::{OutputFormat, OutputFormatter},
    model::{normalize_path, Folder, Tenant},
    param_utils::{get_format_parameter_value, get_tenant},
    physna_v3::{PhysnaApiClient, TryDefault},
};

pub async fn resolve_folder_uuid_by_path(
    api: &mut PhysnaApiClient,
    tenant: &Tenant,
    path: &str,
) -> Result<Uuid, CliError> {
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
    let path = if let Some(path) = path {
        normalize_path(path.clone())
    } else {
        // If no path is provided, default to root path
        "/".to_string()
    };
    trace!("Path requested: \"{}\"", &path);

    // It is not efficient, but the only option is to read the full directory hieratchy from the API
    let hierarchy = FolderHierarchy::build_from_api(&mut api, &tenant.uuid).await?;

    // If tree format is requested, display the hierarchical tree structure
    match format {
        OutputFormat::Tree(_) => {
            let hierarchy = if path.eq("/") {
                hierarchy
            } else {
                hierarchy
                    .filter_by_path(path.as_str())
                    .ok_or(CliError::FolderNotFound(path))?
            };
            hierarchy.print_tree();
        }
        _ => {
            // Convert to folder list with only direct children for non-tree formats
            let folder_list = if path.eq("/") {
                hierarchy.to_direct_children_list()
            } else {
                // Use get_children_by_path to get only direct children, not all descendants
                hierarchy
                    .get_children_by_path(path.as_str())
                    .ok_or(CliError::FolderNotFound(path))?
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
        folder_path_param,
    )
    .await?;

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
    let new_name = sub_matches
        .get_one::<String>(PARAMETER_NAME)
        .ok_or(CliError::MissingRequiredArgument(
            PARAMETER_NAME.to_string(),
        ))?
        .clone();

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    // Extract tenant before calling resolve_folder to avoid borrowing conflicts
    let tenant = ctx.tenant().clone();

    // Resolve folder using the helper function
    let folder: Folder = crate::actions::utils::resolve_folder(
        ctx.api(),
        &tenant,
        folder_uuid_param,
        folder_path_param,
    )
    .await?;

    // Check if trying to rename the root folder
    if folder_path_param.is_some_and(|p| crate::model::normalize_path(p) == "/") {
        return Err(CliError::MissingRequiredArgument(
            "Cannot rename the root folder".to_string(),
        ));
    }

    // Extract tenant UUID before calling rename_folder to avoid borrowing conflicts
    let tenant_uuid = tenant.uuid;

    // Attempt to rename the folder
    if let Err(e) = ctx
        .api()
        .rename_folder(
            &tenant_uuid.to_string(),
            &folder.uuid().to_string(),
            &new_name,
        )
        .await
    {
        // If we got here, the folder was successfully found/resolved, but the rename operation failed
        // This could be due to permissions, API endpoint issues, etc.
        return Err(CliError::FolderRenameFailed(
            folder.uuid().to_string(),
            e.to_string(),
        ));
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
        return Err(CliError::MissingRequiredArgument(
            "Only one of --folder-uuid or --folder-path can be specified, not both".to_string(),
        ));
    }

    // Validate that only one parent folder parameter is provided (mutual exclusivity handled by clap group)
    if parent_folder_uuid_param.is_some() && parent_folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument(
            "Only one of --parent-folder-uuid or --parent-folder-path can be specified, not both"
                .to_string(),
        ));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve folder ID from either ID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        return Err(CliError::MissingRequiredArgument("Missing folder identifier".to_string()));
    };

    // Resolve parent folder ID from either ID parameter or path
    let parent_folder_uuid: Option<Uuid> = if let Some(uuid) = parent_folder_uuid_param {
        Some(*uuid)
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

    api.move_folder(
        &tenant.uuid.to_string(),
        &folder_uuid.to_string(),
        parent_folder_uuid,
    )
    .await?;

    Ok(())
}

pub async fn create_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let name = sub_matches
        .get_one::<String>(PARAMETER_NAME)
        .ok_or(CliError::MissingRequiredArgument(
            PARAMETER_NAME.to_string(),
        ))?
        .clone();
    let parent_folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_PARENT_FOLDER_UUID);
    let parent_folder_path_param = sub_matches.get_one::<String>(PARAMETER_PARENT_FOLDER_PATH);

    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
    if parent_folder_uuid_param.is_some() && parent_folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument(
            "Only one of --parent-folder-uuid or --parent-folder-path can be specified, not both"
                .to_string(),
        ));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve parent folder ID from either ID parameter or path
    let parent_folder_uuid = if let Some(uuid) = parent_folder_uuid_param {
        Some(*uuid)
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

    api.create_folder(&tenant.uuid, name.as_str(), parent_folder_uuid)
        .await?;

    Ok(())
}

pub async fn delete_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    let force_flag = sub_matches.get_flag("force");

    // Validate that only one parent parameter is provided (mutual exclusivity handled by clap group)
    if folder_uuid_param.is_some() && folder_path_param.is_some() {
        return Err(CliError::MissingRequiredArgument(
            "Only one of --folder-uuid or --folder-path can be specified, not both".to_string(),
        ));
    }

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Resolve parent folder ID from either ID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path doesn't have a specific UUID, so this operation is not valid
            return Err(CliError::MissingRequiredArgument(
                "Cannot delete the root folder".to_string(),
            ));
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        return Err(CliError::MissingRequiredArgument("Missing folder path".to_string()));
    };

    match api
        .delete_folder(&tenant.uuid, &folder_uuid, force_flag)
        .await
    {
        Ok(()) => Ok(()),
        Err(api_error) => {
            // Check if this is a 404 error on a folder deletion, which likely means the folder is not empty
            if api_error.to_string().contains("404 Not Found") && !force_flag {
                // The folder exists (we resolved the UUID successfully) but can't be deleted because it's not empty
                return Err(CliError::ActionError(crate::actions::CliActionError::BusinessLogicError(
                    "Folder is not empty. Use --force flag to delete the folder and all its contents recursively.".to_string()
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

    let folder_path = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH).ok_or(
        CliError::MissingRequiredArgument("folder-path is required".to_string()),
    )?;

    trace!("Resolving path: {}", folder_path);

    // Special handling for root path "/"
    if crate::model::normalize_path(folder_path) == "/" {
        // The root path "/" doesn't correspond to a specific folder UUID
        // It represents the root level which contains multiple folders
        // We should return a special indication rather than an error
        println!("ROOT");
        return Ok(());
    }

    match api
        .get_folder_uuid_by_path(&tenant.uuid, folder_path)
        .await?
    {
        Some(uuid) => {
            println!("{}", uuid);
            Ok(())
        }
        None => Err(CliError::FolderNotFound(folder_path.clone())),
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
    let folder_uuid_param =
        sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_FOLDER_UUID);
    let folder_path_param =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        // Resolve folder UUID by path
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_OUTPUT)
    {
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
            let folder: crate::model::Folder = folder;

            let folder_path = folder.path();
            // Special handling for root folder
            if folder_path.trim() == "/" {
                // Use tenant name for root folder
                tenant.name.clone()
            } else {
                let path_segments: Vec<&str> =
                    folder_path.split('/').filter(|s| !s.is_empty()).collect();
                if path_segments.is_empty() {
                    "untitled".to_string()
                } else {
                    path_segments.last().unwrap().to_string()
                }
            }
        };

        let mut path = std::path::PathBuf::new();
        path.push(folder_name);
        path
    };

    // Use the destination directory directly instead of a temporary directory to avoid cross-device issues
    let dest_dir = if output_file_path.is_file() {
        // If output is a file, use its parent directory
        output_file_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf()
    } else {
        // If output is a directory, use it directly
        output_file_path.clone()
    };
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Use BFS to collect all folders in the hierarchy and their assets
    let mut all_assets_with_paths = Vec::new();
    let mut folder_queue = std::collections::VecDeque::new();

    // Get the root folder details to determine its path
    let root_folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let root_folder: crate::model::Folder = root_folder;
    let root_folder_path = root_folder.path();

    // Start BFS with the specified folder
    folder_queue.push_back((folder_uuid, root_folder_path.clone()));

    while let Some((current_folder_uuid, current_folder_path)) = folder_queue.pop_front() {
        // Get all assets in the current folder
        let assets_response = api
            .list_assets_by_parent_folder_uuid(&tenant.uuid, Some(&current_folder_uuid))
            .await?;
        let asset_list: crate::model::AssetList = assets_response;

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
                let stem = path
                    .file_stem()
                    .unwrap_or(std::ffi::OsStr::new(&asset_name_for_path));
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
                let relative_folder_path = current_folder_path
                    .strip_prefix(&root_folder_path)
                    .unwrap_or(&current_folder_path) // fallback if strip_prefix fails
                    .trim_start_matches('/') // remove leading slash
                    .trim_end_matches('/'); // remove trailing slash

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
        let subfolders_response = api
            .get_folder_contents(
                &tenant.uuid,
                Some(&current_folder_uuid),
                "folders",
                Some(1),
                Some(1000),
            )
            .await?;
        for folder in subfolders_response.folders() {
            // Get the full folder details to get the name
            let folder_detail = api.get_folder(&tenant.uuid, folder.uuid()).await?;
            let folder_detail: crate::model::Folder = folder_detail;

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
            &format!(
                "No assets found in folder with UUID: {} or its subfolders",
                folder_uuid
            ),
            &[
                "Verify the folder UUID or path is correct",
                "Check that the folder or its subfolders contain assets",
                "Ensure you have permissions to access the folder",
            ],
        );
        return Ok(());
    }

    // Get the new parameters
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);
    let concurrent_param = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_CONCURRENT)
        .copied()
        .unwrap_or(1);
    let continue_on_error =
        sub_matches.get_flag(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR);
    let delay_param = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_DELAY)
        .copied()
        .unwrap_or(0);
    let resume_flag = sub_matches.get_flag(crate::commands::params::PARAMETER_RESUME);

    // Validate concurrent parameter
    if !(1..=10).contains(&concurrent_param) {
        return Err(CliError::MissingRequiredArgument(format!(
            "Invalid value for '--concurrent': must be between 1 and 10, got {}",
            concurrent_param
        )));
    }

    // Validate delay parameter
    if delay_param > 180 {
        return Err(CliError::MissingRequiredArgument(format!(
            "Invalid value for '--delay': must be between 0 and 180, got {}",
            delay_param
        )));
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
    let total_assets = all_assets_with_paths.len(); // Store the length before moving the vector

    // Download each asset to the appropriate subdirectory in the temp directory
    let mut tasks = Vec::new();

    for (asset, relative_path, physna_path) in all_assets_with_paths {
        let tenant_id = tenant.uuid.to_string();
        let asset_id = asset.uuid().to_string();
        let asset_name = asset.name().to_string();
        let asset_file_path = dest_dir.join(&relative_path);
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
                    individual_pb.set_style(
                        ProgressStyle::default_bar()
                            .template(&"{spinner:.yellow} {msg}".to_string())
                            .unwrap(),
                    );
                    individual_pb.set_message(format!("Downloading: {}", asset_name));
                    Some(individual_pb)
                } else {
                    None
                }
            } else {
                None
            };

            // Check if resume flag is set and file already exists
            if resume_flag && asset_file_path.exists() {
                tracing::debug!("Skipping existing file: {}", asset_file_path.display());

                // Update overall progress bar if present
                if let Some(ref pb) = progress_bar_clone {
                    pb.inc(1);
                }

                return Ok(Ok(asset_name));
            }

            // Add delay if specified (only when actually downloading, not when skipping)
            if delay_param > 0 {
                sleep(delay_duration).await;
            }

            // Create a new API client for this task
            let mut api_task = match PhysnaApiClient::try_default() {
                Ok(client) => client,
                Err(e) => {
                    if continue_on_error_clone {
                        return Ok(Err((asset_name, physna_path, e, true)));
                    // true indicates it's a recoverable error
                    } else {
                        return Err(CliError::PhysnaExtendedApiError(e));
                    }
                }
            };

            // Download the asset file with retry logic (similar to asset download)
            let file_content =
                download_asset_with_retry(&mut api_task, &tenant_id, &asset_id, &asset_name).await;

            match file_content {
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
                                return Ok(Err((
                                    asset_name,
                                    physna_path,
                                    ApiError::IoError(e),
                                    true,
                                )));
                            } else {
                                return Err(CliError::ActionError(
                                    crate::actions::CliActionError::IoError(e),
                                ));
                            }
                        }
                    }

                    let file_result = File::create(&asset_file_path);
                    match file_result {
                        Ok(mut file) => match file.write_all(&file_content) {
                            Ok(_) => {}
                            Err(e) => {
                                if continue_on_error_clone {
                                    return Ok(Err((
                                        asset_name,
                                        physna_path,
                                        ApiError::IoError(e),
                                        true,
                                    )));
                                } else {
                                    return Err(CliError::ActionError(
                                        crate::actions::CliActionError::IoError(e),
                                    ));
                                }
                            }
                        },
                        Err(e) => {
                            if continue_on_error_clone {
                                return Ok(Err((
                                    asset_name,
                                    physna_path,
                                    ApiError::IoError(e),
                                    true,
                                )));
                            } else {
                                return Err(CliError::ActionError(
                                    crate::actions::CliActionError::IoError(e),
                                ));
                            }
                        }
                    }

                    // If the asset is an assembly, extract the ZIP file contents and delete the original ZIP
                    if asset.is_assembly() {
                        match extract_zip_and_cleanup(&asset_file_path) {
                            Ok(_) => {
                                tracing::debug!(
                                    "Successfully extracted assembly ZIP file: {}",
                                    asset_file_path.display()
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to extract assembly ZIP file: {}: {}",
                                    asset_file_path.display(),
                                    e
                                );
                                if continue_on_error_clone {
                                    return Ok(Err((
                                        asset_name,
                                        physna_path,
                                        ApiError::IoError(std::io::Error::other(
                                            format!("Failed to extract ZIP file: {}", e),
                                        )),
                                        true,
                                    )));
                                } else {
                                    return Err(CliError::ActionError(
                                        crate::actions::CliActionError::IoError(
                                            std::io::Error::other(
                                                format!("Failed to extract ZIP file: {}", e),
                                            ),
                                        ),
                                    ));
                                }
                            }
                        }
                    }

                    // Update overall progress bar if present
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }

                    Ok(Ok(asset_name))
                }
                Err(e) => {
                    // Update individual progress bar for error
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Failed: {} - {}", asset_name, e));
                        ipb.finish_and_clear(); // Clear the spinner for this individual download
                    }

                    // Log the detailed error for debugging with asset UUID and Physna path
                    tracing::error!(
                        "Failed to download asset '{}' (Asset UUID: {}, Physna path: {}): {}",
                        asset_name,
                        asset_id,
                        physna_path,
                        e
                    );
                    tracing::debug!(
                        "Error details for asset '{}': error type = {:?}",
                        asset_name,
                        e
                    );

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
                    Ok(asset_result) => match asset_result {
                        Ok(_asset_name) => {
                            success_count += 1;
                        }
                        Err((asset_name, physna_path, error, is_recoverable)) => {
                            if is_recoverable {
                                error_count += 1;
                                eprintln!("⚠️  Warning: Failed to download asset '{}' (Physna path: {}): {}", asset_name, physna_path, error);
                            } else {
                                return Err(CliError::PhysnaExtendedApiError(error));
                            }
                        }
                    },
                    Err(cli_error) => {
                        if continue_on_error {
                            error_count += 1;
                            eprintln!(
                                "⚠️  Warning: Failed to download asset due to CLI error: {}",
                                cli_error
                            );
                        } else {
                            return Err(cli_error);
                        }
                    }
                }
            }
            Err(join_error) => {
                if continue_on_error {
                    error_count += 1;
                    eprintln!("⚠️  Warning: Task failed to execute: {}", join_error);
                } else {
                    return Err(CliError::ActionError(
                        crate::actions::CliActionError::IoError(std::io::Error::other(
                            join_error.to_string(),
                        )),
                    ));
                }
            }
        }
    }

    // Report summary with nice statistics
    println!("\n📊 Download Statistics Report");
    println!("===========================");
    println!("✅ Successfully downloaded: {}", success_count);
    if resume_flag {
        // For resume, we need to calculate how many were skipped
        // This requires knowing the total number of assets vs. how many were actually downloaded
        let skipped_count = total_assets - success_count - error_count;
        println!("⏭️  Skipped (already existed): {}", skipped_count);
    } else {
        println!("⏭️  Skipped (already existed): 0");
    }
    if error_count > 0 {
        println!("❌ Failed downloads: {}", error_count);
    } else {
        println!("❌ Failed downloads: 0");
    }
    println!("📁 Total assets processed: {}", total_assets);
    println!("⏳ Operation completed successfully!");
    println!(
        "\n📁 Files downloaded to destination directory: {:?}",
        dest_dir
    );

    Ok(())
}

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
    use crate::{
        commands::params::{PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID},
        configuration::Configuration,
        error::CliError,
        model::normalize_path,
        param_utils::get_tenant,
        physna_v3::{PhysnaApiClient, TryDefault},
    };
    use std::path::Path;
    use uuid::Uuid;

    tracing::trace!("Executing \"folder upload\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get the local directory path from command line
    let local_dir_path = sub_matches
        .get_one::<std::path::PathBuf>("local-path")
        .ok_or_else(|| {
            CliError::MissingRequiredArgument("Local directory path is required".to_string())
        })?;

    // Check if the local path exists and is a directory
    if !local_dir_path.exists() {
        return Err(CliError::ActionError(
            crate::actions::CliActionError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Local path does not exist: {:?}", local_dir_path),
            )),
        ));
    }

    if !local_dir_path.is_dir() {
        return Err(CliError::ActionError(
            crate::actions::CliActionError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Local path is not a directory: {:?}", local_dir_path),
            )),
        ));
    }

    // Get folder UUID or path from command line
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Store the original folder path for asset path construction
    let original_folder_path = if let Some(path) = folder_path_param {
        path.clone()
    } else {
        // If only UUID was provided, we can't determine the path, so we'll use a placeholder
        // In practice, this case should rarely happen since the upload command typically uses paths
        String::from("/")
    };

    // Resolve the folder UUID - first try to get existing folder, then create if needed
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        // Try to resolve the folder UUID by path
        match resolve_folder_uuid_by_path(&mut api, &tenant, path).await {
            Ok(uuid) => uuid,
            Err(CliError::FolderNotFound(_)) => {
                // Folder doesn't exist, create it
                tracing::trace!(
                    "Folder does not exist, creating new folder with path: {}",
                    path
                );

                // Extract folder name from the path
                let folder_name = Path::new(path)
                    .file_name()
                    .ok_or_else(|| {
                        CliError::ActionError(crate::actions::CliActionError::IoError(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Invalid folder path",
                            ),
                        ))
                    })?
                    .to_str()
                    .ok_or_else(|| {
                        CliError::ActionError(crate::actions::CliActionError::IoError(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Invalid folder name encoding",
                            ),
                        ))
                    })?
                    .to_string();

                // Find parent folder UUID if path has multiple segments
                let parent_folder_path = if path.contains("/") {
                    let parent_path = Path::new(path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .ok_or_else(|| {
                            CliError::ActionError(crate::actions::CliActionError::IoError(
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    "Invalid parent folder path",
                                ),
                            ))
                        })?;

                    if !parent_path.is_empty() && normalize_path(parent_path) != "/" {
                        Some(parent_path.to_string())
                    } else {
                        None // Root folder
                    }
                } else {
                    None
                };

                let parent_folder_uuid = if let Some(parent_path) = parent_folder_path {
                    Some(resolve_folder_uuid_by_path(&mut api, &tenant, &parent_path).await?)
                } else {
                    None
                };

                // Create the new folder
                let new_folder_response = api
                    .create_folder(&tenant.uuid, &folder_name, parent_folder_uuid)
                    .await?;
                let new_folder_uuid = new_folder_response.folder.uuid;
                tracing::trace!("Created new folder with UUID: {}", new_folder_uuid);
                new_folder_uuid
            }
            Err(e) => return Err(e),
        }
    } else {
        // Neither folder UUID nor path provided
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Get the command-line parameters
    let skip_existing = sub_matches.get_flag(crate::commands::params::PARAMETER_SKIP_EXISTING);
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);
    let concurrent_param = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_CONCURRENT)
        .copied()
        .unwrap_or(1);
    let continue_on_error =
        sub_matches.get_flag(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR);
    let delay_param = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_DELAY)
        .copied()
        .unwrap_or(0);

    // Validate concurrent parameter
    if !(1..=10).contains(&concurrent_param) {
        return Err(CliError::MissingRequiredArgument(format!(
            "Invalid value for '--concurrent': must be between 1 and 10, got {}",
            concurrent_param
        )));
    }

    // Validate delay parameter
    if delay_param > 180 {
        return Err(CliError::MissingRequiredArgument(format!(
            "Invalid value for '--delay': must be between 0 and 180, got {}",
            delay_param
        )));
    }

    // Read all files in the local directory
    let entries: Vec<_> = std::fs::read_dir(local_dir_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent_param));

    // Create progress bars if requested
    let (progress_bar, multi_progress) = if show_progress {
        let mp = indicatif::MultiProgress::new();
        let pb = mp.add(indicatif::ProgressBar::new(entries.len() as u64));
        pb.set_style(indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Overall progress")
            .unwrap()
            .progress_chars("#>-"));
        (Some(pb), Some(mp))
    } else {
        (None, None)
    };

    // Create a delay duration if delay is specified
    let delay_duration = std::time::Duration::from_secs(delay_param as u64);

    // Upload each file in the directory
    let mut tasks = Vec::new();

    for entry in entries {
        let file_path = entry.path();

        // Skip if it's a directory
        if file_path.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name_str = file_name
            .to_str()
            .ok_or_else(|| {
                CliError::ActionError(crate::actions::CliActionError::IoError(
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid file name encoding",
                    ),
                ))
            })?
            .to_string(); // Clone to move into async closure

        let tenant_clone = tenant.clone();
        let _api_clone = api.clone(); // Clone the API client (currently unused but may be needed for future API calls)
        let semaphore = semaphore.clone();
        let progress_bar_clone = progress_bar.clone();
        let multi_progress_clone = multi_progress.clone();
        let original_folder_path_clone = original_folder_path.clone(); // Clone the original folder path
        let folder_uuid_clone = folder_uuid;
        let skip_existing_clone = skip_existing;
        let delay_duration_clone = delay_duration;
        let delay_param_clone = delay_param;
        let concurrent_param_clone = concurrent_param;

        // Spawn a task for each upload
        let task = tokio::spawn(async move {
            // Acquire a permit from the semaphore to limit concurrency
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this upload if concurrent > 1 and progress is enabled
            let individual_pb = if concurrent_param_clone > 1 && progress_bar_clone.is_some() {
                if let Some(ref mp) = multi_progress_clone {
                    let individual_pb = mp.add(indicatif::ProgressBar::new_spinner());
                    individual_pb.set_style(
                        indicatif::ProgressStyle::default_bar()
                            .template("{spinner:.yellow} {msg}")
                            .unwrap(),
                    );
                    individual_pb.set_message(format!("Starting upload: {}", file_name_str));
                    Some(individual_pb)
                } else {
                    None
                }
            } else {
                None
            };

            // Add delay if specified
            if delay_param_clone > 0 {
                tokio::time::sleep(delay_duration_clone).await;
            }

            // Create a new API client for this task
            let mut api_task = match crate::physna_v3::PhysnaApiClient::try_default() {
                Ok(client) => client,
                Err(e) => {
                    return Err(CliError::PhysnaExtendedApiError(
                        e,
                    ));
                }
            };

            // Check if an asset with the same name already exists in the folder
            let assets_response = api_task
                .list_assets_by_parent_folder_uuid(&tenant_clone.uuid, Some(&folder_uuid_clone))
                .await;
            let asset_exists = match assets_response {
                Ok(response) => {
                    let asset_list: crate::model::AssetList = response;
                    asset_list
                        .get_all_assets()
                        .iter()
                        .any(|asset| asset.name() == file_name_str)
                }
                Err(_) => false, // If we can't check, assume it doesn't exist to allow upload
            };

            if asset_exists {
                if skip_existing_clone {
                    println!("Skipping existing asset: {}", file_name_str);
                    // Update overall progress bar if present
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }
                    return Ok(Ok(file_name_str));
                } else {
                    return Err(CliError::ActionError(crate::actions::CliActionError::BusinessLogicError(
                        format!("Asset already exists: {}. Use --skip-existing to skip existing assets.", file_name_str)
                    )));
                }
            }

            // Upload the file
            tracing::trace!(
                "Uploading asset: {} to folder UUID: {}",
                file_name_str,
                folder_uuid_clone
            );

            // Read the file content
            let file_content = std::fs::read(&file_path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            // Create a temporary file to pass to the API
            let temp_file = std::env::temp_dir().join(&file_name_str);
            std::fs::write(&temp_file, &file_content)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            // Construct the asset path using the original folder path and file name
            // Use the original folder path provided to the command to ensure correct asset path
            let asset_path = if original_folder_path_clone.ends_with('/') {
                format!("{}{}", original_folder_path_clone, file_name_str)
            } else {
                format!("{}/{}", original_folder_path_clone, file_name_str)
            };

            // Upload the asset to the specified folder
            let upload_result = api_task
                .create_asset(
                    &tenant_clone.uuid,
                    &temp_file,
                    &asset_path,
                    &folder_uuid_clone,
                )
                .await;

            // Clean up the temporary file
            let _ = std::fs::remove_file(&temp_file);

            match upload_result {
                Ok(_) => {
                    // Update individual progress bar
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Uploaded: {}", file_name_str));
                        ipb.finish_and_clear(); // Clear the spinner for this individual upload
                    }

                    // Update overall progress bar if present
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }

                    Ok(Ok(file_name_str))
                }
                Err(e) => {
                    // Update individual progress bar for error
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Failed: {} - {}", file_name_str, e));
                        ipb.finish_and_clear(); // Clear the spinner for this individual upload
                    }

                    // Log the detailed error for debugging
                    tracing::error!(
                        "Failed to upload asset '{}' (Asset path: {}): {}",
                        file_name_str,
                        asset_path,
                        e
                    );
                    tracing::debug!(
                        "Error details for asset '{}': error type = {:?}",
                        file_name_str,
                        e
                    );

                    Err(CliError::PhysnaExtendedApiError(e))
                }
            }
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors_occurred = false;

    for task in tasks {
        match task.await {
            Ok(task_result) => {
                match task_result {
                    Ok(asset_result) => {
                        match asset_result {
                            Ok(asset_name) => {
                                success_count += 1;
                                // Only print individual success messages if progress is not shown
                                // Otherwise, the progress bar already shows the status
                                if !show_progress {
                                    println!("Successfully uploaded: {}", asset_name);
                                }
                            }
                            Err(cli_error) => {
                                error_count += 1;
                                errors_occurred = true;
                                // If continue_on_error is true, we continue processing other assets
                                if !continue_on_error {
                                    return Err(cli_error);
                                }
                                // Log the error but continue processing
                                eprintln!("Error uploading asset: {}", cli_error);
                            }
                        }
                    }
                    Err(cli_error) => {
                        error_count += 1;
                        errors_occurred = true;
                        // If continue_on_error is true, we continue processing other assets
                        if !continue_on_error {
                            return Err(cli_error);
                        }
                        // Log the error but continue processing
                        eprintln!("Error in task: {}", cli_error);
                    }
                }
            }
            Err(join_error) => {
                error_count += 1;
                errors_occurred = true;
                // If continue_on_error is true, we continue processing other assets
                if !continue_on_error {
                    return Err(CliError::ActionError(
                        crate::actions::CliActionError::IoError(std::io::Error::other(
                            join_error.to_string(),
                        )),
                    ));
                }
                // Log the error but continue processing
                eprintln!("Join error: {}", join_error);
            }
        }
    }

    // Print summary if there were errors or if no progress bar was shown
    if errors_occurred {
        eprintln!(
            "❌ Uploaded {} assets successfully, {} assets failed",
            success_count, error_count
        );
    } else if !show_progress {
        println!("✅ Successfully uploaded {} assets", success_count);
    }

    // Finish progress bar if present
    if let Some(pb) = progress_bar {
        pb.finish_with_message("All assets uploaded!");
    }

    Ok(())
}
fn extract_zip_and_cleanup(zip_path: &std::path::Path) -> Result<(), std::io::Error> {
    use std::io::Cursor;

    // Read the ZIP file content
    let zip_content = std::fs::read(zip_path)?;

    // Create a cursor from the content
    let cursor = Cursor::new(zip_content);

    // Create a ZipArchive from the cursor
    let mut archive = zip::ZipArchive::new(cursor)?;

    // Extract all files to the same directory as the ZIP file
    let parent_dir = zip_path.parent().ok_or_else(|| {
        std::io::Error::other("Could not get parent directory")
    })?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        let file_path = parent_dir.join(file.mangled_name());

        if file.is_dir() {
            std::fs::create_dir_all(&file_path)?;
        } else {
            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut output_file = std::fs::File::create(&file_path)?;
            std::io::copy(&mut file, &mut output_file)?;
        }
    }

    // Remove the original ZIP file after successful extraction
    std::fs::remove_file(zip_path)?;

    Ok(())
}
async fn download_asset_with_retry(
    api: &mut crate::physna_v3::PhysnaApiClient,
    tenant_id: &str,
    asset_id: &str,
    asset_name: &str,
) -> Result<Vec<u8>, crate::physna_v3::ApiError> {
    use rand::Rng;

    // First attempt
    match api
        .download_asset(tenant_id, asset_id, Some(asset_name))
        .await
    {
        Ok(content) => Ok(content),
        Err(e) => {
            // If the first attempt fails, wait for a random delay between 5-10 seconds and retry once
            // Don't log the first error to avoid confusing users if the retry succeeds
            tracing::debug!(
                "Asset download failed for '{}' (ID: {}) (attempt 1), retrying after delay: {}",
                asset_name,
                asset_id,
                e
            );

            // Generate random delay between 5 and 10 seconds
            // Use thread_rng in a blocking way to avoid Send issues
            let delay_seconds = tokio::task::spawn_blocking(|| {
                let mut rng = rand::thread_rng();
                rng.gen_range(5..=10)
            })
            .await
            .unwrap_or(5); // Default to 5 seconds if spawning fails

            tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

            // Second and final attempt
            match api
                .download_asset(tenant_id, asset_id, Some(asset_name))
                .await
            {
                Ok(content) => Ok(content),
                Err(final_e) => {
                    tracing::error!(
                        "Asset download failed for '{}' (ID: {}) after retry: {}",
                        asset_name,
                        asset_id,
                        final_e
                    );
                    Err(final_e)
                }
            }
        }
    }
}

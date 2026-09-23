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
    format::{OutputFormat, OutputFormatter},
    model::{normalize_path, Folder, Tenant},
    param_utils::{get_format_parameter_value, get_tenant},
    path_utils::find_similar_paths,
    physna_v3::{PhysnaApiClient, TryDefault},
};

/// A folder a command works on: the tenant's root, which has no folder record and
/// no UUID, or an ordinary folder.
///
/// `resolve_folder_uuid_by_path` cannot answer for the root, and several commands
/// that called it had a "root" branch that could therefore never run: `folder
/// download --folder-path /` failed with "folder not found". Resolving to this type
/// makes every caller decide what the root means for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderTarget {
    Root,
    Folder(Uuid),
}

impl FolderTarget {
    /// The folder's UUID, `None` for the root (which is how the listing endpoints
    /// spell "no parent folder").
    pub fn uuid(&self) -> Option<Uuid> {
        match self {
            FolderTarget::Root => None,
            FolderTarget::Folder(uuid) => Some(*uuid),
        }
    }
}

/// Resolve `--folder-uuid` / `--folder-path` to a [`FolderTarget`]. `/` and `/Home`
/// are the root.
pub async fn resolve_folder_target(
    api: &mut PhysnaApiClient,
    tenant: &Tenant,
    folder_uuid: Option<&Uuid>,
    folder_path: Option<&String>,
) -> Result<FolderTarget, CliError> {
    match (folder_uuid, folder_path) {
        (Some(uuid), _) => Ok(FolderTarget::Folder(*uuid)),
        (None, Some(path)) if normalize_path(path) == "/" => Ok(FolderTarget::Root),
        (None, Some(path)) => Ok(FolderTarget::Folder(
            resolve_folder_uuid_by_path(api, tenant, path).await?,
        )),
        (None, None) => Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        )),
    }
}

/// "Folder not found", with the closest existing paths as suggestions.
pub fn folder_not_found(
    hierarchy: &crate::folder_hierarchy::FolderHierarchy,
    path: &str,
) -> CliError {
    let suggestions = find_similar_paths(hierarchy, path);
    let suggestion_message = match suggestions.as_slice() {
        [] => String::new(),
        [only] => format!("\n\nDid you mean: {}", only),
        many => format!(
            "\n\nDid you mean one of:\n  {}",
            many.iter()
                .map(|s| format!("• {}", s))
                .collect::<Vec<_>>()
                .join("\n  ")
        ),
    };
    CliError::FolderNotFound(path.to_string(), suggestion_message)
}

pub async fn resolve_folder_uuid_by_path(
    api: &mut PhysnaApiClient,
    tenant: &Tenant,
    path: &str,
) -> Result<Uuid, CliError> {
    trace!("Resolving the UUID for folder path {}...", path);

    // Root path should be handled separately by the calling function, so this function is only for non-root paths
    match api.get_folder_uuid_by_path(&tenant.uuid, path).await {
        Ok(Some(folder_uuid)) => Ok(folder_uuid),
        Ok(None) => {
            // Folder not found - try to provide helpful suggestions. The lookup that
            // just missed refreshed the cache, so this is the current hierarchy.
            let hierarchy =
                crate::folder_cache::FolderCache::get_or_fetch(api, &tenant.uuid).await?;
            Err(folder_not_found(&hierarchy, path))
        }
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
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID).copied();
    let path_param = sub_matches
        .get_one::<String>(PARAMETER_FOLDER_PATH)
        .map(|path| normalize_path(path.clone()));

    // The listing comes from the shared folder cache, which every other command
    // resolves paths against; --reload refreshes it. (It used to bypass the cache
    // both ways: a full fetch on every call that was then thrown away.)
    let reload_cache = sub_matches.get_flag(crate::commands::params::PARAMETER_RELOAD);
    let hierarchy = if reload_cache {
        trace!("Reload flag set, refreshing folder cache...");
        crate::folder_cache::FolderCache::refresh(&mut api, &tenant.uuid).await?
    } else {
        crate::folder_cache::FolderCache::get_or_fetch(&mut api, &tenant.uuid).await?
    };

    // The folder can be given by path or by UUID; the listing code works on paths, so a
    // UUID is resolved against the hierarchy just fetched. (The UUID used to be accepted
    // by the parser and never read, which listed the root instead.)
    let path = match (path_param, folder_uuid_param) {
        (Some(path), _) => path,
        (None, Some(folder_uuid)) => hierarchy
            .get_path_for_folder(&folder_uuid)
            .map(|p| normalize_path(format!("/{}", p)))
            .ok_or_else(|| CliError::FolderNotFound(folder_uuid.to_string(), String::new()))?,
        (None, None) => "/".to_string(),
    };
    trace!("Path requested: \"{}\"", &path);

    // If tree format is requested, display the hierarchical tree structure
    match format {
        OutputFormat::Tree(_) => {
            let hierarchy = if path.eq("/") {
                hierarchy
            } else {
                hierarchy
                    .filter_by_path(path.as_str())
                    .ok_or(CliError::FolderNotFound(path.clone(), String::new()))?
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
                    .ok_or(CliError::FolderNotFound(path.clone(), String::new()))?
            };

            crate::format::print_output(&folder_list.format(format)?);
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

    crate::format::print_output(&folder.format(format)?);

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

    // Checked before anything is resolved: the root cannot be resolved to a
    // folder, so checked afterwards this message was never reached.
    // Check if trying to rename the root folder
    if folder_path_param.is_some_and(|p| crate::model::normalize_path(p) == "/") {
        return Err(CliError::MissingRequiredArgument(
            "Cannot rename the root folder".to_string(),
        ));
    }

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

    // The cached folder hierarchy still holds the old name; drop it so the
    // next path resolution rebuilds from the API.
    crate::folder_cache::FolderCache::invalidate(&tenant_uuid.to_string()).unwrap_or_else(|e| {
        tracing::debug!("Failed to invalidate folder cache: {}", e);
    });

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
        return Err(CliError::MissingRequiredArgument(
            "Missing folder identifier".to_string(),
        ));
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

    // The cached folder hierarchy still shows the old location; drop it so
    // the next path resolution rebuilds from the API.
    crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()).unwrap_or_else(|e| {
        tracing::debug!("Failed to invalidate folder cache: {}", e);
    });

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

    let created = api
        .create_folder(&tenant.uuid, name.as_str(), parent_folder_uuid)
        .await?;
    // The UUID is what a script needs next; it used to be discarded.
    println!("{}", created.folder.uuid);

    // Drop the cached folder hierarchy so the new folder resolves without
    // relying on the cache-miss refresh heuristic.
    crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()).unwrap_or_else(|e| {
        tracing::debug!("Failed to invalidate folder cache: {}", e);
    });

    Ok(())
}

pub async fn delete_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let folder_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);
    let force_flag = sub_matches.get_flag("force");
    let yes_flag = sub_matches.get_flag("yes");

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
        return Err(CliError::MissingRequiredArgument(
            "Missing folder path".to_string(),
        ));
    };

    // Report and stop without deleting anything when --dry-run is given
    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would delete folder '{}'{}",
            folder_path_param.unwrap_or(&folder_uuid.to_string()),
            if force_flag {
                " and ALL its contents"
            } else {
                ""
            }
        );
        return Ok(());
    }

    // Ask for confirmation unless --yes flag is provided
    if !yes_flag {
        let delete_msg = if force_flag {
            format!(
                "Delete folder '{}' and ALL its contents?",
                folder_path_param.unwrap_or(&folder_uuid.to_string())
            )
        } else {
            format!(
                "Delete folder '{}'?",
                folder_path_param.unwrap_or(&folder_uuid.to_string())
            )
        };

        if !crate::terminal::confirm(&delete_msg, Some("This action cannot be undone"))? {
            eprintln!("Deletion cancelled.");
            return Ok(());
        }
    }

    match api
        .delete_folder(&tenant.uuid, &folder_uuid, force_flag)
        .await
    {
        Ok(()) => {
            // The cached folder hierarchy still contains the deleted folder;
            // drop it so subsequent path resolutions don't target it.
            crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()).unwrap_or_else(
                |e| {
                    tracing::debug!("Failed to invalidate folder cache: {}", e);
                },
            );
            Ok(())
        }
        Err(api_error) => {
            // Check if this is a 404 error on a folder deletion, which likely means the folder is not empty
            if matches!(api_error, ApiError::NotFoundError(_)) && !force_flag {
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

    // Check if reload flag is set to clear the cache
    let reload_cache = sub_matches.get_flag(crate::commands::params::PARAMETER_RELOAD);
    if reload_cache {
        trace!("Reload flag set, clearing folder cache before resolving...");
        crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()).unwrap_or_else(
            |e| {
                tracing::debug!("Failed to invalidate folder cache: {}", e);
            },
        );
    }

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
        None => Err(CliError::FolderNotFound(folder_path.clone(), String::new())),
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

    // Invalidate folder cache to ensure we get fresh data from the server
    crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()).unwrap_or_else(|e| {
        tracing::debug!("Failed to invalidate folder cache: {}", e);
    });

    // Get folder UUID or path from command line
    let folder_uuid_param =
        sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_FOLDER_UUID);
    let folder_path_param =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FOLDER_PATH);

    // `/` (or `/Home`) is the tenant's root: every asset in the tenant.
    let target =
        resolve_folder_target(&mut api, &tenant, folder_uuid_param, folder_path_param).await?;

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_OUTPUT)
    {
        output_path.clone()
    } else {
        // Use the folder name as the default output file name
        // Determine the folder name from the provided path or get it from the folder details
        let folder_name = match (target, folder_path_param) {
            // The root has no folder record; it is named after the tenant.
            (FolderTarget::Root, _) => tenant.name.clone(),
            (FolderTarget::Folder(_), Some(path)) => path
                .split('/')
                .rfind(|s| !s.is_empty())
                .unwrap_or("untitled")
                .to_string(),
            (FolderTarget::Folder(folder_uuid), None) => {
                // The folder record carries its name; its `path` field is always
                // empty here, which is why this used to produce a directory called
                // "untitled".
                let name = api.get_folder(&tenant.uuid, &folder_uuid).await?.name();
                if name.trim().is_empty() {
                    tenant.name.clone()
                } else {
                    name
                }
            }
        };

        // The folder's name comes from the server; it must be one plain name before
        // it becomes the default download directory.
        crate::actions::utils::safe_file_name(&folder_name).ok_or_else(|| {
            CliError::from(crate::actions::CliActionError::BusinessLogicError(format!(
                "The folder's name '{}' is not a safe local directory name; choose one with -o/--output",
                folder_name
            )))
        })?
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
    // Assets left out because they have no downloadable result yet (still
    // indexing) or processing failed: (Physna path, state). They used to be dropped
    // without a word, missing from the totals, and the run still exited 0.
    let mut not_downloadable: Vec<(String, String)> = Vec::new();
    let mut folder_queue: std::collections::VecDeque<(Option<Uuid>, String)> =
        std::collections::VecDeque::new();

    let root_folder_path = match target {
        FolderTarget::Root => "/".to_string(),
        FolderTarget::Folder(folder_uuid) => {
            api.get_folder(&tenant.uuid, &folder_uuid).await?.path()
        }
    };
    folder_queue.push_back((target.uuid(), root_folder_path.clone()));

    while let Some((current_folder_uuid, current_folder_path)) = folder_queue.pop_front() {
        let asset_list = api
            .list_assets_by_parent_folder_uuid(&tenant.uuid, current_folder_uuid.as_ref())
            .await?;

        for asset in asset_list.get_all_assets() {
            // `finished` assets, and assemblies waiting for a missing part
            // (`missing-dependencies`): both have a file to download, and
            // `download_assembly` copes with an assembly that has no bundle yet.
            let state = asset
                .processing_status()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            if state != "finished" && state != "missing-dependencies" {
                not_downloadable.push((asset.path().clone(), state));
                continue;
            }

            // Files keep the asset's own name; an assembly's bundle is unpacked
            // next to it (see `download_assembly`).
            let relative_folder_path = current_folder_path
                .strip_prefix(root_folder_path.as_str())
                .unwrap_or(&current_folder_path)
                .trim_matches('/');
            let relative_path = if relative_folder_path.is_empty() {
                asset.name().to_string()
            } else {
                format!("{}/{}", relative_folder_path, asset.name())
            };

            all_assets_with_paths.push((asset.clone(), relative_path, asset.path().clone()));
        }

        // Get subfolders of current folder to process next. Walks every page
        // so folders with more direct subfolders than one page still have
        // their full subtree processed.
        let subfolders_response = api
            .list_all_subfolders(&tenant.uuid, current_folder_uuid.as_ref())
            .await?;
        for folder in subfolders_response.folders() {
            // The listing already carries the name; fetching each folder again cost
            // one request per subfolder before the first byte was downloaded.
            let folder_path = if current_folder_path.ends_with('/') {
                format!("{}{}", current_folder_path, folder.name())
            } else {
                format!("{}/{}", current_folder_path, folder.name())
            };
            folder_queue.push_back((Some(*folder.uuid()), folder_path));
        }
    }

    if all_assets_with_paths.is_empty() {
        crate::error_utils::report_warning(&format!(
            "No downloadable assets found in folder {} or its subfolders ({} not processed yet or failed processing); nothing to download",
            root_folder_path,
            not_downloadable.len()
        ));
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
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
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
    let mut first_error: Option<CliError> = None; // Track the first error if not continuing
    let mut error_messages: Vec<String> = Vec::new(); // Collect error messages to print later

    // Download each asset to the appropriate subdirectory in the temp directory
    let mut tasks = Vec::new();

    for (asset, relative_path, physna_path) in all_assets_with_paths {
        let tenant_id = tenant.uuid.to_string();
        let asset_id = asset.uuid().to_string();
        let asset_name = asset.name().to_string();
        let asset_file_path = match crate::actions::utils::safe_relative_path(&relative_path) {
            Some(safe) => dest_dir.join(safe),
            None => {
                return Err(CliError::ActionError(
                    crate::actions::CliActionError::BusinessLogicError(format!(
                        "refusing to write '{}': the server-provided name would escape the output directory",
                        relative_path
                    )),
                ))
            }
        };
        let is_assembly = asset.is_assembly();
        let mut api_task = api.clone();
        let semaphore = semaphore.clone();
        let progress_bar_clone = progress_bar.clone();
        let multi_progress_clone = multi_progress.clone();
        let delay_duration = Duration::from_secs(delay_param as u64);
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
                            .template("{spinner:.yellow} [{elapsed_precise}] {msg}")
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

            // With --resume a file already on disk is not downloaded again. For an
            // assembly that is the assembly file itself, which `download_assembly`
            // writes only after all of its parts.
            if resume_flag && asset_file_path.exists() {
                tracing::debug!("Skipping existing file: {}", asset_file_path.display());
                if let Some(ref pb) = progress_bar_clone {
                    pb.inc(1);
                }
                return Ok((asset_name, true));
            }

            // Add delay if specified (only when actually downloading, not when skipping)
            if delay_param > 0 {
                sleep(delay_duration).await;
            }

            // Streamed to disk through a temporary file (see download_asset_to_file);
            // the client handles renewal and transient retries itself. Assemblies go
            // through the same path as `asset download`, which unpacks a dependency
            // bundle and keeps a raw assembly file as it is.
            let downloaded: Result<(), CliError> = if is_assembly {
                crate::actions::assets::download::download_assembly(
                    &mut api_task,
                    &tenant_id,
                    &asset_id,
                    &asset_name,
                    &asset_file_path,
                )
                .await
                .map(|_| ())
            } else {
                api_task
                    .download_asset_to_file(
                        &tenant_id,
                        &asset_id,
                        Some(&asset_name),
                        &asset_file_path,
                    )
                    .await
                    .map(|_| ())
                    .map_err(CliError::PhysnaExtendedApiError)
            };

            if let Some(ref ipb) = individual_pb {
                ipb.finish_and_clear();
            }
            match downloaded {
                Ok(()) => {
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }
                    Ok((asset_name, false))
                }
                // Reported after the progress bars are cleared, not here.
                Err(e) => Err((asset_name, physna_path, e)),
            }
        });

        tasks.push(task);
    }

    // Track how many assets were skipped because they already existed
    let mut skipped_count = 0;
    let mut not_attempted = 0;

    // Without --continue-on-error the first failure stops the run: every task
    // still queued is aborted rather than left to fail (or succeed) on its own.
    // Aborting a task that has already finished is a no-op.
    let abort_handles: Vec<_> = tasks.iter().map(|t| t.abort_handle()).collect();
    let stop_remaining = || {
        for handle in &abort_handles {
            handle.abort();
        }
    };

    for task in tasks {
        match task.await {
            Ok(Ok((_asset_name, was_skipped))) => {
                if was_skipped {
                    skipped_count += 1;
                } else {
                    success_count += 1;
                }
            }
            Ok(Err((asset_name, physna_path, error))) => {
                error_count += 1;
                // Collected and printed after the progress bars are cleared.
                error_messages.push(format!(
                    "⚠️  Failed to download asset '{}' (Physna path: {}): {}",
                    asset_name,
                    physna_path,
                    download_error_text(&error)
                ));
                if !continue_on_error && first_error.is_none() {
                    first_error = Some(error);
                    stop_remaining();
                }
            }
            Err(join_error) if join_error.is_cancelled() => {
                not_attempted += 1;
            }
            Err(join_error) => {
                error_count += 1;
                error_messages.push(format!("⚠️  Task failed to execute: {}", join_error));
                if !continue_on_error && first_error.is_none() {
                    first_error = Some(CliError::ActionError(
                        crate::actions::CliActionError::IoError(std::io::Error::other(
                            join_error.to_string(),
                        )),
                    ));
                    stop_remaining();
                }
            }
        }
    }

    // Finish progress bars before printing summary and errors
    if let Some(pb) = progress_bar {
        pb.finish_and_clear();
    }
    if let Some(mp) = multi_progress {
        mp.clear().ok();
    }

    // Report summary with nice statistics - print this FIRST so errors appear above it
    print_download_summary(
        success_count,
        skipped_count,
        error_count,
        not_attempted,
        not_downloadable.len(),
        total_assets,
        &dest_dir,
    );

    // Print collected error messages AFTER the stats so they remain visible on screen
    if !error_messages.is_empty() {
        eprintln!();
        eprintln!("📋 Detailed Error List:");
        eprintln!("======================");
        for error_msg in &error_messages {
            eprintln!("{}", error_msg);
        }
    }
    if !not_downloadable.is_empty() {
        eprintln!();
        eprintln!("⏭️  Not downloaded (not processed yet, or processing failed):");
        for (path, state) in &not_downloadable {
            eprintln!("   {} ({})", path, state);
        }
        eprintln!("   'pcli2 asset diagnose --path <path>' explains a failed asset.");
    }

    // A run with failures exits non-zero whether or not it was allowed to continue;
    // --continue-on-error only decides whether the remaining assets were attempted.
    if let Some(error) = first_error {
        if !continue_on_error {
            return Err(error);
        }
    }
    if error_count > 0 {
        return Err(CliError::ActionError(
            crate::actions::CliActionError::PartialFailure {
                failed: error_count,
                total: total_assets,
                what: "download(s)".to_string(),
            },
        ));
    }

    Ok(())
}

/// The text of a failed download for the error list: the API's own message for an
/// API error, as before, rather than the "API error: ..." wrapper.
fn download_error_text(error: &CliError) -> String {
    match error {
        CliError::PhysnaExtendedApiError(e) => e.to_string(),
        other => other.to_string(),
    }
}

/// Print download statistics summary
fn print_download_summary(
    success_count: usize,
    skipped_count: usize,
    error_count: usize,
    not_attempted: usize,
    not_downloadable: usize,
    total_assets: usize,
    dest_dir: &std::path::PathBuf,
) {
    // Status goes to stderr: stdout is for data.
    eprintln!("\n📊 Download Statistics Report");
    eprintln!("===========================");
    eprintln!("✅ Successfully downloaded: {}", success_count);
    eprintln!("⏭️  Skipped (already existed): {}", skipped_count);
    eprintln!("❌ Failed downloads: {}", error_count);
    if not_attempted > 0 {
        eprintln!(
            "⏹️  Not attempted (stopped after the first failure): {}",
            not_attempted
        );
    }
    if not_downloadable > 0 {
        eprintln!(
            "⏭️  Not downloaded (not processed yet, or processing failed): {}",
            not_downloadable
        );
    }
    eprintln!("📁 Total assets processed: {}", total_assets);
    if error_count > 0 || not_attempted > 0 {
        eprintln!("⏳ Operation completed with errors!");
    } else {
        eprintln!("⏳ Operation completed successfully!");
    }
    eprintln!(
        "\n📁 Files downloaded to destination directory: {:?}",
        dest_dir
    );
}

/// Download thumbnails for all assets in a folder.
///
/// This function handles the "folder thumbnail" command, downloading thumbnails for all assets
/// in a specified folder and its subfolders.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the thumbnails were downloaded successfully
/// * `Err(CliError)` - If an error occurred during thumbnail download
pub async fn download_folder_thumbnails(sub_matches: &clap::ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"folder thumbnail\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder UUID or path from command line
    let folder_uuid_param =
        sub_matches.get_one::<Uuid>(crate::commands::params::PARAMETER_FOLDER_UUID);
    let folder_path_param =
        sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FOLDER_PATH);

    // `/` (or `/Home`) is the tenant's root: every asset in the tenant.
    let target =
        resolve_folder_target(&mut api, &tenant, folder_uuid_param, folder_path_param).await?;

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(crate::commands::params::PARAMETER_OUTPUT)
    {
        output_path.clone()
    } else {
        // Use the folder name as the default output file name
        // Determine the folder name from the provided path or get it from the folder details
        let folder_name = match (target, folder_path_param) {
            // The root has no folder record; it is named after the tenant.
            (FolderTarget::Root, _) => tenant.name.clone(),
            (FolderTarget::Folder(_), Some(path)) => path
                .split('/')
                .rfind(|s| !s.is_empty())
                .unwrap_or("untitled")
                .to_string(),
            (FolderTarget::Folder(folder_uuid), None) => {
                // The folder record carries its name; its `path` field is always
                // empty here, which is why this used to produce a directory called
                // "untitled".
                let name = api.get_folder(&tenant.uuid, &folder_uuid).await?.name();
                if name.trim().is_empty() {
                    tenant.name.clone()
                } else {
                    name
                }
            }
        };

        // The folder's name comes from the server; it must be one plain name before
        // it becomes the default output directory.
        crate::actions::utils::safe_file_name(&folder_name).ok_or_else(|| {
            CliError::from(crate::actions::CliActionError::BusinessLogicError(format!(
                "The folder's name '{}' is not a safe local directory name; choose one with -o/--output",
                folder_name
            )))
        })?
    };

    // Use the destination directory directly
    let dest_dir = output_file_path.clone();
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Use BFS to collect all folders in the hierarchy and their assets
    let mut all_assets_with_paths = Vec::new();
    let mut folder_queue = std::collections::VecDeque::new();

    let root_folder_path = match target {
        FolderTarget::Root => "/".to_string(),
        FolderTarget::Folder(folder_uuid) => {
            api.get_folder(&tenant.uuid, &folder_uuid).await?.path()
        }
    };

    // Start BFS with the specified folder
    folder_queue.push_back((target.uuid(), root_folder_path.clone()));

    while let Some((current_folder_uuid, current_folder_path)) = folder_queue.pop_front() {
        // Get all assets in the current folder
        let assets_response = api
            .list_assets_by_parent_folder_uuid(&tenant.uuid, current_folder_uuid.as_ref())
            .await?;
        let asset_list = assets_response;

        // Add assets with their relative paths
        for asset in asset_list.get_all_assets() {
            // Calculate the relative path from the root folder
            let asset_name = asset.name().to_string();
            let asset_name_no_ext = std::path::Path::new(&asset_name)
                .file_stem()
                .unwrap_or(std::ffi::OsStr::new(&asset_name))
                .to_string_lossy()
                .to_string();

            let relative_path = if current_folder_path == root_folder_path {
                // If it's the root folder, just use the asset name with .png extension
                format!("{}.png", asset_name_no_ext)
            } else {
                // Otherwise, create a subfolder path by removing the root folder path prefix
                let relative_folder_path = current_folder_path
                    .strip_prefix(&root_folder_path)
                    .unwrap_or(&current_folder_path) // fallback if strip_prefix fails
                    .trim_start_matches('/') // remove leading slash
                    .trim_end_matches('/'); // remove trailing slash

                if relative_folder_path.is_empty() {
                    format!("{}.png", asset_name_no_ext)
                } else {
                    format!("{}/{}.png", relative_folder_path, asset_name_no_ext)
                }
            };

            // Use the asset's original path as the physna_path
            let physna_path = asset.path().clone();

            all_assets_with_paths.push((asset.clone(), relative_path, physna_path));
        }

        // Get subfolders of current folder to process next. Walks every page
        // so folders with more direct subfolders than one page still have
        // their full subtree processed.
        let subfolders_response = api
            .list_all_subfolders(&tenant.uuid, current_folder_uuid.as_ref())
            .await?;
        for folder in subfolders_response.folders() {
            // The listing already carries the name (see download_folder).
            let folder_path = if current_folder_path.ends_with('/') {
                format!("{}{}", current_folder_path, folder.name())
            } else {
                format!("{}/{}", current_folder_path, folder.name())
            };

            // Add to queue to process this subfolder
            folder_queue.push_back((Some(*folder.uuid()), folder_path));
        }
    }

    if all_assets_with_paths.is_empty() {
        // An empty folder is not a failure (the command exits 0), so it is a
        // warning, as for `folder download`, not an error message.
        crate::error_utils::report_warning(&format!(
            "No assets found in folder {} or its subfolders; nothing to download",
            root_folder_path
        ));
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
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
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

    // Download each asset's thumbnail to the appropriate subdirectory in the destination directory
    let mut tasks = Vec::new();

    for (asset, relative_path, physna_path) in all_assets_with_paths {
        let tenant_id = tenant.uuid.to_string();
        let asset_id = asset.uuid().to_string();
        let asset_name = asset.name().to_string();
        let asset_thumbnail_path = match crate::actions::utils::safe_relative_path(&relative_path)
        {
            Some(safe) => dest_dir.join(safe),
            None => {
                return Err(CliError::ActionError(
                    crate::actions::CliActionError::BusinessLogicError(format!(
                        "refusing to write '{}': the server-provided name would escape the output directory",
                        relative_path
                    )),
                ))
            }
        };
        let mut api_task = api.clone();
        let semaphore = semaphore.clone();
        let progress_bar_clone = progress_bar.clone();
        let multi_progress_clone = multi_progress.clone();
        let delay_duration = Duration::from_secs(delay_param as u64);
        let continue_on_error_clone = continue_on_error;
        let concurrent_param_clone = concurrent_param;

        // Spawn a task for each thumbnail download
        let task = tokio::spawn(async move {
            // Acquire a permit from the semaphore to limit concurrency
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this download if concurrent > 1 and progress is enabled
            let individual_pb = if concurrent_param_clone > 1 && progress_bar_clone.is_some() {
                if let Some(ref mp) = multi_progress_clone {
                    let individual_pb = mp.add(ProgressBar::new_spinner()); // We'll update this later with actual size if known
                    individual_pb.set_style(
                        ProgressStyle::default_bar()
                            .template("{spinner:.yellow} [{elapsed_precise}] {msg}")
                            .unwrap(),
                    );
                    individual_pb.set_message(format!("Downloading thumbnail: {}", asset_name));
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

            // Download the asset thumbnail with retry logic
            let thumbnail_content = download_asset_thumbnail_with_retry(
                &mut api_task,
                &tenant_id,
                &asset_id,
                &asset_name,
            )
            .await;

            match thumbnail_content {
                Ok(thumbnail_content) => {
                    // Update individual progress bar
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Downloaded thumbnail: {}", asset_name));
                        ipb.finish_and_clear(); // Clear the spinner for this individual download
                    }

                    // Create parent directories if they don't exist
                    if let Some(parent) = asset_thumbnail_path.parent() {
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

                    let file_result = File::create(&asset_thumbnail_path);
                    match file_result {
                        Ok(mut file) => match file.write_all(&thumbnail_content) {
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

                    // Update overall progress bar if present
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }

                    Ok(Ok(ThumbnailOutcome::Downloaded))
                }
                Err(ApiError::NotFoundError(msg)) if msg.contains("Asset thumbnail not found") => {
                    // Update individual progress bar for skipped asset
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Skipped thumbnail (not found): {}", asset_name));
                        ipb.finish_and_clear(); // Clear the spinner for this individual download
                    }

                    // Log that the thumbnail was not found but continue processing
                    tracing::debug!(
                        "Thumbnail not found for asset '{}' (Asset UUID: {}, Physna path: {}): {}",
                        asset_name,
                        asset_id,
                        physna_path,
                        msg
                    );

                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }

                    // Not a success: nothing was written. Reported separately so the
                    // summary does not claim a thumbnail that does not exist.
                    Ok(Ok(ThumbnailOutcome::NoThumbnail))
                }
                Err(e) => {
                    // Update individual progress bar for error
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Failed thumbnail: {} - {}", asset_name, e));
                        ipb.finish_and_clear(); // Clear the spinner for this individual download
                    }

                    // Log the detailed error for debugging with asset UUID and Physna path
                    tracing::error!(
                        "Failed to download thumbnail for asset '{}' (Asset UUID: {}, Physna path: {}): {}",
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

    let mut missing_count = 0;
    // Wait for all tasks to complete
    for task in tasks {
        match task.await {
            Ok(task_result) => match task_result {
                Ok(asset_result) => match asset_result {
                    Ok(ThumbnailOutcome::Downloaded) => {
                        success_count += 1;
                    }
                    Ok(ThumbnailOutcome::NoThumbnail) => {
                        missing_count += 1;
                    }
                    Err((asset_name, physna_path, error, is_recoverable)) => {
                        if is_recoverable {
                            error_count += 1;
                            crate::error_utils::report_warning(&format!(
                                "Failed to download thumbnail for asset '{}' (Physna path: {}): {}",
                                asset_name, physna_path, error
                            ));
                        } else {
                            return Err(CliError::PhysnaExtendedApiError(error));
                        }
                    }
                },
                Err(cli_error) => {
                    if continue_on_error {
                        error_count += 1;
                        crate::error_utils::report_warning(&format!(
                            "Failed to download thumbnail due to CLI error: {}",
                            cli_error
                        ));
                    } else {
                        return Err(cli_error);
                    }
                }
            },
            Err(join_error) => {
                if continue_on_error {
                    error_count += 1;
                    crate::error_utils::report_warning(&format!(
                        "Task failed to execute: {}",
                        join_error
                    ));
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
    eprintln!("\n📊 Thumbnail Download Statistics Report");
    eprintln!("=====================================");
    eprintln!("✅ Successfully downloaded: {}", success_count);
    eprintln!("⏭️  No thumbnail available: {}", missing_count);
    eprintln!("❌ Failed downloads: {}", error_count);
    eprintln!("📁 Total assets processed: {}", total_assets);
    if error_count > 0 {
        eprintln!("⏳ Operation completed with errors!");
    } else {
        eprintln!("⏳ Operation completed successfully!");
    }
    eprintln!(
        "\n📁 Thumbnails downloaded to destination directory: {:?}",
        dest_dir
    );

    if error_count > 0 {
        return Err(CliError::ActionError(
            crate::actions::CliActionError::PartialFailure {
                failed: error_count,
                total: total_assets,
                what: "thumbnail download(s)".to_string(),
            },
        ));
    }

    Ok(())
}

/// What one thumbnail task achieved.
enum ThumbnailOutcome {
    Downloaded,
    /// The asset has no thumbnail; nothing was written.
    NoThumbnail,
}

/// Download an asset thumbnail.
///
/// The client already renews the token and retries once on 401/403, and retries
/// transient failures, so there is nothing left to loop over here: an error that
/// comes back is final for this asset. (An earlier version renewed the token up to
/// three more times per asset, which on a Viewer account meant three auth-server
/// calls for every thumbnail in a folder.)
async fn download_asset_thumbnail_with_retry(
    api: &mut PhysnaApiClient,
    tenant_id: &str,
    asset_id: &str,
    asset_name: &str,
) -> Result<Vec<u8>, ApiError> {
    match api.download_asset_thumbnail(tenant_id, asset_id).await {
        Ok(content) => Ok(content),
        Err(ApiError::NotFoundError(msg)) if msg.contains("Asset thumbnail not found") => {
            tracing::debug!(
                "Thumbnail not found for asset '{}', skipping: {}",
                asset_name,
                msg
            );
            Err(ApiError::NotFoundError(msg))
        }
        Err(e) => Err(e),
    }
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
        .get_one::<std::path::PathBuf>(crate::commands::params::PARAMETER_INPUT)
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

    // Report and stop before resolving the remote folder when --dry-run is
    // given: resolution may create the target folder, which a dry run must
    // never do.
    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(local_dir_path)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| !path.is_dir())
            .collect();
        files.sort();

        let target = folder_path_param.cloned().unwrap_or_else(|| {
            folder_uuid_param
                .map(|uuid| uuid.to_string())
                .unwrap_or_default()
        });
        println!(
            "Dry run: would upload {} file(s) from '{}' to folder '{}':",
            files.len(),
            local_dir_path.display(),
            target
        );
        for file in &files {
            println!("  {}", file.display());
        }
        println!("Note: the target folder would be created if it does not exist.");
        return Ok(());
    }

    // Resolve the folder UUID - first try to get existing folder, then create if needed.
    // The root has no UUID and is represented by the nil UUID.
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if folder_path_param
        .map(|p| crate::model::normalize_path(p) == "/")
        .unwrap_or(false)
    {
        Uuid::nil()
    } else if let Some(path) = folder_path_param {
        // Try to resolve the folder UUID by path
        match resolve_folder_uuid_by_path(&mut api, &tenant, path).await {
            Ok(uuid) => uuid,
            Err(CliError::FolderNotFound(_, _)) => {
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

                // Create the new folder or get existing folder UUID
                let folder_uuid = match api
                    .create_folder(&tenant.uuid, &folder_name, parent_folder_uuid)
                    .await
                {
                    Ok(response) => {
                        tracing::trace!("Created new folder with UUID: {}", response.folder.uuid);
                        response.folder.uuid
                    }
                    Err(crate::physna_v3::ApiError::ConflictError(msg))
                        if msg.contains("already exists") =>
                    {
                        // Folder already exists, invalidate the cache and resolve its UUID
                        tracing::trace!("Folder already exists, invalidating cache and resolving UUID for path: {}", path);
                        let _ =
                            crate::folder_cache::FolderCache::invalidate(&tenant.uuid.to_string()); // Ignore error during cache invalidation
                        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
                    }
                    Err(e) => return Err(CliError::PhysnaExtendedApiError(e)),
                };
                folder_uuid
            }
            Err(e) => return Err(e),
        }
    } else {
        // Neither folder UUID nor path provided
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // From here on the destination is the folder's canonical path, never the
    // user's spelling of it (see resolve_upload_destination for why).
    let original_folder_path = if folder_uuid.is_nil() {
        "/".to_string()
    } else {
        crate::actions::utils::canonical_folder_path(&mut api, &tenant.uuid, &folder_uuid).await?
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

    // Only files are uploaded; excluding directories up front keeps the
    // total (and therefore the skipped/failed accounting) accurate.
    let entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| !entry.path().is_dir())
        .collect();

    // Store the total count before moving entries
    let total_entries_count = entries.len();

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent_param));

    // Create progress bars if requested
    let (progress_bar, multi_progress) = if show_progress {
        let mp = indicatif::MultiProgress::new();
        let pb = mp.add(indicatif::ProgressBar::new(total_entries_count as u64));
        pb.set_style(indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
            .unwrap()
            .progress_chars("#>-"));
        (Some(pb), Some(mp))
    } else {
        (None, None)
    };

    // Create a delay duration if delay is specified
    let delay_duration = std::time::Duration::from_secs(delay_param as u64);

    // Which of the files already exist in the destination, asked once. Each task
    // used to list the whole folder for itself - N files times every page of the
    // listing - and treated a failed listing as "does not exist", so
    // --skip-existing could re-upload on a transient error. The server is now
    // asked about the exact target paths (one request per 1000 files) and a
    // failed check fails the run. The set holds the paths as they were asked
    // about; each task rebuilds its own path the same way to test membership.
    let existing_paths: std::sync::Arc<std::collections::HashSet<String>> = {
        let requested: Vec<String> = entries
            .iter()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                entry
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .map(|name| crate::actions::utils::asset_path_for(&original_folder_path, &name))
            .collect();
        std::sync::Arc::new(
            api.find_existing_asset_paths(&tenant.uuid, &requested)
                .await?,
        )
    };

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
        // The upload variant shares the token and renewal state with `api`.
        let mut api_task = api.for_upload_operations();
        let existing_paths = existing_paths.clone();
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
                            .template("{spinner:.yellow} [{elapsed_precise}] {msg}")
                            .unwrap(),
                    );
                    individual_pb.set_message(format!("Uploading: {}", file_name_str));
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

            let asset_exists = existing_paths.contains(&crate::actions::utils::asset_path_for(
                &original_folder_path_clone,
                &file_name_str,
            ));

            if asset_exists {
                if skip_existing_clone {
                    // Update individual progress bar for skipped asset
                    if let Some(ref ipb) = individual_pb {
                        ipb.set_message(format!("Skipped (exists): {}", file_name_str));
                        ipb.finish_and_clear(); // Clear the spinner for this individual upload
                    }

                    eprintln!("Skipping existing asset: {}", file_name_str);
                    // Update overall progress bar if present
                    if let Some(ref pb) = progress_bar_clone {
                        pb.inc(1);
                    }
                    return Ok(Ok((file_name_str, true)));
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

            // Construct the asset path using the original folder path and file name
            // Remove leading slash if present to avoid path conflicts
            let asset_path = match original_folder_path_clone.trim_matches('/') {
                "" => file_name_str.clone(),
                parent => format!("{}/{}", parent, file_name_str),
            };

            // Upload the asset to the specified folder using the full path
            let upload_result = api_task
                .create_asset(
                    &tenant_clone.uuid,
                    &file_path,
                    &asset_path,
                    &folder_uuid_clone,
                )
                .await;

            // Clean up the temporary file

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

                    Ok(Ok((file_name_str, false)))
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
    let mut skipped_count = 0;

    for task in tasks {
        match task.await {
            Ok(task_result) => {
                match task_result {
                    Ok(asset_result) => {
                        match asset_result {
                            Ok((asset_name, was_skipped)) => {
                                if was_skipped {
                                    skipped_count += 1;
                                } else {
                                    success_count += 1;
                                    // Only print individual success messages if progress is not shown
                                    // Otherwise, the progress bar already shows the status
                                    if !show_progress {
                                        eprintln!("Successfully uploaded: {}", asset_name);
                                    }
                                }
                            }
                            Err(cli_error) => {
                                error_count += 1;
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

    // Calculate total assets processed
    let total_assets = total_entries_count;

    // Finish progress bar before the summary so it does not paint over it
    if let Some(pb) = progress_bar {
        pb.finish_and_clear();
    }

    // Print detailed statistics report (stderr: stdout is for data)
    eprintln!("\n📊 Upload Statistics Report");
    eprintln!("===========================");
    eprintln!("✅ Successfully uploaded: {}", success_count);
    eprintln!("⏭️  Skipped (already existed): {}", skipped_count);
    eprintln!("❌ Failed uploads: {}", error_count);
    eprintln!("📁 Total assets processed: {}", total_assets);
    if error_count > 0 {
        eprintln!("⏳ Operation completed with errors!");
    } else {
        eprintln!("⏳ Operation completed successfully!");
    }
    eprintln!("\n📁 Source directory: {:?}", local_dir_path);
    eprintln!("📁 Destination folder: {}", original_folder_path);

    if error_count > 0 {
        return Err(CliError::ActionError(
            crate::actions::CliActionError::PartialFailure {
                failed: error_count,
                total: total_assets,
                what: "upload(s)".to_string(),
            },
        ));
    }

    Ok(())
}

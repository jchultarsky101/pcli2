use crate::physna_v3::ApiError;
use clap::ArgMatches;
use std::path::PathBuf;
use std::time::Duration;
use tracing::trace;
use uuid::Uuid;

use crate::actions::bulk::{ItemFailure, ItemOutcome};
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
    // Optional; checked before anything is created.
    let description = sub_matches
        .get_one::<String>("description")
        .map(|text| folder_description(text))
        .transpose()?;

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
        .create_folder(
            &tenant.uuid,
            name.as_str(),
            parent_folder_uuid,
            description.as_deref(),
        )
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

/// The longest description the API accepts, in characters.
const MAX_FOLDER_DESCRIPTION_CHARS: usize = 255;

/// Check a folder description the way the API does, so a mistake is a usage
/// error (exit 64) before any request: the server drops surrounding spaces and
/// then wants 1 to 255 characters.
fn folder_description(text: &str) -> Result<String, CliError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CliError::InvalidArgument(
            "The description is empty; 'pcli2 folder description clear' removes one".to_string(),
        ));
    }
    let length = text.chars().count();
    if length > MAX_FOLDER_DESCRIPTION_CHARS {
        return Err(CliError::InvalidArgument(format!(
            "The description is {} characters long; at most {} are allowed",
            length, MAX_FOLDER_DESCRIPTION_CHARS
        )));
    }
    Ok(text.to_string())
}

/// `folder description get`: the description alone on stdout, or nothing
/// when the folder has none.
pub async fn get_folder_description(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant = ctx.tenant().clone();
    let folder = crate::actions::utils::resolve_folder(
        ctx.api(),
        &tenant,
        sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID),
        sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH),
    )
    .await?;
    if let Some(description) = folder.description() {
        println!("{}", description);
    }
    Ok(())
}

/// `folder description set --text TEXT`. Silent on success.
pub async fn set_folder_description(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let text = folder_description(
        sub_matches
            .get_one::<String>("text")
            .map(String::as_str)
            .unwrap_or_default(),
    )?;
    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant = ctx.tenant().clone();
    let folder = crate::actions::utils::resolve_folder(
        ctx.api(),
        &tenant,
        sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID),
        sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH),
    )
    .await?;
    ctx.api()
        .set_folder_description(&tenant.uuid, folder.uuid(), &text)
        .await?;
    invalidate_folder_cache(&tenant.uuid);
    Ok(())
}

/// `folder description clear`. Silent on success, including when the folder
/// had no description.
pub async fn clear_folder_description(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant = ctx.tenant().clone();
    let folder = crate::actions::utils::resolve_folder(
        ctx.api(),
        &tenant,
        sub_matches.get_one::<Uuid>(PARAMETER_FOLDER_UUID),
        sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH),
    )
    .await?;
    ctx.api()
        .clear_folder_description(&tenant.uuid, folder.uuid())
        .await?;
    invalidate_folder_cache(&tenant.uuid);
    Ok(())
}

/// The cached folder listing still shows the old description; drop it.
fn invalidate_folder_cache(tenant_uuid: &Uuid) {
    crate::folder_cache::FolderCache::invalidate(&tenant_uuid.to_string()).unwrap_or_else(|e| {
        tracing::debug!("Failed to invalidate folder cache: {}", e);
    });
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
    std::fs::create_dir_all(&dest_dir)?;

    let root_folder_path = match target {
        FolderTarget::Root => "/".to_string(),
        // Also confirms the folder exists before anything else is fetched.
        FolderTarget::Folder(folder_uuid) => {
            api.get_folder(&tenant.uuid, &folder_uuid).await?.path()
        }
    };

    let mut all_assets_with_paths = Vec::new();
    // Assets left out because they have no downloadable result yet (still
    // indexing) or processing failed: (Physna path, state). They used to be dropped
    // without a word, missing from the totals, and the run still exited 0.
    let mut not_downloadable: Vec<(String, String)> = Vec::new();
    for (asset, directory) in collect_folder_assets(&mut api, &tenant, target).await? {
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
        // Files keep the asset's own name; an assembly's bundle is unpacked next
        // to it (see `download_assembly`).
        let relative_path = join_relative(&directory, &asset.name());
        let physna_path = asset.path().clone();
        all_assets_with_paths.push((asset, relative_path, physna_path));
    }

    if all_assets_with_paths.is_empty() {
        crate::error_utils::report_warning(&format!(
            "No downloadable assets found in folder {} or its subfolders ({} not processed yet or failed processing); nothing to download",
            root_folder_path,
            not_downloadable.len()
        ));
        return Ok(());
    }

    let options = bulk_options(sub_matches, "Downloading");
    let resume_flag = sub_matches.get_flag(crate::commands::params::PARAMETER_RESUME);

    // Every destination is checked before anything is downloaded.
    let mut items = Vec::with_capacity(all_assets_with_paths.len());
    for (asset, relative_path, physna_path) in all_assets_with_paths {
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
        items.push((
            asset.name().to_string(),
            (asset, asset_file_path, physna_path),
        ));
    }

    let tenant_id = tenant.uuid.to_string();
    let report = crate::actions::bulk::run_bulk(
        items,
        &options,
        move |(asset, asset_file_path, physna_path), context| {
            let mut api = api.clone();
            let tenant_id = tenant_id.clone();
            async move {
                // With --resume a file already on disk is not downloaded again. For
                // an assembly that is the assembly file itself, which
                // `download_assembly` writes only after all of its parts.
                if resume_flag && asset_file_path.exists() {
                    tracing::debug!("Skipping existing file: {}", asset_file_path.display());
                    return Ok(ItemOutcome::Skipped);
                }
                context.pace().await;

                let asset_name = asset.name().to_string();
                let asset_id = asset.uuid().to_string();
                // Streamed to disk through a temporary file (see
                // download_asset_to_file); the client handles renewal and transient
                // retries itself. Assemblies go through the same path as `asset
                // download`, which unpacks a dependency bundle and keeps a raw
                // assembly file as it is.
                let downloaded: Result<(), CliError> = if asset.is_assembly() {
                    crate::actions::assets::download::download_assembly(
                        &mut api,
                        &tenant_id,
                        &asset_id,
                        &asset_name,
                        &asset_file_path,
                    )
                    .await
                    .map(|_| ())
                } else {
                    api.download_asset_to_file(
                        &tenant_id,
                        &asset_id,
                        Some(&asset_name),
                        &asset_file_path,
                    )
                    .await
                    .map(|_| ())
                    .map_err(CliError::PhysnaExtendedApiError)
                };
                downloaded
                    .map(|()| ItemOutcome::Done)
                    .map_err(|error| ItemFailure {
                        message: format!(
                            "⚠️  Failed to download asset '{}' (Physna path: {}): {}",
                            asset_name,
                            physna_path,
                            download_error_text(&error)
                        ),
                        error,
                    })
            }
        },
    )
    .await;

    print_download_summary(
        report.done,
        report.skipped,
        report.failed(),
        report.not_attempted,
        not_downloadable.len(),
        report.total,
        &dest_dir,
    );
    // After the summary, so they stay on screen.
    report.print_failures();
    if !not_downloadable.is_empty() {
        eprintln!();
        eprintln!("⏭️  Not downloaded (not processed yet, or processing failed):");
        for (path, state) in &not_downloadable {
            eprintln!("   {} ({})", path, state);
        }
        eprintln!("   'pcli2 asset diagnose --path <path>' explains a failed asset.");
    }

    report.into_result(options.continue_on_error, "download(s)")
}

/// The run options every folder bulk command shares.
fn bulk_options(sub_matches: &ArgMatches, verb: &'static str) -> crate::actions::bulk::BulkOptions {
    crate::actions::bulk::BulkOptions {
        concurrency: sub_matches
            .get_one::<usize>(crate::commands::params::PARAMETER_CONCURRENT)
            .copied()
            .unwrap_or(1),
        delay: Duration::from_secs(
            sub_matches
                .get_one::<usize>(crate::commands::params::PARAMETER_DELAY)
                .copied()
                .unwrap_or(0) as u64,
        ),
        continue_on_error: sub_matches
            .get_flag(crate::commands::params::PARAMETER_CONTINUE_ON_ERROR),
        show_progress: crate::terminal::show_progress(sub_matches),
        verb,
    }
}

/// Every asset under `target`, with the directory (relative to `target`) each
/// one belongs in, ordered by that directory and then by name.
///
/// The subtree comes from the folder tree (one request per thousand folders,
/// usually already cached) and the folders' assets are listed several at a time.
/// `folder download` and `folder thumbnail` used to walk the tree one folder at a
/// time with two requests per folder, so a large tree took minutes before the
/// first file arrived.
async fn collect_folder_assets(
    api: &mut PhysnaApiClient,
    tenant: &Tenant,
    target: FolderTarget,
) -> Result<Vec<(crate::model::Asset, String)>, CliError> {
    use futures::stream::{self, StreamExt};

    let hierarchy = crate::folder_cache::FolderCache::get_or_fetch(api, &tenant.uuid).await?;
    let path_of = |uuid: &Uuid| hierarchy.get_path_for_folder(uuid).unwrap_or_default();
    let (base, mut folders): (String, Vec<(Option<Uuid>, String)>) = match target {
        FolderTarget::Root => (
            String::new(),
            std::iter::once((None, String::new()))
                .chain(
                    hierarchy
                        .all_subtree_uuids()
                        .into_iter()
                        .map(|uuid| (Some(uuid), path_of(&uuid))),
                )
                .collect(),
        ),
        FolderTarget::Folder(root) => {
            let subtree = hierarchy.subtree_uuids(&root);
            if subtree.is_empty() {
                return Err(CliError::PhysnaExtendedApiError(
                    ApiError::FolderHierarchyUnavailable(format!(
                        "folder {} is not in the tenant's folder tree",
                        root
                    )),
                ));
            }
            (
                path_of(&root),
                subtree
                    .into_iter()
                    .map(|uuid| (Some(uuid), path_of(&uuid)))
                    .collect(),
            )
        }
    };
    for (_, path) in &mut folders {
        *path = path
            .strip_prefix(base.as_str())
            .unwrap_or(path)
            .trim_matches('/')
            .to_string();
    }

    let tenant_uuid = tenant.uuid;
    let listings: Vec<Result<(String, crate::model::AssetList), ApiError>> = stream::iter(folders)
        .map(|(uuid, relative)| {
            let mut api = api.clone();
            async move {
                api.list_assets_by_parent_folder_uuid(&tenant_uuid, uuid.as_ref())
                    .await
                    .map(|listing| (relative, listing))
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;

    let mut assets = Vec::new();
    for listing in listings {
        let (relative, listing) = listing?;
        for asset in listing.get_all_assets() {
            assets.push((asset.clone(), relative.clone()));
        }
    }
    assets.sort_by(|a, b| (&a.1, a.0.name()).cmp(&(&b.1, b.0.name())));
    Ok(assets)
}

/// Join a directory relative to the download root and a file name.
fn join_relative(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", directory, name)
    }
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

    // The subtree is read from the folder tree; a cached one could miss a folder
    // created since, so it is fetched fresh, as `folder download` does.
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
    std::fs::create_dir_all(&dest_dir)?;

    let root_folder_path = match target {
        FolderTarget::Root => "/".to_string(),
        // Also confirms the folder exists before anything else is fetched.
        FolderTarget::Folder(folder_uuid) => {
            api.get_folder(&tenant.uuid, &folder_uuid).await?.path()
        }
    };

    // One thumbnail per asset: `<directory>/<asset name without extension>.png`.
    let mut all_assets_with_paths = Vec::new();
    for (asset, directory) in collect_folder_assets(&mut api, &tenant, target).await? {
        let name = asset.name();
        let stem = std::path::Path::new(&name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        let relative_path = join_relative(&directory, &format!("{}.png", stem));
        let physna_path = asset.path().clone();
        all_assets_with_paths.push((asset, relative_path, physna_path));
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

    let options = bulk_options(sub_matches, "Downloading thumbnail");

    // Every destination is checked before anything is downloaded.
    let mut items = Vec::with_capacity(all_assets_with_paths.len());
    for (asset, relative_path, physna_path) in all_assets_with_paths {
        let thumbnail_path = match crate::actions::utils::safe_relative_path(&relative_path) {
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
        items.push((
            asset.name().to_string(),
            (asset, thumbnail_path, physna_path),
        ));
    }

    let tenant_id = tenant.uuid.to_string();
    let report = crate::actions::bulk::run_bulk(
        items,
        &options,
        move |(asset, thumbnail_path, physna_path), context| {
            let mut api = api.clone();
            let tenant_id = tenant_id.clone();
            async move {
                context.pace().await;
                let asset_name = asset.name().to_string();
                let asset_id = asset.uuid().to_string();
                let fail = |error: CliError| ItemFailure {
                    message: format!(
                        "⚠️  Failed to download thumbnail for asset '{}' (Physna path: {}): {}",
                        asset_name,
                        physna_path,
                        download_error_text(&error)
                    ),
                    error,
                };
                match download_asset_thumbnail_with_retry(
                    &mut api,
                    &tenant_id,
                    &asset_id,
                    &asset_name,
                )
                .await
                {
                    Ok(content) => {
                        if let Some(parent) = thumbnail_path.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| fail(e.into()))?;
                        }
                        // Written whole or not at all: an interrupted write used to
                        // leave a truncated PNG under the final name.
                        crate::fs_utils::write_atomically(&thumbnail_path, &content)
                            .map_err(|e| fail(e.into()))?;
                        Ok(ItemOutcome::Done)
                    }
                    // Not a failure and not a success: nothing was written, and the
                    // summary says so rather than claiming a thumbnail.
                    Err(ApiError::NotFoundError(msg))
                        if msg.contains("Asset thumbnail not found") =>
                    {
                        tracing::debug!(
                            "Thumbnail not found for asset '{}' (Physna path: {}): {}",
                            asset_name,
                            physna_path,
                            msg
                        );
                        Ok(ItemOutcome::Unavailable)
                    }
                    Err(e) => Err(fail(CliError::PhysnaExtendedApiError(e))),
                }
            }
        },
    )
    .await;

    // Report summary with nice statistics
    eprintln!("\n📊 Thumbnail Download Statistics Report");
    eprintln!("=====================================");
    eprintln!("✅ Successfully downloaded: {}", report.done);
    eprintln!("⏭️  No thumbnail available: {}", report.unavailable);
    eprintln!("❌ Failed downloads: {}", report.failed());
    if report.not_attempted > 0 {
        eprintln!(
            "⏹️  Not attempted (stopped after the first failure): {}",
            report.not_attempted
        );
    }
    eprintln!("📁 Total assets processed: {}", report.total);
    if report.failed() > 0 || report.not_attempted > 0 {
        eprintln!("⏳ Operation completed with errors!");
    } else {
        eprintln!("⏳ Operation completed successfully!");
    }
    eprintln!(
        "\n📁 Thumbnails downloaded to destination directory: {:?}",
        dest_dir
    );
    report.print_failures();

    report.into_result(options.continue_on_error, "thumbnail download(s)")
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
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(local_dir_path)?
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
                    .create_folder(&tenant.uuid, &folder_name, parent_folder_uuid, None)
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

    let skip_existing = sub_matches.get_flag(crate::commands::params::PARAMETER_SKIP_EXISTING);
    let options = bulk_options(sub_matches, "Uploading");

    // Only files are uploaded; directories are left out up front so the totals
    // count what is actually attempted.
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(local_dir_path)? {
        let path = entry?.path();
        if path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                CliError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid file name encoding: {:?}", path),
                ))
            })?
            .to_string();
        files.push((name, path));
    }
    files.sort();

    // Which of the files already exist in the destination, asked once (one
    // request per 1000 files) rather than by listing the folder per file.
    let requested: Vec<String> = files
        .iter()
        .map(|(name, _)| crate::actions::utils::asset_path_for(&original_folder_path, name))
        .collect();
    let existing = api
        .find_existing_asset_paths(&tenant.uuid, &requested)
        .await?;

    // Without --skip-existing an existing file is an error. It is raised before
    // anything is uploaded: it used to surface only when that file's turn came,
    // after the files ahead of it had already been uploaded.
    if !skip_existing {
        let clashes: Vec<&str> = files
            .iter()
            .zip(&requested)
            .filter(|(_, target)| existing.contains(*target))
            .map(|((name, _), _)| name.as_str())
            .collect();
        if !clashes.is_empty() {
            return Err(CliError::ActionError(
                crate::actions::CliActionError::BusinessLogicError(format!(
                    "Asset already exists: {}. Use --skip-existing to skip existing assets.",
                    clashes.join(", ")
                )),
            ));
        }
    }

    let items: Vec<(String, (String, std::path::PathBuf, bool))> = files
        .into_iter()
        .zip(requested)
        .map(|((name, path), target)| {
            let exists = existing.contains(&target);
            (name.clone(), (name, path, exists))
        })
        .collect();

    let upload_api = api.for_upload_operations();
    let tenant_uuid = tenant.uuid;
    let destination = original_folder_path.clone();
    let show_progress = options.show_progress;
    let report = crate::actions::bulk::run_bulk(
        items,
        &options,
        move |(file_name, file_path, exists), context| {
            // The upload variant shares the token and renewal state with `api`.
            let mut api = upload_api.clone();
            let destination = destination.clone();
            async move {
                if exists {
                    context.note(&format!("Skipping existing asset: {}", file_name));
                    return Ok(ItemOutcome::Skipped);
                }
                context.pace().await;

                let asset_path = match destination.trim_matches('/') {
                    "" => file_name.clone(),
                    parent => format!("{}/{}", parent, file_name),
                };
                tracing::trace!(
                    "Uploading asset: {} to folder UUID: {}",
                    file_name,
                    folder_uuid
                );
                match api
                    .create_asset(&tenant_uuid, &file_path, &asset_path, &folder_uuid)
                    .await
                {
                    Ok(_) => {
                        // The progress bar shows this already.
                        if !show_progress {
                            context.note(&format!("Successfully uploaded: {}", file_name));
                        }
                        Ok(ItemOutcome::Done)
                    }
                    Err(e) => Err(ItemFailure {
                        message: format!(
                            "⚠️  Failed to upload '{}' (as {}): {}",
                            file_name, asset_path, e
                        ),
                        error: CliError::PhysnaExtendedApiError(e),
                    }),
                }
            }
        },
    )
    .await;

    // Print detailed statistics report (stderr: stdout is for data)
    eprintln!("\n📊 Upload Statistics Report");
    eprintln!("===========================");
    eprintln!("✅ Successfully uploaded: {}", report.done);
    eprintln!("⏭️  Skipped (already existed): {}", report.skipped);
    eprintln!("❌ Failed uploads: {}", report.failed());
    if report.not_attempted > 0 {
        eprintln!(
            "⏹️  Not attempted (stopped after the first failure): {}",
            report.not_attempted
        );
    }
    eprintln!("📁 Total assets processed: {}", report.total);
    if report.failed() > 0 || report.not_attempted > 0 {
        eprintln!("⏳ Operation completed with errors!");
    } else {
        eprintln!("⏳ Operation completed successfully!");
    }
    eprintln!("\n📁 Source directory: {:?}", local_dir_path);
    eprintln!("📁 Destination folder: {}", original_folder_path);
    report.print_failures();

    report.into_result(options.continue_on_error, "upload(s)")
}

#[cfg(test)]
mod folder_description_tests {
    use super::folder_description;

    #[test]
    fn surrounding_spaces_go_and_the_limit_counts_characters() {
        assert_eq!(folder_description("  Rail parts  ").unwrap(), "Rail parts");
        // 255 two-byte characters are 510 bytes, and allowed.
        assert_eq!(
            folder_description(&"é".repeat(255))
                .unwrap()
                .chars()
                .count(),
            255
        );
        let too_long = folder_description(&"x".repeat(256)).unwrap_err();
        assert!(
            too_long.to_string().contains("256 characters"),
            "{too_long}"
        );
        assert_eq!(
            too_long.exit_code(),
            crate::exit_codes::PcliExitCode::UsageError
        );
    }

    #[test]
    fn an_empty_description_points_at_clear() {
        for text in ["", "   ", "\t\n"] {
            let error = folder_description(text).unwrap_err();
            assert!(
                error.to_string().contains("folder description clear"),
                "{error}"
            );
        }
    }
}

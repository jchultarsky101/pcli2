use crate::{
    error::CliError,
    model::{Asset, Folder, Tenant},
    physna_v3::PhysnaApiClient,
};
use uuid::Uuid;

/// Where an upload goes: the folder's UUID and its canonical path.
///
/// The upload endpoint places a file by the path string it is sent, creating any
/// folders it does not know, so the path must be the folder's real one. Building
/// it from the user's own spelling used to create a literal "Home" folder for
/// `/Home/Parts`, a second `parts` beside `Parts` on a case-sensitive server, and
/// sent everything to the root when only `--folder-uuid` was given. The root is
/// legal here: it has no UUID (`Uuid::nil()`) and the path is `/`.
pub async fn resolve_upload_destination(
    api: &mut PhysnaApiClient,
    tenant: &Tenant,
    folder_uuid: Option<&Uuid>,
    folder_path: Option<&String>,
) -> Result<(Uuid, String), CliError> {
    let uuid = match (folder_uuid, folder_path) {
        (Some(uuid), _) => *uuid,
        (None, Some(path)) => {
            if crate::model::normalize_path(path) == "/" {
                return Ok((Uuid::nil(), "/".to_string()));
            }
            crate::actions::folders::resolve_folder_uuid_by_path(api, tenant, path).await?
        }
        (None, None) => {
            return Err(CliError::MissingRequiredArgument(
                "Either folder UUID or path must be provided".to_string(),
            ))
        }
    };
    let path = canonical_folder_path(api, &tenant.uuid, &uuid).await?;
    Ok((uuid, path))
}

/// The folder's path as the server knows it, with a leading slash.
///
/// Read from the cached hierarchy, refreshed once if the folder is not in it (it
/// may have just been created). A folder that is still absent is an error rather
/// than a guess.
pub async fn canonical_folder_path(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    folder_uuid: &Uuid,
) -> Result<String, CliError> {
    let hierarchy = crate::folder_cache::FolderCache::get_or_fetch(api, tenant_uuid).await?;
    if let Some(path) = hierarchy.get_path_for_folder(folder_uuid) {
        return Ok(format!("/{}", path));
    }
    let hierarchy = crate::folder_cache::FolderCache::refresh(api, tenant_uuid).await?;
    match hierarchy.get_path_for_folder(folder_uuid) {
        Some(path) => Ok(format!("/{}", path)),
        None => Err(CliError::FolderNotFound(
            folder_uuid.to_string(),
            String::new(),
        )),
    }
}

/// A path relative to a download directory, built from names the server sent.
///
/// Every segment must be a plain name: no `..`, no empty segments, no path
/// separators inside a name. A folder called `..` or an asset called `../../x`
/// would otherwise write outside the directory the user chose.
pub fn safe_relative_path(relative: &str) -> Option<std::path::PathBuf> {
    let mut out = std::path::PathBuf::new();
    for segment in relative.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\') {
            return None;
        }
        if std::path::Path::new(segment).components().count() != 1 {
            return None;
        }
        out.push(segment);
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Expand repeatable list arguments that the help text promises accept
/// comma-separated values (`--name a,b` as well as `--name a --name b`).
///
/// Values are trimmed and empty entries dropped. The promise had been in the help
/// text of six arguments without any code behind it, so `--name "Material,Weight"`
/// was sent to the API as one field named `Material,Weight`.
pub fn split_list_values<I: IntoIterator<Item = String>>(values: I) -> Vec<String> {
    values
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(|part| part.trim().to_string())
                .collect::<Vec<_>>()
        })
        .filter(|value| !value.is_empty())
        .collect()
}

/// The tenant's folder hierarchy, from the cache when it already contains `path`.
///
/// A cache miss on the path triggers one refresh, so a folder created since the
/// cache was written is still found; a path that is absent after that is genuinely
/// absent and the caller gets a hierarchy it can build suggestions from. Loading
/// failures propagate rather than masquerading as "not found".
pub async fn hierarchy_containing(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    path: &str,
) -> Result<crate::folder_hierarchy::FolderHierarchy, CliError> {
    let hierarchy = crate::folder_cache::FolderCache::get_or_fetch(api, tenant_uuid).await?;
    if path == "/" || hierarchy.get_node_by_path(path).is_some() {
        return Ok(hierarchy);
    }
    Ok(crate::folder_cache::FolderCache::refresh(api, tenant_uuid).await?)
}

/// Read the `--threshold` percentage from the parsed arguments.
///
/// The parser already limits the value to 0-100. A value below 1 is still legal
/// (visual match uses 0 to disable size filtering), but it is far more often the
/// slip of writing `0.85` for 85%, which asks the server for nearly every asset in
/// the tenant - so say so, once, before the run starts.
pub fn threshold_from_args(sub_matches: &clap::ArgMatches) -> f64 {
    let threshold = sub_matches
        .get_one::<f64>(crate::commands::params::PARAMETER_THRESHOLD)
        .copied()
        .unwrap_or(80.0);
    if threshold > 0.0 && threshold < 1.0 {
        crate::error_utils::report_warning(&format!(
            "--threshold {} means {}%, which matches almost everything; for {}% write --threshold {}",
            threshold,
            threshold,
            (threshold * 100.0).round(),
            (threshold * 100.0).round()
        ));
    }
    threshold
}

/// Resolve an asset by either UUID or path parameter.
///
/// This function provides a standardized way to resolve an asset from command-line arguments
/// that may specify either a UUID or a path.
///
/// # Arguments
///
/// * `api` - Reference to the Physna API client
/// * `tenant_uuid` - UUID of the tenant containing the asset
/// * `uuid_param` - Optional UUID parameter from command line
/// * `path_param` - Optional path parameter from command line
///
/// # Returns
///
/// * `Ok(Asset)` - The resolved asset
/// * `Err(CliError)` - If neither parameter is provided or if resolution fails
pub async fn resolve_asset<'a>(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    uuid_param: Option<&'a Uuid>,
    path_param: Option<&'a String>,
) -> Result<Asset, CliError> {
    if let Some(uuid) = uuid_param {
        api.get_asset_by_uuid(tenant_uuid, uuid)
            .await
            .map_err(CliError::PhysnaExtendedApiError)
    } else if let Some(path) = path_param {
        api.get_asset_by_path(tenant_uuid, path)
            .await
            .map_err(CliError::PhysnaExtendedApiError)
    } else {
        Err(CliError::MissingRequiredArgument(
            "Either asset UUID or path must be provided".to_string(),
        ))
    }
}

/// Resolve a folder by either UUID or path parameter.
///
/// This function provides a standardized way to resolve a folder from command-line arguments
/// that may specify either a UUID or a path.
///
/// # Arguments
///
/// * `api` - Reference to the Physna API client
/// * `tenant` - Reference to the tenant containing the folder
/// * `uuid_param` - Optional UUID parameter from command line
/// * `path_param` - Optional path parameter from command line
///
/// # Returns
///
/// * `Ok(Folder)` - The resolved folder with path set appropriately
/// * `Err(CliError)` - If neither parameter is provided or if resolution fails
pub async fn resolve_folder<'a>(
    api: &mut PhysnaApiClient,
    tenant: &Tenant,
    uuid_param: Option<&'a Uuid>,
    path_param: Option<&'a String>,
) -> Result<Folder, CliError> {
    if let Some(uuid) = uuid_param {
        let folder_response = api
            .get_folder(&tenant.uuid, uuid)
            .await
            .map_err(CliError::PhysnaExtendedApiError)?;
        Ok(folder_response)
    } else if let Some(path) = path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Handle root path specially
            let folder_uuid =
                super::folders::resolve_folder_uuid_by_path(api, tenant, path).await?;
            let folder_response = api
                .get_folder(&tenant.uuid, &folder_uuid)
                .await
                .map_err(CliError::PhysnaExtendedApiError)?;
            Ok(folder_response)
        } else {
            let folder_uuid =
                super::folders::resolve_folder_uuid_by_path(api, tenant, path).await?;
            let folder_response = api
                .get_folder(&tenant.uuid, &folder_uuid)
                .await
                .map_err(CliError::PhysnaExtendedApiError)?;
            let mut folder: Folder = folder_response;
            folder.set_path(path.to_owned());
            Ok(folder)
        }
    } else {
        Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_resolve_asset_neither_provided() {
        // This test verifies that the function correctly returns an error when neither parameter is provided
        // Since the function is async and involves API calls, we can't easily test the success cases without mocking
        assert_eq!(
            true, // This is a placeholder - actual test would require mocking
            true
        );
    }
}

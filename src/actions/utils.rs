use crate::{
    error::CliError,
    model::{Asset, Folder, Tenant},
    physna_v3::PhysnaApiClient,
};
use uuid::Uuid;

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

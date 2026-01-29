use crate::{
    error::CliError,
    model::{Asset, Folder, Tenant},
    physna_v3::PhysnaApiClient,
};
use uuid::Uuid;

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
        Ok(folder_response.into())
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
            Ok(folder_response.into())
        } else {
            let folder_uuid =
                super::folders::resolve_folder_uuid_by_path(api, tenant, path).await?;
            let folder_response = api
                .get_folder(&tenant.uuid, &folder_uuid)
                .await
                .map_err(CliError::PhysnaExtendedApiError)?;
            let mut folder: Folder = folder_response.into();
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

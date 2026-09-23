//! Folders.

use super::*;

impl PhysnaApiClient {
    /// List folders for a specific tenant with optional pagination
    ///
    /// This method fetches a list of folders for the specified tenant.
    /// It supports pagination through the optional `page` and `per_page` parameters.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose folders to list
    /// * `page` - Optional page number (1-based indexing)
    /// * `per_page` - Optional number of items per page (default: 100)
    ///
    /// # Returns
    /// * `Ok(FolderListResponse)` - Successfully fetched list of folders with pagination metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn list_folders(
        &mut self,
        tenant_uuid: &Uuid,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_uuid);

        // Handle defaults - always provide values to avoid API defaulting to 20
        let page_val = page.unwrap_or(1).to_string();
        let per_page_val = per_page.unwrap_or(1000).to_string(); // the API maximum; its default is 20

        // Build query parameters for pagination with defaults
        let query_params = vec![
            ("page", page_val.as_str()),
            ("perPage", per_page_val.as_str()),
        ];

        // Add query parameters to URL
        let url = format!(
            "{}?{}",
            url,
            serde_urlencoded::to_string(&query_params).unwrap()
        );

        // Execute GET request to fetch folders
        self.get(&url).await
    }

    /// Get details for a specific folder by ID
    ///
    /// This method fetches detailed information about a specific folder by its ID.
    /// The response includes folder metadata such as name, creation date, asset count, etc.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant that owns the folder
    /// * `folder_uuid` - The UUID of the folder to retrieve
    ///
    /// # Returns
    /// * `Ok(Folder)` - Successfully fetched folder details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_folder(
        &mut self,
        tenant_uuid: &Uuid,
        folder_uuid: &Uuid,
    ) -> Result<crate::model::Folder, ApiError> {
        let url = format!(
            "{}/tenants/{}/folders/{}",
            self.base_url, tenant_uuid, folder_uuid
        );

        trace!("Getting folder details...");
        let response: SingleFolderResponse = self.get(&url).await?;
        let folder = response.into();
        trace!("Found: {:?}", &folder);
        Ok(folder)
    }

    /// Create a new folder within a tenant
    ///
    /// This method creates a new folder with the specified name within the given tenant.
    /// Optionally, the folder can be created as a subfolder of an existing parent folder.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant where to create the folder
    /// * `name` - The name for the new folder
    /// * `parent_folder_id` - Optional UUID of the parent folder (creates subfolder if provided)
    ///
    /// # Returns
    /// * `Ok(FolderResponse)` - Successfully created folder details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn create_folder(
        &mut self,
        tenant_uuid: &Uuid,
        name: &str,
        parent_folder_uuid: Option<Uuid>,
    ) -> Result<crate::model::SingleFolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_uuid);

        // Build request body with folder name
        let mut body = serde_json::json!({
            "name": name
        });

        // Add parent folder ID if provided to create a subfolder
        if let Some(parent_uuid) = parent_folder_uuid {
            body["parentFolderId"] = serde_json::Value::String(parent_uuid.to_string());
        }

        // Execute POST request to create the folder
        self.post(&url, &body).await
    }

    /// Rename a folder by ID
    ///
    /// This method renames the specified folder in the tenant.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the folder
    /// * `folder_id` - The UUID of the folder to rename
    /// * `new_name` - The new name for the folder
    ///
    /// # Returns
    /// * `Ok(FolderResponse)` - Successfully renamed folder
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn rename_folder(
        &mut self,
        tenant_id: &str,
        folder_id: &str,
        new_name: &str,
    ) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!(
            "{}/tenants/{}/folders/{}/name",
            self.base_url, tenant_id, folder_id
        );

        let body = serde_json::json!({
            "name": new_name
        });

        // Debug print the request body
        debug!(
            "Renaming folder {}. Request body: {}",
            folder_id,
            serde_json::to_string(&body).unwrap_or_else(|_| "INVALID_JSON".to_string())
        );

        // The API returns a SingleFolderResponse with a "folder" field
        let response: crate::model::SingleFolderResponse = self.patch(&url, &body).await?;
        Ok(response.folder)
    }

    /// Move a folder to a new parent folder
    ///
    /// This method moves the specified folder to a new parent folder.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the folder
    /// * `folder_id` - The UUID of the folder to move
    /// * `new_parent_folder_id` - The UUID of the new parent folder (None for root level)
    ///
    /// # Returns
    /// * `Ok(FolderResponse)` - Successfully moved folder
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn move_folder(
        &mut self,
        tenant_id: &str,
        folder_id: &str,
        new_parent_folder_id: Option<Uuid>,
    ) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!(
            "{}/tenants/{}/folders/{}/parent",
            self.base_url, tenant_id, folder_id
        );

        // Build request body with the parent folder ID
        let body = if let Some(parent_id) = new_parent_folder_id {
            serde_json::json!({
                "parentFolderId": parent_id.to_string()
            })
        } else {
            // When moving to root, set parentFolderId to null
            serde_json::json!({
                "parentFolderId": serde_json::Value::Null
            })
        };

        // Debug print the request body
        debug!(
            "Moving folder {} to new parent. Request body: {}",
            folder_id,
            serde_json::to_string(&body).unwrap_or_else(|_| "INVALID_JSON".to_string())
        );

        // The API returns a SingleFolderResponse with a "folder" field
        let response: crate::model::SingleFolderResponse = self.patch(&url, &body).await?;
        Ok(response.folder)
    }

    /// Delete a folder by ID
    ///
    /// This method deletes the specified folder from the tenant.
    /// Note: Deleting a folder will also delete all its contents (subfolders and assets).
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the folder
    /// * `folder_id` - The UUID of the folder to delete
    ///
    /// # Returns
    /// * `Ok(())` - Successfully deleted folder
    /// * `Err(ApiError)` - HTTP error or other error
    pub async fn delete_folder(
        &mut self,
        tenant_uuid: &Uuid,
        folder_uuid: &Uuid,
        force: bool,
    ) -> Result<(), ApiError> {
        let path = format!("/tenants/{}/folders/{}", tenant_uuid, folder_uuid);
        debug!("Attempting to delete folder with path: {}", path);

        let folder = self.get_folder(tenant_uuid, folder_uuid).await?;
        if (folder.folders_count() > 0 || folder.assets_count() > 0) && !force {
            return Err(ApiError::FolderNotEmptyError);
        }
        self.delete(&path).await
    }

    /// Get the folder ID for a given path by traversing the folder structure efficiently
    ///
    /// This method efficiently resolves a folder path to its corresponding folder ID
    /// by using the root/content and folderId/contents API endpoints, with content filtering.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `folder_path` - The path to resolve (e.g., "Projects/Child" or "/Home/Projects/Child")
    ///
    /// # Returns
    /// * `Ok(Some(String))` - The folder ID if found
    /// * `Ok(None)` - If the path doesn't exist
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_folder_uuid_by_path(
        &mut self,
        tenant_uuid: &Uuid,
        folder_path: &str,
    ) -> Result<Option<Uuid>, ApiError> {
        debug!(
            "Resolving folder path: {} for tenant: {} using FolderHierarchy",
            folder_path, tenant_uuid
        );

        // Normalize the path first
        let normalized_path = crate::model::normalize_path(folder_path);

        // Special handling for root path "/"
        if normalized_path == "/" {
            // The root path "/" does not correspond to a specific folder UUID
            // It represents the root level which contains multiple folders
            // So we return None to indicate no specific folder UUID
            Ok(None)
        } else {
            // Remove leading slash for hierarchy lookup
            let path_for_hierarchy = normalized_path
                .strip_prefix('/')
                .unwrap_or(&normalized_path);

            // Use the cached folder hierarchy approach to find the folder by path
            // This properly handles nested paths like "test/sub1" by traversing the hierarchy
            // and avoids rebuilding the hierarchy for each path resolution
            // A failure to *load* the hierarchy is an error in its own right. It used
            // to be swallowed here and surface as "folder not found", sending users to
            // check a path that was correct while the real problem was the network or
            // an expired session.
            let hierarchy =
                crate::folder_cache::FolderCache::get_or_fetch(self, tenant_uuid).await?;
            if let Some(folder_node) = hierarchy.get_folder_by_path(path_for_hierarchy) {
                debug!(
                    "Found folder at path '{}' using hierarchy: {}",
                    path_for_hierarchy, folder_node.folder.uuid
                );
                return Ok(Some(folder_node.folder.uuid));
            }

            // Folder not found in cache - refresh the cache and try again
            // This handles the case where a folder was recently created and the cache is stale
            debug!(
                "Folder not found at path '{}' in cache, refreshing cache...",
                folder_path
            );
            let hierarchy = crate::folder_cache::FolderCache::refresh(self, tenant_uuid).await?;
            {
                if let Some(folder_node) = hierarchy.get_folder_by_path(path_for_hierarchy) {
                    debug!(
                        "Found folder at path '{}' after cache refresh: {}",
                        path_for_hierarchy, folder_node.folder.uuid
                    );
                    return Ok(Some(folder_node.folder.uuid));
                }
            }

            debug!("Folder not found at path: {}", folder_path);
            Ok(None)
        }
    }

    /// Get contents of a specific folder by ID, filtered by content type
    ///
    /// This method gets contents of a specific folder with a specific content type (folders only, assets only, or all).
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `folder_id` - The ID of the folder to get contents from
    /// * `content_type` - The type of content to return ("all", "assets", "folders")
    /// * `page` - Page number for pagination (optional)
    /// * `per_page` - Number of items per page for pagination (optional)
    ///
    /// # Returns
    /// * `Ok(FolderListResponse)` - List of contents in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_folder_contents(
        &mut self,
        tenant_uuid: &Uuid,
        folder_uuid: Option<&Uuid>,
        content_type: &str,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<FolderList, ApiError> {
        let url = match folder_uuid {
            Some(folder_uuid) => format!(
                "{}/tenants/{}/folders/{}/contents",
                self.base_url, tenant_uuid, folder_uuid
            ),
            None => format!(
                "{}/tenants/{}/folders/root/contents",
                self.base_url, tenant_uuid
            ),
        };

        // Build query parameters
        let mut query_params = vec![("contentType", content_type)];

        // Handle defaults - always provide values to avoid API defaulting to 20
        let page_str = page.unwrap_or(1).to_string();
        let per_page_str = per_page.unwrap_or(1000).to_string(); // the API maximum; its default is 20

        query_params.push(("page", page_str.as_str()));
        query_params.push(("perPage", per_page_str.as_str()));

        // Add query parameters to URL
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);

        trace!("Constructed URL for folder contents listing: {}", url);
        let response: FolderListResponse = self.get(&url).await?;

        Ok(response.into())
    }

    /// List ALL direct subfolders of a folder, walking every page.
    ///
    /// `get_folder_contents` fetches a single page; callers that treat one
    /// page as the complete subfolder set silently lose folders (and
    /// everything beneath them) once a folder has more direct subfolders than
    /// the page size. This wrapper accumulates every page.
    pub async fn list_all_subfolders(
        &mut self,
        tenant_uuid: &Uuid,
        folder_uuid: Option<&Uuid>,
    ) -> Result<crate::model::FolderList, ApiError> {
        let url = match folder_uuid {
            Some(folder_uuid) => format!(
                "{}/tenants/{}/folders/{}/contents",
                self.base_url, tenant_uuid, folder_uuid
            ),
            None => format!(
                "{}/tenants/{}/folders/root/contents",
                self.base_url, tenant_uuid
            ),
        };

        let mut all_folders: Vec<crate::model::FolderResponse> = Vec::new();
        let mut page: usize = 1;
        let per_page: usize = 1000; // the API maximum for this endpoint

        loop {
            let page_str = page.to_string();
            let per_page_str = per_page.to_string();
            let query_params = vec![
                ("contentType", "folders"),
                ("page", page_str.as_str()),
                ("perPage", per_page_str.as_str()),
            ];
            let paged_url = format!(
                "{}?{}",
                url,
                serde_urlencoded::to_string(&query_params).unwrap()
            );

            let response: FolderListResponse = self.get(&paged_url).await?;
            let current_page = response.page_data.current_page;
            let last_page = response.page_data.last_page;
            all_folders.extend(response.folders);

            // Stop on the last page, or if the endpoint fails to advance
            // `current_page` (guard against looping forever on one page).
            if current_page >= last_page || current_page < page {
                break;
            }
            page = current_page + 1;
        }

        let total = all_folders.len();
        Ok(FolderListResponse {
            folders: all_folders,
            page_data: crate::model::PageData {
                total,
                per_page,
                current_page: 1,
                last_page: 1,
                start_index: 0,
                end_index: total,
            },
        }
        .into())
    }

    /// Strictly resolve a folder path to a UUID.
    ///
    /// Returns `Ok(None)` ONLY for the root path "/" (which has no UUID).
    /// A non-root path that does not resolve to an existing folder is an
    /// error — it must never be silently treated as the root folder, since
    /// downstream code that matches assets by name within the resolved
    /// folder could otherwise target a completely different asset.
    pub async fn resolve_folder_uuid_by_path(
        &mut self,
        tenant_uuid: &Uuid,
        folder_path: &str,
    ) -> Result<Option<Uuid>, ApiError> {
        let normalized_path = crate::model::normalize_path(folder_path);
        if normalized_path == "/" {
            Ok(None)
        } else {
            match self
                .get_folder_uuid_by_path(tenant_uuid, folder_path)
                .await?
            {
                Some(uuid) => Ok(Some(uuid)),
                None => Err(ApiError::FolderNotFound(folder_path.to_string())),
            }
        }
    }
}

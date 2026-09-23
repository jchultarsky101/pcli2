//! Assets: listing, lookup, creation, moves and dependencies.

use super::*;

impl PhysnaApiClient {
    /// List all assets in a specific folder by folder UUID
    ///
    /// This method lists assets that are contained in a specific folder using the
    /// /tenants/{tenantId}/folders/{folderId}/contents endpoint with contentType=assets.
    /// This is the efficient way to list assets in a specific folder, unlike the
    /// list_assets method which fetches all assets in the tenant.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The ID of the tenant
    /// * `folder_uuid` - The ID of the folder to list assets from. If None, it will list the root folder
    ///
    /// # Returns
    /// * `Ok(AssetListResponse)` - List of assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_assets_by_parent_folder_uuid(
        &mut self,
        tenant_uuid: &Uuid,
        parent_folder_uuid: Option<&Uuid>,
    ) -> Result<AssetList, ApiError> {
        let mut page: usize = 1;
        let per_page: usize = 1000; // the API maximum for this endpoint
        let mut assets: Vec<Asset> = Vec::new();

        loop {
            let response = self
                .list_assets_by_parent_folder_uuid_with_pagination(
                    tenant_uuid,
                    parent_folder_uuid,
                    page,
                    per_page,
                )
                .await?;
            let partial_asset_list: Vec<Asset> = response.assets.iter().map(|a| a.into()).collect();
            assets.extend(partial_asset_list);

            if response.page_data.current_page >= response.page_data.last_page {
                break;
            }
            // A server that echoes a stale page number would otherwise be asked for
            // the same page forever.
            if response.page_data.current_page < page {
                warn!(
                    "Asset listing returned page {} when page {} was requested; stopping with {} asset(s)",
                    response.page_data.current_page,
                    page,
                    assets.len()
                );
                break;
            }

            // Increment the current page
            page = response.page_data.current_page + 1;
        }

        Ok(assets.into())
    }

    /// List only the assets in a specific folder by path
    ///
    /// This method efficiently lists assets in a specific folder by first
    /// resolving the folder path to a folder ID and then listing assets in that folder.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant
    /// * `parent_folder_path` - The path of the folder to list assets from
    ///
    /// # Returns
    /// * `Ok(AssetListResponse)` - List of assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_assets_by_parent_folder_path(
        &mut self,
        tenant_uuid: &Uuid,
        parent_folder_path: &str,
    ) -> Result<AssetList, ApiError> {
        debug!(
            "Listing assets in a folder by path: {} for tenant: {}",
            parent_folder_path, tenant_uuid
        );

        let parent_folder_uuid = self
            .resolve_folder_uuid_by_path(tenant_uuid, parent_folder_path)
            .await?;

        // Now list all assets in this specific folder using the efficient API endpoint with pagination
        let assets = self
            .list_assets_by_parent_folder_uuid(tenant_uuid, parent_folder_uuid.clone().as_ref())
            .await?;
        Ok(assets)
    }

    /// List the assets in a folder *and* in every folder beneath it.
    ///
    /// The Physna contents endpoint only ever returns a folder's direct children, so a
    /// container folder that holds nothing but subfolders reports zero assets. This
    /// method walks the cached folder hierarchy and queries each folder in the subtree,
    /// which is what folder-wide reports (match reports in particular) need.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant
    /// * `parent_folder_path` - The path of the folder to list assets from. The tenant
    ///   root (`/`) means every folder in the tenant, plus any assets sitting at root.
    /// * `on_progress` - Called after each folder in the subtree is queried, with
    ///   `(folders_scanned, folders_total, assets_so_far)`. A deep tree takes one API
    ///   call per folder, so a caller showing a progress display needs this to avoid
    ///   looking hung. Pass `|_, _, _| {}` when there is nothing to report to.
    ///
    /// # Returns
    /// * `Ok(AssetList)` - Every asset in the subtree, de-duplicated by UUID
    /// * `Err(ApiError)` - If the path does not resolve, or an API call fails
    pub async fn list_assets_by_parent_folder_path_recursive<F>(
        &mut self,
        tenant_uuid: &Uuid,
        parent_folder_path: &str,
        mut on_progress: F,
    ) -> Result<AssetList, ApiError>
    where
        F: FnMut(usize, usize, usize),
    {
        debug!(
            "Recursively listing assets in a folder by path: {} for tenant: {}",
            parent_folder_path, tenant_uuid
        );

        // Resolve first so a bad path still fails with FolderNotFound rather than
        // silently returning an empty list.
        let parent_folder_uuid = self
            .resolve_folder_uuid_by_path(tenant_uuid, parent_folder_path)
            .await?;

        // Without the hierarchy there is no way to enumerate descendants. This used to
        // fall back to listing only the folder's direct children, which is a silent
        // lie: the caller asked for a recursive scan and got a non-recursive one, with
        // the explanatory warning drawn over by the scan spinner on the very next
        // redraw. A folder holding one asset and five hundred subfolders reported one
        // asset and looked like it had worked. Fail instead.
        let hierarchy = crate::folder_cache::FolderCache::get_or_fetch(self, tenant_uuid)
            .await
            .map_err(|e| ApiError::FolderHierarchyUnavailable(e.to_string()))?;

        let (subtree_uuids, include_root_level) = match parent_folder_uuid.as_ref() {
            // A resolved folder: that folder plus all of its descendants.
            Some(uuid) => {
                let subtree = hierarchy.subtree_uuids(uuid);
                // The path resolved against the API but the folder is absent from the
                // cached hierarchy, so its descendants cannot be enumerated - a stale
                // cache, most often a folder created since it was written. Same
                // reasoning as above: better an error than a quietly partial report.
                if subtree.is_empty() {
                    return Err(ApiError::FolderHierarchyUnavailable(format!(
                        "'{}' is not present in the cached folder hierarchy",
                        parent_folder_path
                    )));
                }
                (subtree, false)
            }
            // The tenant root has no UUID of its own; it covers every folder, and
            // assets can also live at root level with no parent folder at all.
            None => (hierarchy.all_subtree_uuids(), true),
        };

        let mut assets = AssetList::empty();

        // The root level is an extra query on top of the subtree folders, so it counts
        // towards the total the caller is shown.
        let total = subtree_uuids.len() + usize::from(include_root_level);
        let mut scanned = 0;

        if include_root_level {
            for asset in self
                .list_assets_by_parent_folder_uuid(tenant_uuid, None)
                .await?
                .get_all_assets()
            {
                assets.insert(asset.clone());
            }
            scanned += 1;
            on_progress(scanned, total, assets.len());
        }

        for folder_uuid in &subtree_uuids {
            for asset in self
                .list_assets_by_parent_folder_uuid(tenant_uuid, Some(folder_uuid))
                .await?
                .get_all_assets()
            {
                assets.insert(asset.clone());
            }
            scanned += 1;
            on_progress(scanned, total, assets.len());
        }

        debug!(
            "Found {} assets across {} folder(s) under path: {}",
            assets.len(),
            subtree_uuids.len(),
            parent_folder_path
        );

        Ok(assets)
    }

    /// Get details for a specific asset by ID
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant that owns the asset
    /// * `asset_uuid` - The UUID of the asset to retrieve
    ///
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully fetched asset details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_asset_by_uuid(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
    ) -> Result<Asset, ApiError> {
        debug!(
            "Getting asset details for tenant_uuid: {}, asset_uuid: {}",
            tenant_uuid, asset_uuid
        );
        let url = format!(
            "{}/tenants/{}/assets/{}",
            self.base_url, tenant_uuid, asset_uuid
        );
        let response: SingleAssetResponse = self.get(&url).await?;
        debug!(
            "Successfully retrieved asset details for asset_id: {}",
            asset_uuid
        );
        Ok(response.asset.into())
    }

    pub async fn get_asset_by_path<S: AsRef<str>>(
        &mut self,
        tenant_uuid: &Uuid,
        asset_path: S,
    ) -> Result<Asset, ApiError> {
        let asset_path = asset_path.as_ref();
        let parent_folder_path = Self::get_parent_folder_path(asset_path)?;
        let assets = self
            .list_assets_by_parent_folder_path(tenant_uuid, &parent_folder_path)
            .await?;
        match Self::asset_name_from_path(asset_path) {
            Some(asset_name) => assets
                .find_by_name(&asset_name)
                .cloned()
                .ok_or(ApiError::PathNotFound(asset_path.to_owned())),
            None => Err(ApiError::InvalidAssetPath(asset_path.to_owned())),
        }
    }

    /// Delete an asset by ID
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to delete
    ///
    /// # Returns
    /// * `Ok(())` - Successfully deleted asset
    /// * `Err(ApiError)` - HTTP error or other error
    pub async fn delete_asset(&mut self, tenant_id: &str, asset_id: &str) -> Result<(), ApiError> {
        let path = format!("/tenants/{}/assets/{}", tenant_id, asset_id);
        self.delete(&path).await
    }

    /// Create a new asset by uploading a file
    ///
    /// This method uploads a file as a new asset in the specified tenant.
    /// The file is sent as multipart/form-data with appropriate metadata.
    ///
    /// The method handles automatic token refresh on authentication errors (401/403)
    /// and includes retry logic for handling conflict errors that may occur when
    /// the asset service is temporarily busy.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant where to create the asset
    /// * `file_path` - The local file system path to the file to upload
    /// * `folder_path` - Optional folder path where to place the asset (e.g., "/Home/Folder/Subfolder")
    /// * `folder_id` - Optional folder ID where to place the asset (takes precedence if both path and ID are provided)
    ///
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully created asset details from the API
    /// * `Err(ApiError)` - If there's an HTTP error, IO error, authentication issue, or other API error
    ///   including conflict errors if the asset already exists
    ///
    /// # Example
    /// ```no_run
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// use uuid::Uuid;
    /// use std::path::Path;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let tenant_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let folder_uuid = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let asset = client.create_asset(&tenant_uuid, Path::new("/path/to/file.stl"), &"/Home/MyFolder".to_string(), &folder_uuid).await?;
    ///     println!("Created asset with UUID: {}", asset.uuid());
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_asset(
        &mut self,
        tenant_uuid: &Uuid,
        file_path: &Path,
        asset_path: &String,
        folder_uuid: &Uuid,
    ) -> Result<crate::model::Asset, ApiError> {
        self.create_asset_with_metadata(tenant_uuid, file_path, asset_path, folder_uuid, None)
            .await
    }

    pub async fn create_asset_with_metadata(
        &mut self,
        tenant_uuid: &Uuid,
        file_path: &Path,
        asset_path: &String,
        // Kept for API stability; the upload targets the folder via the full
        // asset path (`createMissingFolders=true`), not a folder id.
        _folder_uuid: &Uuid,
        metadata: Option<&std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<crate::model::Asset, ApiError> {
        trace!("Creating new asset by uploading a file...");

        let url = format!("{}/tenants/{}/assets", self.base_url, tenant_uuid);

        if !file_path.exists() || !file_path.is_file() {
            return Err(ApiError::PathNotFound(
                file_path.to_string_lossy().into_owned(),
            ));
        }

        let file_name = file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(); // It is save to unwrap because we already confired the file exists

        let metadata_json = metadata
            .map(|m| serde_json::to_string(m).unwrap_or_default())
            .unwrap_or_default();

        trace!(
            "Uploading file: {}, with full path: {}",
            file_name,
            asset_path
        );

        let mime_type = mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .to_string();
        let build = |client: &reqwest::Client| -> Result<reqwest::RequestBuilder, ApiError> {
            // Opened afresh for every attempt: a retry after a token renewal or a
            // transient 5xx has to stream the file from the start again.
            let file = std::fs::File::open(file_path).map_err(ApiError::IoError)?;
            let file_part = reqwest::multipart::Part::stream(tokio::fs::File::from_std(file))
                .file_name(file_name.clone())
                .mime_str(&mime_type)
                .map_err(|e| {
                    ApiError::InvalidParameterError(format!(
                        "invalid MIME type '{}' for upload: {}",
                        mime_type, e
                    ))
                })?;
            // Send the full asset path and let the API handle folder creation
            let form = reqwest::multipart::Form::new()
                .part("file", file_part)
                .text("path", asset_path.clone())
                .text("metadata", metadata_json.clone())
                .text("createMissingFolders", "true");
            Ok(client.post(&url).multipart(form))
        };

        debug!("Creating asset with path: {}", asset_path);

        let response = self
            .request_with_auth(build, false)
            .await
            .map_err(map_upload_error)?;
        let text: String = response.text().await?;
        debug!("Raw asset creation response: {}", text);
        parse_created_asset(&text)
    }

    /// Replace the file of an existing asset, keeping its ID, path and metadata.
    ///
    /// `PUT /tenants/{tenantId}/assets/{assetId}/file`: the asset is re-indexed
    /// with the new content and comes back in the `indexing` state. The API
    /// requires the new file's extension to match the asset's path; the caller
    /// checks that before calling, since a mismatch is better refused locally
    /// than reported as a server error after the upload. A 409 is passed on as
    /// the server phrased it (it does not mean "already exists" here).
    pub async fn replace_asset_file(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        file_path: &Path,
    ) -> Result<crate::model::Asset, ApiError> {
        trace!("Replacing the file of asset {}...", asset_uuid);

        let url = format!(
            "{}/tenants/{}/assets/{}/file",
            self.base_url, tenant_uuid, asset_uuid
        );

        if !file_path.exists() || !file_path.is_file() {
            return Err(ApiError::PathNotFound(
                file_path.to_string_lossy().into_owned(),
            ));
        }

        let file_name = file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(); // Safe to unwrap: the file was just confirmed to exist

        let mime_type = mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .to_string();
        let build = |client: &reqwest::Client| -> Result<reqwest::RequestBuilder, ApiError> {
            // Opened afresh for every attempt: a retry after a token renewal or a
            // transient 5xx has to stream the file from the start again.
            let file = std::fs::File::open(file_path).map_err(ApiError::IoError)?;
            let file_part = reqwest::multipart::Part::stream(tokio::fs::File::from_std(file))
                .file_name(file_name.clone())
                .mime_str(&mime_type)
                .map_err(|e| {
                    ApiError::InvalidParameterError(format!(
                        "invalid MIME type '{}' for upload: {}",
                        mime_type, e
                    ))
                })?;
            let form = reqwest::multipart::Form::new().part("file", file_part);
            Ok(client.put(&url).multipart(form))
        };

        debug!(
            "Replacing the file of asset {} with {}",
            asset_uuid, file_name
        );

        let response = self
            .request_with_auth(build, false)
            .await
            .map_err(map_file_error)?;
        let text: String = response.text().await?;
        debug!("Raw asset replacement response: {}", text);
        parse_created_asset(&text)
    }

    /// Create multiple assets by uploading files matching a glob pattern
    ///
    /// This method uploads multiple files as assets in the specified tenant.
    /// Files are matched using a glob pattern and uploaded concurrently.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant where to create the assets
    /// * `glob_pattern` - The glob pattern to match files to upload (e.g., "data/puzzle/*.STL")
    /// * `folder_path` - Optional folder path where to place the assets
    /// * `folder_id` - Optional folder ID where to place the assets
    /// * `concurrent` - Maximum number of concurrent uploads
    /// * `show_progress` - Whether to display a progress bar during upload
    ///
    /// # Returns
    /// * `Ok(Vec<crate::model::AssetResponse>)` - Successfully created assets
    /// * `Err(ApiError)` - HTTP error, IO error, or other error
    pub async fn create_assets_batch(
        &mut self,
        tenant_uuid: &Uuid,
        glob_pattern: &str,
        folder_path: Option<&str>,
        folder_uuid: Option<&Uuid>,
        concurrent: usize,
        show_progress: bool,
    ) -> Result<BatchUploadOutcome, ApiError> {
        // Expand the glob pattern to get matching files
        let paths = expand_upload_paths(glob_pattern)?;

        debug!(
            "Found {} files matching pattern: {}",
            paths.len(),
            glob_pattern
        );

        self.create_assets_from_paths(
            tenant_uuid,
            paths,
            folder_path,
            folder_uuid,
            concurrent,
            show_progress,
        )
        .await
    }

    /// Upload the given files into a folder, `concurrent` at a time.
    ///
    /// The list form of [`create_assets_batch`](Self::create_assets_batch), for
    /// callers that have already decided which files to send - for example after
    /// dropping the ones the folder already holds.
    pub async fn create_assets_from_paths(
        &mut self,
        tenant_uuid: &Uuid,
        paths: Vec<std::path::PathBuf>,
        folder_path: Option<&str>,
        folder_uuid: Option<&Uuid>,
        concurrent: usize,
        show_progress: bool,
    ) -> Result<BatchUploadOutcome, ApiError> {
        debug!(
            "Creating batch assets in tenant: {}, folder_path: {:?}, folder_id: {:?}",
            &tenant_uuid, folder_path, folder_uuid
        );

        // Handle the case where no files match the glob pattern
        if paths.is_empty() {
            return Ok(BatchUploadOutcome::default());
        }

        // Create progress bar if requested
        let progress_bar = if show_progress {
            let pb = ProgressBar::new(paths.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec} {msg}")
                .unwrap()
                .progress_chars("#>-"));
            Some(pb)
        } else {
            None
        };

        // Every task works on a clone of this client. A clone shares the token slot,
        // the renewal lock and the connection pool, so an expiry mid-batch costs one
        // renewal between all tasks instead of one per file. The upload variant
        // carries the upload timeout when one is configured.
        let client_template = self.for_upload_operations();
        let folder_path = folder_path.map(|s| s.to_string());

        debug!(
            "Folder path for batch upload: {:?}, folder ID: {:?}",
            folder_path, folder_uuid
        );

        // Both a folder path and a folder UUID are required by the per-file
        // upload; validate up front instead of unwrapping inside spawned
        // tasks, where a panic would be silently swallowed by the join.
        let folder_path = match folder_path {
            Some(p) => p,
            None => {
                return Err(ApiError::InvalidParameterError(
                    "A folder path is required for batch upload".to_string(),
                ))
            }
        };
        let folder_uuid_required = match folder_uuid {
            Some(u) => *u,
            None => {
                return Err(ApiError::InvalidParameterError(
                    "A folder UUID is required for batch upload".to_string(),
                ))
            }
        };

        // Use a semaphore to control concurrency
        use tokio::sync::Semaphore;
        use tokio_stream::wrappers::ReceiverStream;

        let semaphore = std::sync::Arc::new(Semaphore::new(concurrent));
        // Ensure buffer size is at least 1 to avoid panic with empty channels
        let (tx, rx) = tokio::sync::mpsc::channel::<
            Result<crate::model::Asset, (std::path::PathBuf, ApiError)>,
        >(paths.len().max(1));

        // Process each file with controlled concurrency
        let tasks: Vec<_> = paths
            .into_iter()
            .map(|path_buf| {
                let tx = tx.clone();
                let client_template = client_template.clone();
                let folder_path = folder_path.clone();
                let folder_uuid = folder_uuid_required;
                let progress_bar = progress_bar.clone();
                let tenant_uuid = *tenant_uuid;
                let semaphore = semaphore.clone();

                tokio::spawn(async move {
                    // Acquire a permit to control concurrency
                    let _permit = semaphore.acquire().await.unwrap();

                    let path_str = path_buf.to_string_lossy().to_string();
                    let file_name = path_buf.file_name();

                    let file_name = match file_name {
                        Some(file_name) => file_name,
                        None => {
                            if let Err(e) = tx
                                .send(Err((
                                    path_buf.clone(),
                                    ApiError::PathNotFound(path_buf.to_string_lossy().into()),
                                )))
                                .await
                            {
                                tracing::error!("Failed to send error result: {}", e);
                            }
                            return;
                        }
                    };

                    let mut client = client_template;

                    // Upload the file
                    let asset_path = match folder_path.trim_matches('/') {
                        "" => file_name.to_string_lossy().into_owned(),
                        parent => format!("/{}/{}", parent, file_name.to_string_lossy()),
                    };
                    debug!(
                        "Uploading file: {}, as asset_path: {}, folder_uuid: {:?}",
                        path_str, asset_path, folder_uuid
                    );
                    let result = client
                        .create_asset(&tenant_uuid, &path_buf, &asset_path, &folder_uuid)
                        .await;

                    // Update progress bar if present
                    if let Some(pb) = &progress_bar {
                        pb.inc(1);
                        match &result {
                            Ok(asset) => {
                                pb.set_message(format!("Uploaded: {}", asset.path()));
                            }
                            Err(_) => {
                                pb.set_message(format!(
                                    "Failed: {}",
                                    path_buf.file_name().unwrap_or_default().to_string_lossy()
                                ));
                            }
                        }
                    }

                    // Send result through channel - success or detailed error with file path
                    match result {
                        Ok(asset) => {
                            if let Err(e) = tx.send(Ok(asset)).await {
                                tracing::error!("Failed to send success result: {}", e);
                            }
                        }
                        Err(error) => {
                            if let Err(e) = tx.send(Err((path_buf.clone(), error))).await {
                                tracing::error!("Failed to send error result: {}", e);
                            }
                        }
                    }
                })
            })
            .collect();

        // Wait for all tasks to complete. A panicked task would otherwise be
        // silently dropped from both the success and failure counts.
        for task in tasks {
            if let Err(join_error) = task.await {
                tracing::error!("Batch upload task failed to complete: {}", join_error);
            }
        }

        // Drop the original sender so the receiver knows when all tasks are done
        drop(tx);

        // Collect results from the channel
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut successful_assets = Vec::new();
        let mut failed_files = Vec::new();

        use tokio_stream::StreamExt;
        let mut receiver_stream = ReceiverStream::new(rx);
        while let Some(result) = receiver_stream.next().await {
            match result {
                Ok(asset) => {
                    successful_assets.push(asset);
                    success_count += 1;
                }
                Err((file_path, error)) => {
                    failed_files.push((file_path, error));
                    failure_count += 1;
                }
            }
        }

        // Report summary if progress is shown
        if show_progress {
            if let Some(pb) = &progress_bar {
                pb.finish_with_message(format!(
                    "Batch upload complete: {} successful, {} failed",
                    success_count, failure_count
                ));
            }
        }

        debug!(
            "Batch upload completed: {} successful, {} failed",
            success_count, failure_count
        );

        Ok(BatchUploadOutcome {
            assets: successful_assets,
            failures: failed_files,
        })
    }

    /// Move an asset to another folder, or to the root when `folder_uuid` is `None`.
    ///
    /// `PATCH /tenants/{tenantId}/assets/{assetId}/folder` with `{"folderId": ...}`
    /// (`null` for the root). The asset keeps its UUID; its path changes. Returns
    /// the asset as the server now sees it.
    pub async fn move_asset(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        folder_uuid: Option<Uuid>,
    ) -> Result<Asset, ApiError> {
        let url = format!(
            "{}/tenants/{}/assets/{}/folder",
            self.base_url, tenant_uuid, asset_uuid
        );
        let body = serde_json::json!({ "folderId": folder_uuid.map(|id| id.to_string()) });
        debug!(
            "Moving asset {} to folder {}",
            asset_uuid,
            folder_uuid
                .map(|id| id.to_string())
                .unwrap_or_else(|| "root".to_string())
        );
        let response: crate::model::SingleAssetResponse = self.patch(&url, &body).await?;
        Ok((&response.asset).into())
    }

    /// Which of `paths` already hold an asset, as a set of the strings asked about.
    ///
    /// `POST /tenants/{tenantId}/assets/existing-paths`: a path matches when an
    /// asset with the same file name exists in the same folder (case-sensitive,
    /// with or without a leading slash); paths in folders that do not exist are
    /// never returned. The server echoes each matching path exactly as it was
    /// sent, so the caller tests membership with the string it built. Requests
    /// carry at most 1000 paths (the specification's maximum), so a larger batch
    /// is sent in chunks; an empty batch makes no request.
    pub async fn find_existing_asset_paths(
        &mut self,
        tenant_uuid: &Uuid,
        paths: &[String],
    ) -> Result<std::collections::HashSet<String>, ApiError> {
        const MAX_PATHS_PER_REQUEST: usize = 1000;
        let url = format!(
            "{}/tenants/{}/assets/existing-paths",
            self.base_url, tenant_uuid
        );
        let mut existing = std::collections::HashSet::new();
        for chunk in paths.chunks(MAX_PATHS_PER_REQUEST) {
            debug!(
                "Checking {} path(s) for existing assets in tenant {}",
                chunk.len(),
                tenant_uuid
            );
            let response: crate::model::ExistingPathsResponse = self
                .post_query(&url, &serde_json::json!({ "paths": chunk }))
                .await?;
            existing.extend(response.existing_paths);
        }
        Ok(existing)
    }

    /// The assets with the given UUIDs, in the order they were asked for.
    ///
    /// `POST /tenants/{tenantId}/assets/batch`, at most 1000 ids per request
    /// (the specification's maximum), so a longer list goes out in chunks.
    /// Duplicate ids are asked about once. An id the tenant does not have is
    /// simply absent from the result: use [`missing_asset_ids`] to find out
    /// which. An empty list makes no request.
    pub async fn get_assets_batch(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuids: &[Uuid],
    ) -> Result<Vec<Asset>, ApiError> {
        const MAX_IDS_PER_REQUEST: usize = 1000;
        let mut wanted: Vec<Uuid> = Vec::with_capacity(asset_uuids.len());
        for id in asset_uuids {
            if !wanted.contains(id) {
                wanted.push(*id);
            }
        }
        let url = format!("{}/tenants/{}/assets/batch", self.base_url, tenant_uuid);
        let mut found: std::collections::HashMap<Uuid, Asset> =
            std::collections::HashMap::with_capacity(wanted.len());
        for chunk in wanted.chunks(MAX_IDS_PER_REQUEST) {
            debug!(
                "Fetching {} asset(s) by id from tenant {}",
                chunk.len(),
                tenant_uuid
            );
            let response: crate::model::AssetListResponse = self
                .post_query(&url, &serde_json::json!({ "assetIds": chunk }))
                .await?;
            for asset in &response.assets {
                let asset: Asset = asset.into();
                found.insert(asset.uuid(), asset);
            }
        }
        Ok(wanted.iter().filter_map(|id| found.remove(id)).collect())
    }

    /// Reprocess a single asset by its UUID
    ///
    /// This method triggers reprocessing of a specific asset in the Physna system
    /// via `POST /tenants/{tenantId}/assets/{assetId}/reprocess`. The asset's
    /// state is set to `indexing` and a reprocess event is emitted; assets can be
    /// reprocessed regardless of their current state.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant that owns the asset
    /// * `asset_uuid` - The UUID of the asset to reprocess
    ///
    /// # Returns
    /// * `Ok(())` - Successfully triggered reprocessing
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn reprocess_asset(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/tenants/{}/assets/{}/reprocess",
            self.base_url, tenant_uuid, asset_uuid
        );

        // POST with no body; the API answers 204 No Content.
        self.execute_request_no_response(|client| Ok(client.post(&url)), false)
            .await
    }

    /// List assets by state from the Physna API
    ///
    /// This function retrieves a list of assets in a specific state (processing, ready, failed, deleted) for a specific tenant.
    /// The endpoint supports pagination to handle large numbers of assets.
    ///
    /// # Arguments
    ///
    /// * `tenant_uuid` - The UUID of the tenant to get assets for
    /// * `state` - The state to filter assets by (indexing, finished, failed, unsupported, no-3d-data, missing-dependencies)
    ///
    /// # Returns
    ///
    /// * `Ok(AssetList)` - Successfully fetched list of assets in the specified state
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_assets_by_state(
        &mut self,
        tenant_uuid: &Uuid,
        state: &str,
    ) -> Result<AssetList, ApiError> {
        debug!(
            "Getting assets by state for tenant_uuid: {}, state: {}",
            tenant_uuid, state
        );

        // Validate the state parameter
        match state {
            "indexing" | "finished" | "failed" | "unsupported" | "no-3d-data" | "missing-dependencies" => {},
            _ => return Err(ApiError::InvalidParameterError(format!("Invalid state: {}. Valid states are: indexing, finished, failed, unsupported, no-3d-data, missing-dependencies", state))),
        }

        let per_page: usize = 1000; // the API maximum for this endpoint
        let mut all_assets: Vec<Asset> = Vec::new();
        let mut pager = crate::paging::Pager::new("asset listing by state");

        // Loop through all pages to get all assets with the specified state
        loop {
            let page = pager.page();
            let url = format!(
                "{}/tenants/{}/assets/state/{}?page={}&perPage={}",
                self.base_url, tenant_uuid, state, page, per_page
            );
            debug!("Assets by state request URL: {}", url);

            // Execute the GET request using the generic method
            let response: AssetListResponse = match self.get(&url).await {
                Ok(response) => response,
                Err(e) => {
                    // If we get a 404, it might mean there are no assets with that state
                    match &e {
                        ApiError::NotFoundError(_) => {
                            debug!(
                                "No assets found with state: {} for tenant: {}",
                                state, tenant_uuid
                            );
                            // Return an empty response instead of an error
                            AssetListResponse {
                                assets: vec![],
                                page_data: crate::model::PageData {
                                    current_page: page,
                                    per_page,
                                    total: 0,
                                    last_page: 1,
                                    start_index: 0,
                                    end_index: 0,
                                },
                            }
                        }
                        _ => return Err(e),
                    }
                }
            };

            // Extract assets from the current page
            let current_page_assets: Vec<Asset> =
                response.assets.iter().map(|a| a.into()).collect();
            all_assets.extend(current_page_assets);

            if !pager.advance(
                response.page_data.current_page,
                response.page_data.last_page,
                all_assets.len(),
            ) {
                break;
            }
        }

        debug!(
            "Successfully retrieved {} assets by state: {} for tenant_uuid: {}",
            all_assets.len(),
            state,
            tenant_uuid
        );

        // Convert the collected assets to an AssetList
        Ok(all_assets.into())
    }

    /// List all assets in a tenant using the GET /tenants/{tenantId}/assets endpoint.
    ///
    /// Handles pagination automatically, fetching all pages.
    pub async fn list_all_tenant_assets(
        &mut self,
        tenant_uuid: &Uuid,
    ) -> Result<AssetList, ApiError> {
        debug!("Listing all assets for tenant_uuid: {}", tenant_uuid);

        let per_page: usize = 1000; // the API maximum for this endpoint
        let mut all_assets: Vec<Asset> = Vec::new();
        // Up to ten million assets. Past that it used to stop with only a debug
        // line; the partial inventory now says it is partial.
        let mut pager = crate::paging::Pager::with_max_pages("tenant asset listing", 10_000);

        loop {
            let page = pager.page();
            let url = format!(
                "{}/tenants/{}/assets?page={}&perPage={}",
                self.base_url, tenant_uuid, page, per_page
            );
            debug!("List all tenant assets URL: {}", url);

            let response: AssetListResponse = self.get(&url).await?;
            let current_page_assets: Vec<Asset> =
                response.assets.iter().map(|a| a.into()).collect();
            all_assets.extend(current_page_assets);

            if !pager.advance(
                response.page_data.current_page,
                response.page_data.last_page,
                all_assets.len(),
            ) {
                break;
            }
        }

        debug!(
            "Successfully retrieved {} total assets for tenant_uuid: {}",
            all_assets.len(),
            tenant_uuid
        );

        Ok(all_assets.into())
    }

    /// Get asset dependencies by path without building a tree structure
    ///
    /// This method returns the raw dependencies response from the API, which includes
    /// both existing assets and missing dependencies (assets that are referenced but not present in Physna)
    /// Get asset state counts from the Physna API
    ///
    /// This function retrieves the count of assets in each state (processing, ready, failed, deleted) for a specific tenant.
    ///
    /// # Arguments
    ///
    /// * `tenant_uuid` - The UUID of the tenant to get asset state counts for
    ///
    /// # Returns
    ///
    /// * `Ok(AssetStateCounts)` - Successfully fetched asset state counts
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_asset_state_counts(
        &mut self,
        tenant_uuid: &Uuid,
    ) -> Result<AssetStateCounts, ApiError> {
        debug!(
            "Getting asset state counts for tenant_uuid: {}",
            tenant_uuid
        );

        let url = format!("{}/tenants/{}/assets/state", self.base_url, tenant_uuid);
        debug!("Asset state counts request URL: {}", url);

        // Execute the GET request using the generic method
        let response: crate::model::AssetStateCounts = self.get(&url).await?;
        debug!(
            "Successfully retrieved asset state counts for tenant_uuid: {}",
            tenant_uuid
        );

        Ok(response)
    }

    /// Link a missing dependency of an assembly to an existing asset.
    ///
    /// `POST /tenants/{tenantId}/assets/{assetId}/resolve-dependency` with
    /// `{"resolvedAssetId", "dependencyPath"}`; the assembly is re-indexed with
    /// the resolved dependency. `dependency_path` is the path string the
    /// dependency listing reports for the missing part. Answers 204.
    pub async fn resolve_asset_dependency(
        &mut self,
        tenant_uuid: &Uuid,
        assembly_uuid: &Uuid,
        dependency_path: &str,
        resolved_asset_uuid: &Uuid,
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/tenants/{}/assets/{}/resolve-dependency",
            self.base_url, tenant_uuid, assembly_uuid
        );
        let body = serde_json::json!({
            "resolvedAssetId": resolved_asset_uuid.to_string(),
            "dependencyPath": dependency_path,
        });
        debug!(
            "Resolving dependency '{}' of assembly {} with asset {}",
            dependency_path, assembly_uuid, resolved_asset_uuid
        );
        self.post_no_response(&url, &body).await
    }

    /// Public method to get asset dependencies list by UUID
    ///
    /// This method returns the raw dependencies response instead of building
    /// an assembly tree. All pages are accumulated: an asset with more direct
    /// dependencies than one page returns the complete list.
    pub async fn get_asset_dependencies_list_by_uuid(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
    ) -> Result<AssetDependenciesResponse, ApiError> {
        let mut page: usize = 1;
        let per_page: usize = 1000; // the API maximum for this endpoint
        let mut all_dependencies = Vec::new();

        loop {
            let response = self
                .get_asset_dependencies_by_uuid_with_pagination(
                    tenant_uuid,
                    asset_uuid,
                    page,
                    per_page,
                )
                .await?;

            let current_page = response.page_data.current_page;
            let last_page = response.page_data.last_page;
            all_dependencies.extend(response.dependencies);

            // Stop on the last page, or if the endpoint fails to advance
            // `current_page` (guard against looping forever on one page).
            if current_page >= last_page || current_page < page {
                break;
            }
            page = current_page + 1;
        }

        let total = all_dependencies.len();
        Ok(AssetDependenciesResponse {
            dependencies: all_dependencies,
            page_data: crate::model::PageData {
                total,
                per_page,
                current_page: 1,
                last_page: 1,
                start_index: 0,
                end_index: total,
            },
            original_asset_path: String::new(),
        })
    }

    /// Get asset dependencies by UUID
    ///
    /// This method retrieves the dependencies of an asset using its UUID directly,
    /// which is more efficient than resolving the path to UUID first.
    ///
    /// # Arguments
    ///
    /// * `tenant_uuid` - The UUID of the tenant that owns the asset
    /// * `asset_uuid` - The UUID of the asset to get dependencies for
    ///
    /// # Returns
    ///
    /// * `Ok(AssemblyTree)` - Successfully built assembly tree with dependencies
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_asset_dependencies_by_uuid(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
    ) -> Result<AssemblyTree, ApiError> {
        let asset = self.get_asset_by_uuid(tenant_uuid, asset_uuid).await?;

        let mut tree = AssemblyTree::new(asset);
        Box::pin(self.populate_asset_dependencies_recursive_by_uuid(
            tenant_uuid,
            tree.root_mut(),
            asset_uuid,
            &mut std::collections::HashSet::new(),
        ))
        .await?;
        Ok(tree)
    }
}

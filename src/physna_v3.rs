use crate::auth::AuthClient;
use crate::model::{CurrentUserResponse, FolderListResponse};
use reqwest;
use serde_json;
use tracing::{debug, trace, error};
use glob::glob;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};

/// Error emitted by the Physna V3 Api
/// 
/// This enum represents all possible errors that can occur when interacting with the Physna V3 API.
/// It includes HTTP errors, JSON parsing errors, authentication errors, and retry failures.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// HTTP request error from the reqwest crate
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    
    /// JSON parsing error from serde_json
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// IO error from std::io operations
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    /// Authentication error with a descriptive message
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    /// Request failed after retry attempts with a descriptive message
    #[error("Request failed after retry: {0}")]
    RetryFailed(String),
    
    /// Glob pattern error
    #[error("Glob pattern error: {0}")]
    GlobError(#[from] glob::GlobError),
    
    /// Glob pattern error for path matching
    #[error("Glob pattern path error: {0}")]
    GlobPatternError(#[from] glob::PatternError),
    
    /// Conflict error (e.g., asset already exists)
    #[error("Conflict: {0}")]
    ConflictError(String),
}

/// Physna V3 API client
/// 
/// This client provides methods to interact with the Physna V3 REST API.
/// It handles authentication, automatic token refresh, and common HTTP operations.
/// 
/// The client supports:
/// - Automatic access token management with refresh on expiration
/// - Client credentials for token refresh
/// - Common HTTP operations (GET, POST, PUT, DELETE)
/// - Automatic retry on authentication failures
#[derive(Clone)]
pub struct PhysnaApiClient {
    /// Base URL for the Physna V3 API (e.g., "https://app-api.physna.com/v3")
    base_url: String,
    
    /// Current access token for API authentication
    access_token: Option<String>,
    
    /// Client credentials (client_id, client_secret) for token refresh
    client_credentials: Option<(String, String)>, // (client_id, client_secret)
    
    /// HTTP client for making requests
    http_client: reqwest::Client,
}

impl PhysnaApiClient {
    /// Create a new Physna API client with default configuration
    /// 
    /// The client is initialized with:
    /// - Default base URL: "https://app-api.physna.com/v3"
    /// - No access token (must be set with `with_access_token`)
    /// - No client credentials (must be set with `with_client_credentials`)
    /// - Default HTTP client
    /// 
    /// # Returns
    /// A new `PhysnaApiClient` instance
    pub fn new() -> Self {
        Self {
            base_url: "https://app-api.physna.com/v3".to_string(),
            access_token: None,
            client_credentials: None,
            http_client: reqwest::Client::new(),
        }
    }
    
    /// Set the base URL for the API client
    /// 
    /// # Arguments
    /// * `base_url` - The base URL for the Physna V3 API (e.g., "https://app-api.physna.com/v3")
    /// 
    /// # Returns
    /// The updated `PhysnaApiClient` instance with the new base URL
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Set the access token for API authentication
    /// 
    /// # Arguments
    /// * `token` - The access token to use for API requests
    /// 
    /// # Returns
    /// The updated `PhysnaApiClient` instance with the access token set
    pub fn with_access_token(mut self, token: String) -> Self {
        self.access_token = Some(token);
        self
    }
    
    /// Set the client credentials for automatic token refresh
    /// 
    /// # Arguments
    /// * `client_id` - The client ID for authentication
    /// * `client_secret` - The client secret for authentication
    /// 
    /// # Returns
    /// The updated `PhysnaApiClient` instance with client credentials set
    pub fn with_client_credentials(mut self, client_id: String, client_secret: String) -> Self {
        self.client_credentials = Some((client_id, client_secret));
        self
    }
    
    /// Attempt to refresh the access token using client credentials
    /// 
    /// This method tries to obtain a new access token using the stored client credentials.
    /// It's called automatically when API requests fail with authentication errors (401/403).
    /// 
    /// # Returns
    /// * `Ok(())` - Token successfully refreshed
    /// * `Err(ApiError::AuthError)` - Failed to refresh token or no credentials available
    async fn refresh_token(&mut self) -> Result<(), ApiError> {
        // Check if we have client credentials available for token refresh
        if let Some((client_id, client_secret)) = &self.client_credentials {
            trace!("Refreshing access token");
            
            // Create a new auth client with the stored credentials
            let auth_client = AuthClient::new(client_id.clone(), client_secret.clone());
            
            // Attempt to get a new access token
            match auth_client.get_access_token().await {
                Ok(new_token) => {
                    debug!("Successfully refreshed access token");
                    // Update the stored access token
                    self.access_token = Some(new_token);
                    Ok(())
                }
                Err(e) => {
                    // Return an authentication error with details
                    Err(ApiError::AuthError(format!("Failed to refresh token: {}", e)))
                }
            }
        } else {
            // No client credentials available for token refresh
            Err(ApiError::AuthError("No client credentials available for token refresh".to_string()))
        }
    }
    
    /// Generic method to build and execute HTTP requests with automatic token refresh on 401/403 errors
    /// 
    /// This method provides a unified interface for making HTTP requests to the Physna V3 API.
    /// It automatically handles:
    /// - Adding access tokens to authenticated requests
    /// - Detecting authentication failures (401/403)
    /// - Refreshing expired tokens using client credentials
    /// - Retrying failed requests with refreshed tokens
    /// 
    /// # Type Parameters
    /// * `T` - The type to deserialize the response into (must implement `DeserializeOwned`)
    /// * `F` - A closure that builds the HTTP request
    /// 
    /// # Arguments
    /// * `request_builder` - A closure that takes a `reqwest::Client` and returns a `RequestBuilder`
    /// 
    /// # Returns
    /// * `Ok(T)` - Successfully executed request with parsed response
    /// * `Err(ApiError)` - HTTP error, JSON parsing error, or authentication failure
    async fn execute_request<T, F>(&mut self, request_builder: F) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        // Build and execute the initial request
        let mut request = request_builder(&self.http_client);
        
        // Add access token header if available for authentication
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;

        // Check if we need to retry due to authentication issues (401 Unauthorized or 403 Forbidden)
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || 
           response.status() == reqwest::StatusCode::FORBIDDEN {
            debug!("Received authentication error ({}), attempting token refresh", response.status());
            
            // Try to refresh the expired or invalid access token
            self.refresh_token().await?;
            
            // Retry the original request with the newly refreshed token
            debug!("Retrying request with refreshed token");
            let mut retry_request = request_builder(&self.http_client);
            
            // Add the refreshed access token to the retry request
            if let Some(token) = &self.access_token {
                retry_request = retry_request.header("Authorization", format!("Bearer {}", token));
            }
            
            let retry_response = retry_request.send().await?;
            
            // Check if the retry was successful
            if retry_response.status().is_success() {
                // Parse and return the JSON response
                let result: T = retry_response.json().await?;
                Ok(result)
            } else {
                // Retry failed - return detailed error information
                Err(ApiError::RetryFailed(format!(
                    "Original error: {}, Retry failed with status: {}", 
                    response.status(), 
                    retry_response.status()
                )))
            }
        } else if response.status().is_success() {
            // Initial request was successful - parse and return the JSON response
            let result: T = response.json().await?;
            Ok(result)
        } else {
            // Other HTTP error - return the error status
            Err(ApiError::HttpError(response.error_for_status().unwrap_err()))
        }
    }
    
    /// Generic method to build and execute GET requests
    async fn get<T>(&mut self, url: &str) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.execute_request(|client| client.get(url)).await
    }
    
    /// Generic method to build and execute POST requests
    async fn post<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(|client| client.post(url).json(body)).await
    }
    
    /// Generic method to build and execute PUT requests
    async fn put<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(|client| client.put(url).json(body)).await
    }
    
    /// Generic method to build and execute DELETE requests with automatic token refresh
    async fn delete(&mut self, url: &str) -> Result<(), ApiError> {
        // For DELETE requests, we build and execute the request directly
        // without trying to parse JSON from the response
        let mut request = self.http_client.delete(url);
        
        // Add access token if available
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;
        
        // Check if we need to retry due to authentication issues
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || 
           response.status() == reqwest::StatusCode::FORBIDDEN {
            debug!("Received authentication error ({}), attempting token refresh", response.status());
            
            // Try to refresh the token
            self.refresh_token().await?;
            
            // Retry the request with the new token
            debug!("Retrying DELETE request with refreshed token");
            let mut retry_request = self.http_client.delete(url);
            
            if let Some(token) = &self.access_token {
                retry_request = retry_request.header("Authorization", format!("Bearer {}", token));
            }
            
            let retry_response = retry_request.send().await?;
            
            if retry_response.status().is_success() {
                Ok(())
            } else {
                Err(ApiError::RetryFailed(format!(
                    "Original error: {}, Retry failed with status: {}", 
                    response.status(), 
                    retry_response.status()
                )))
            }
        } else if response.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::HttpError(response.error_for_status().unwrap_err()))
        }
    }

    /// Get the current user's information from the Physna V3 API
    /// 
    /// This method fetches information about the currently authenticated user,
    /// including their tenant settings and other user-specific configuration.
    /// The response contains the user's profile information and available tenants.
    /// 
    /// # Returns
    /// * `Ok(CurrentUserResponse)` - Successfully fetched current user information
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_current_user(&mut self) -> Result<CurrentUserResponse, ApiError> {
        let url = format!("{}/users/me", self.base_url);
        self.get(&url).await
    }

    /// List all available tenants for the current user
    /// 
    /// This method fetches all tenants available to the currently authenticated user.
    /// Tenants represent different organizations or environments that the user has access to.
    /// Each tenant has its own set of folders, assets, and configurations.
    /// 
    /// # Returns
    /// * `Ok(Vec<TenantSetting>)` - Successfully fetched list of available tenants
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn list_tenants(&mut self) -> Result<Vec<crate::model::TenantSetting>, ApiError> {
        let user = self.get_current_user().await?;
        Ok(user.user.settings)
    }
    
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
    pub async fn list_folders(&mut self, tenant_id: &str, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_id);
        
        // Build query parameters for pagination if provided
        let mut query_params = Vec::new();
        if let Some(page) = page {
            query_params.push(("page", page.to_string()));
        }
        if let Some(per_page) = per_page {
            query_params.push(("per_page", per_page.to_string()));
        }
        
        // Add query parameters to URL if provided
        let url = if !query_params.is_empty() {
            format!("{}?{}", url, serde_urlencoded::to_string(query_params).unwrap())
        } else {
            url
        };
        
        // Execute GET request to fetch folders
        self.get(&url).await
    }
    
    /// Get details for a specific folder by ID
    /// 
    /// This method fetches detailed information about a specific folder by its ID.
    /// The response includes folder metadata such as name, creation date, asset count, etc.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the folder
    /// * `folder_id` - The UUID of the folder to retrieve
    /// 
    /// # Returns
    /// * `Ok(FolderResponse)` - Successfully fetched folder details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_folder(&mut self, tenant_id: &str, folder_id: &str) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_id, folder_id);
        self.get(&url).await
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
    pub async fn create_folder(&mut self, tenant_id: &str, name: &str, parent_folder_id: Option<&str>) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_id);
        
        // Build request body with folder name
        let mut body = serde_json::json!({
            "name": name
        });
        
        // Add parent folder ID if provided to create a subfolder
        if let Some(parent_id) = parent_folder_id {
            body["parentFolderId"] = serde_json::Value::String(parent_id.to_string());
        }
        
        // Execute POST request to create the folder
        self.post(&url, &body).await
    }
    
    pub async fn update_folder(&mut self, tenant_id: &str, folder_id: &str, name: &str) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_id, folder_id);
        
        let body = serde_json::json!({
            "name": name
        });
        
        // The API returns a SingleFolderResponse with a "folder" field
        let response: crate::model::SingleFolderResponse = self.put(&url, &body).await?;
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
    pub async fn delete_folder(&mut self, tenant_id: &str, folder_id: &str) -> Result<(), ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_id, folder_id);
        self.delete(&url).await
    }
    
    // Asset operations
    
    /// List all assets for a tenant with optional pagination and search
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant whose assets to list
    /// * `folder_id` - Optional folder ID to filter assets within a specific folder
    /// * `page` - Optional page number (1-based indexing)
    /// * `per_page` - Optional number of items per page (default: 100)
    /// 
    /// # Returns
    /// * `Ok(AssetListResponse)` - Successfully fetched list of assets with pagination metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn list_assets(&mut self, tenant_id: &str, folder_id: Option<String>, page: Option<u32>, per_page: Option<u32>) -> Result<crate::model::AssetListResponse, ApiError> {
        let url = format!("{}/tenants/{}/assets", self.base_url, tenant_id);
        
        // Build query parameters for pagination and filtering if provided
        let mut query_params = Vec::new();
        if let Some(folder_id) = folder_id {
            query_params.push(("folderId", folder_id));
        }
        if let Some(page) = page {
            query_params.push(("page", page.to_string()));
        }
        if let Some(per_page) = per_page {
            query_params.push(("per_page", per_page.to_string()));
        }
        
        // Add query parameters to URL if provided
        let url = if !query_params.is_empty() {
            format!("{}?{}", url, serde_urlencoded::to_string(query_params).unwrap())
        } else {
            url
        };
        
        self.get(&url).await
    }
    
    /// Get details for a specific asset by ID
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to retrieve
    /// 
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully fetched asset details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_asset(&mut self, tenant_id: &str, asset_id: &str) -> Result<crate::model::AssetResponse, ApiError> {
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_id, asset_id);
        let response: crate::model::SingleAssetResponse = self.get(&url).await?;
        Ok(response.asset)
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
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_id, asset_id);
        self.delete(&url).await
    }
    
    /// Update an asset's metadata
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to update
    /// * `name` - The new name for the asset
    /// 
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully updated asset with new metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn update_asset(&mut self, tenant_id: &str, asset_id: &str, name: &str) -> Result<crate::model::AssetResponse, ApiError> {
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_id, asset_id);
        
        let body = serde_json::json!({
            "name": name
        });
        
        self.put(&url, &body).await
    }
    
    /// Create a new asset by uploading a file
    /// 
    /// This method uploads a file as a new asset in the specified tenant.
    /// The file is sent as multipart/form-data with the file content.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant where to create the asset
    /// * `file_path` - The path to the file to upload
    /// * `folder_path` - Optional folder path where to place the asset
    /// * `folder_id` - Optional folder ID where to place the asset
    /// 
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully created asset details
    /// * `Err(ApiError)` - HTTP error, IO error, or other error
    pub async fn create_asset(&mut self, tenant_id: &str, file_path: &str, folder_path: Option<&str>, folder_id: Option<&str>) -> Result<crate::model::AssetResponse, ApiError> {
        let url = format!("{}/tenants/{}/assets", self.base_url, tenant_id);
        
        // Read the file content
        let file_data = tokio::fs::read(file_path).await?;
        
        // Extract filename from path
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .ok_or_else(|| ApiError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid file path"
            )))?
            .to_str()
            .ok_or_else(|| ApiError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid file name"
            )))?
            .to_string();
            
        // Create the asset path for the multipart form (folder path + filename or just filename)
        let asset_path = if let Some(folder_path) = folder_path {
            if !folder_path.is_empty() {
                format!("{}/{}", folder_path, file_name)
            } else {
                file_name.clone()
            }
        } else {
            file_name.clone()
        };
            
        // Create a file part from the file data
        let file_part = reqwest::multipart::Part::bytes(file_data)
            .file_name(file_name.clone());
        
        // Build the multipart form with file part and required parameters
        let mut form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("path", asset_path.clone())  // Use the full asset path including folder
            .text("metadata", "")  // Empty metadata as in the working example
            .text("createMissingFolders", "");  // Empty createMissingFolders as in the working example
        
        // Add folder ID if provided
        if let Some(folder_id) = folder_id {
            debug!("Adding folderId parameter: {}", folder_id);
            // For multipart forms, we need to add non-file parts as text
            form = form.text("folderId", folder_id.to_string());
        }
        
        debug!("Creating asset with path: {}", asset_path);
        
        // Build and execute the request with multipart form data
        let mut request = self.http_client.post(&url)
            .multipart(form);
        
        // Add access token if available
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;
        
        // Check if we need to retry due to authentication issues
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || 
           response.status() == reqwest::StatusCode::FORBIDDEN {
            debug!("Received authentication error ({}), attempting token refresh", response.status());
            
            // Try to refresh the token
            self.refresh_token().await?;
            
            // Create a new form for the retry
            let file_data = tokio::fs::read(file_path).await?;
            let file_part = reqwest::multipart::Part::bytes(file_data)
                .file_name(file_name.clone());
            
            // Create the asset path for the multipart form (folder path + filename or just filename)
            let asset_path = if let Some(folder_path) = folder_path {
                if !folder_path.is_empty() {
                    format!("{}/{}", folder_path, file_name)
                } else {
                    file_name.clone()
                }
            } else {
                file_name.clone()
            };
            
            // Build the multipart form with file part and required parameters
            let mut retry_form = reqwest::multipart::Form::new()
                .part("file", file_part)
                .text("path", asset_path.clone())  // Use the full asset path including folder
                .text("metadata", "")  // Empty metadata as in the working example
                .text("createMissingFolders", "");  // Empty createMissingFolders as in the working example
            
            // Add folder ID if provided
            if let Some(folder_id) = folder_id {
                debug!("Adding folderId parameter (retry): {}", folder_id);
                retry_form = retry_form.text("folderId", folder_id.to_string());
            }
            
            debug!("Retrying asset creation with path: {}", asset_path);
            
            // Retry the request with the new token
            debug!("Retrying asset creation request with refreshed token");
            let mut retry_request = self.http_client.post(&url)
                .multipart(retry_form);
            
            if let Some(token) = &self.access_token {
                retry_request = retry_request.header("Authorization", format!("Bearer {}", token));
            }
            
            let retry_response = retry_request.send().await?;
            
            if retry_response.status().is_success() {
                // Try to get the raw response text for debugging
                let text = retry_response.text().await?;
                debug!("Raw asset creation retry response: {}", text);
                
                // Try to parse as SingleAssetResponse
                match serde_json::from_str::<crate::model::SingleAssetResponse>(&text) {
                    Ok(result) => Ok(result.asset),
                    Err(_) => {
                        // Try to parse as AssetResponse directly
                        match serde_json::from_str::<crate::model::AssetResponse>(&text) {
                            Ok(asset) => Ok(asset),
                            Err(e) => {
                                error!("Failed to parse retry response as either SingleAssetResponse or AssetResponse: {}", e);
                                Err(ApiError::JsonError(e))
                            }
                        }
                    }
                }
            } else {
                // Handle specific HTTP error codes with user-friendly messages
                let status = retry_response.status();
                match status {
                    reqwest::StatusCode::CONFLICT => {
                        Err(ApiError::ConflictError("Asset already exists. Please use a different filename or delete the existing asset first.".to_string()))
                    }
                    reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
                        Err(ApiError::ConflictError("Invalid request data. Please check your input and try again.".to_string()))
                    }
                    reqwest::StatusCode::PAYLOAD_TOO_LARGE => {
                        Err(ApiError::ConflictError("File is too large. Please check the file size limits and try again.".to_string()))
                    }
                    _ => {
                        Err(ApiError::RetryFailed(format!(
                            "Original error: {}, Retry failed with status: {}", 
                            response.status(), 
                            retry_response.status()
                        )))
                    }
                }
            }
        } else if response.status().is_success() {
            // Try to get the raw response text for debugging
            let text = response.text().await?;
            debug!("Raw asset creation response: {}", text);
            
            // Try to parse as SingleAssetResponse
            match serde_json::from_str::<crate::model::SingleAssetResponse>(&text) {
                Ok(result) => Ok(result.asset),
                Err(_) => {
                    // Try to parse as AssetResponse directly
                    match serde_json::from_str::<crate::model::AssetResponse>(&text) {
                        Ok(asset) => Ok(asset),
                        Err(e) => {
                            error!("Failed to parse response as either SingleAssetResponse or AssetResponse: {}", e);
                            Err(ApiError::JsonError(e))
                        }
                    }
                }
            }
        } else {
            // Handle specific HTTP error codes with user-friendly messages
            let status = response.status();
            match status {
                reqwest::StatusCode::CONFLICT => {
                    Err(ApiError::ConflictError("Asset already exists. Please use a different filename or delete the existing asset first.".to_string()))
                }
                reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
                    Err(ApiError::ConflictError("Invalid request data. Please check your input and try again.".to_string()))
                }
                reqwest::StatusCode::PAYLOAD_TOO_LARGE => {
                    Err(ApiError::ConflictError("File is too large. Please check the file size limits and try again.".to_string()))
                }
                _ => {
                    Err(ApiError::HttpError(response.error_for_status().unwrap_err()))
                }
            }
        }
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
        tenant_id: &str, 
        glob_pattern: &str, 
        folder_path: Option<&str>,
        folder_id: Option<&str>,
        concurrent: usize,
        show_progress: bool
    ) -> Result<Vec<crate::model::AssetResponse>, ApiError> {
        debug!("Creating batch assets in tenant: {}, folder_path: {:?}, folder_id: {:?}", tenant_id, folder_path, folder_id);
        
        // Expand the glob pattern to get matching files
        let paths: Vec<_> = glob(glob_pattern)?
            .filter_map(|path_result| path_result.ok()) // Filter out any errors and extract the PathBuf
            .collect();
        
        debug!("Found {} files matching pattern: {}", paths.len(), glob_pattern);
        
        // Create progress bar if requested
        let progress_bar = if show_progress {
            let pb = ProgressBar::new(paths.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"));
            Some(pb)
        } else {
            None
        };
        
        // Create a stream of futures for uploading files concurrently
        let base_url = self.base_url.clone();
        let access_token = self.access_token.clone();
        let client_credentials = self.client_credentials.clone();
        let tenant_id = tenant_id.to_string();
        let folder_path = folder_path.map(|s| s.to_string());
        let folder_id = folder_id.map(|s| s.to_string());
        
        debug!("Folder path for batch upload: {:?}, folder ID: {:?}", folder_path, folder_id);
        
        let results: Result<Vec<_>, _> = stream::iter(paths)
            .map(|path_buf| {
                let base_url = base_url.clone();
                let access_token = access_token.clone();
                let client_credentials = client_credentials.clone();
                let tenant_id = tenant_id.clone();
                let folder_path = folder_path.clone();
                let folder_id = folder_id.clone();
                let progress_bar = progress_bar.clone();
                
                async move {
                    let path_str = path_buf.to_string_lossy().to_string();
                    debug!("Uploading file: {}, with folder_path: {:?}, folder_id: {:?}", path_str, folder_path, folder_id);
                    
                    // Create a new client for each request to avoid borrowing issues
                    let mut client = PhysnaApiClient::new().with_base_url(base_url);
                    if let Some(token) = access_token {
                        client = client.with_access_token(token);
                    }
                    if let Some((client_id, client_secret)) = client_credentials {
                        client = client.with_client_credentials(client_id, client_secret);
                    }
                    
                    // Upload the file
                    let result = client.create_asset(&tenant_id, &path_str, folder_path.as_deref(), folder_id.as_deref()).await;
                    
                    // Update progress bar if present
                    if let Some(pb) = &progress_bar {
                        pb.inc(1);
                        match &result {
                            Ok(asset) => {
                                pb.set_message(format!("Uploaded: {}", asset.path));
                            }
                            Err(_) => {
                                pb.set_message(format!("Failed: {}", path_buf.file_name().unwrap_or_default().to_string_lossy()));
                            }
                        }
                    }
                    
                    result
                }
            })
            .buffer_unordered(concurrent)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect();
        
        // Finish progress bar if present
        if let Some(pb) = progress_bar {
            pb.finish_with_message("Batch upload complete");
        }
        
        results
    }
}

impl Default for PhysnaApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_asset_url() {
        let client = PhysnaApiClient::new();
        // This test verifies that the URL is constructed correctly
        // We're not actually making a network request in this test
        let tenant_id = "test-tenant";
        let url = format!("{}/tenants/{}/assets", client.base_url, tenant_id);
        assert_eq!(url, "https://app-api.physna.com/v3/tenants/test-tenant/assets");
    }
}
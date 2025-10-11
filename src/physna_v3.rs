use crate::auth::AuthClient;
use crate::model::{CurrentUserResponse, FolderListResponse};
use reqwest;
use serde_json;
use serde_urlencoded;
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
/// - Common HTTP operations (GET, POST, PUT, DELETE, PATCH)
/// - Automatic retry on authentication failures (401/403)
/// - Batch operations for efficient processing of multiple resources
/// - Comprehensive error handling with detailed error types
/// 
/// Usage example:
/// ```no_run
/// use pcli2::physna_v3::PhysnaApiClient;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut client = PhysnaApiClient::new()
///         .with_access_token("your_access_token".to_string())
///         .with_client_credentials("your_client_id".to_string(), "your_client_secret".to_string());
///     
///     let tenants = client.list_tenants().await?;
///     Ok(())
/// }
/// ```
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
    /// - Default HTTP client with appropriate timeouts and headers
    /// 
    /// # Returns
    /// A new `PhysnaApiClient` instance ready for configuration
    /// 
    /// # Example
    /// ```
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// 
    /// let client = PhysnaApiClient::new();
    /// // Configure with your credentials
    /// let configured_client = client
    ///     .with_access_token("your_token".to_string())
    ///     .with_client_credentials("client_id".to_string(), "client_secret".to_string());
    /// ```
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

        // Check if we should retry due to authentication issues (401 Unauthorized or 403 Forbidden)
        // We retry on both 401 and 403 as they can both indicate authentication issues
        // A 401 clearly indicates an invalid token
        // A 403 can also indicate an expired token in some cases
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || response.status() == reqwest::StatusCode::FORBIDDEN {
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
                // Try to get the raw response text for debugging deserialization issues
                let response_text = retry_response.text().await?;
                trace!("Raw response text for deserialization: {}", response_text);
                
                // Try to parse and return the JSON response
                match serde_json::from_str::<T>(&response_text) {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        error!("Failed to deserialize response: {}. Raw response: {}", e, response_text);
                        Err(ApiError::JsonError(e))
                    }
                }
            } else {
                // Retry failed - provide clear error information
                let status = retry_response.status();
                let error_text = retry_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!("API request failed after retry. Original error: {}, Retry failed with status: {} and body: {}", 
                    response.status(), status, error_text);
                Err(ApiError::RetryFailed(format!(
                    "Original error: {}, Retry failed with status: {} and body: {}", 
                    response.status(), status, error_text
                )))
            }
        } else if response.status().is_success() {
            // Initial request was successful - try to get the raw response text for debugging
            let response_text = response.text().await?;
            trace!("Raw response text for deserialization: {}", response_text);
            
            // Try to parse and return the JSON response
            match serde_json::from_str::<T>(&response_text) {
                Ok(result) => Ok(result),
                Err(e) => {
                    error!("Failed to deserialize response: {}. Raw response: {}", e, response_text);
                    Err(ApiError::JsonError(e))
                }
            }
        } else {
            // For all other errors, return the error status
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
        // Log the request for debugging
        let body_json = serde_json::to_string_pretty(body).unwrap_or_else(|_| "Unable to serialize body".to_string());
        trace!("POST request to {}: {}", url, body_json);
        
        let result = self.execute_request(|client| client.post(url).json(body)).await;
        
        // Log the response for debugging
        match &result {
            Ok(_) => trace!("POST request to {} succeeded", url),
            Err(e) => trace!("POST request to {} failed: {}", url, e),
        }
        
        result
    }
    
    /// Generic method to build and execute PUT requests
    async fn put<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(|client| client.put(url).json(body)).await
    }
    
    /// Generic method to build and execute PATCH requests
    #[allow(dead_code)]
    async fn patch<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(|client| client.patch(url).json(body)).await
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
        let page_str = page.map(|p| p.to_string());
        let per_page_str = per_page.map(|pp| pp.to_string());
        
        if let Some(ref page_val) = page_str {
            query_params.push(("page", page_val.as_str()));
        }
        if let Some(ref per_page_val) = per_page_str {
            query_params.push(("per_page", per_page_val.as_str()));
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
    
    /// List folders in a specific parent folder with optional pagination
    /// 
    /// This method lists folders that have a specific parent folder, allowing
    /// for efficient traversal of the folder hierarchy without fetching all folders.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `parent_folder_id` - The ID of the parent folder (None for root level)
    /// * `page` - Page number for pagination (optional)
    /// * `per_page` - Number of items per page for pagination (optional)
    /// 
    /// # Returns
    /// * `Ok(FolderListResponse)` - List of folders in the parent
    /// * `Err(ApiError)` - If there was an error during API calls
    
    /// Get details for a specific folder by ID
    /// 
    /// This method fetches detailed information about a specific folder by its ID.
    /// The response includes folder metadata such as name, creation date, asset count, etc.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the folder
    /// * `folder_id` - The UUID of the folder to retrieve
    
    /// List folders in a specific parent folder with optional pagination
    /// 
    /// This method lists folders that have a specific parent folder, allowing
    /// for efficient traversal of the folder hierarchy without fetching all folders.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `parent_folder_id` - The ID of the parent folder (None for root level)
    /// * `page` - Page number for pagination (optional)
    /// * `per_page` - Number of items per page for pagination (optional)
    /// 
    /// # Returns
    /// * `Ok(FolderListResponse)` - List of folders in the parent
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_folders_in_parent(&mut self, tenant_id: &str, parent_folder_id: Option<&str>, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_id);
        
        // Build query parameters for parent filtering, pagination
        let mut query_params = vec![("contentType", "folders")];
        if let Some(parent_id) = parent_folder_id {
            query_params.push(("parentFolderId", parent_id));
        }
        
        // Store page and per_page as strings to avoid temporary value issues
        let page_str = page.map(|p| p.to_string());
        let per_page_str = per_page.map(|pp| pp.to_string());
        
        if let Some(ref page_val) = page_str {
            query_params.push(("page", page_val.as_str()));
        }
        if let Some(ref per_page_val) = per_page_str {
            query_params.push(("per_page", per_page_val.as_str()));
        }
        
        // Add query parameters to URL if provided
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);
        
        trace!("Making API call to list folders in parent: {}", url);
        self.get(&url).await
    }
    

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
    
    /// List assets in a specific folder by folder ID
    /// 
    /// This method lists assets that are contained in a specific folder using the 
    /// /tenants/{tenantId}/folders/{folderId}/contents endpoint with contentType=assets.
    /// This is the efficient way to list assets in a specific folder, unlike the
    /// list_assets method which fetches all assets in the tenant.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `folder_id` - The ID of the folder to list assets from
    /// * `page` - Page number for pagination (optional)
    /// * `per_page` - Number of items per page for pagination (optional)
    /// 
    /// # Returns
    /// * `Ok(AssetListResponse)` - List of assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_assets_in_folder(&mut self, tenant_id: &str, folder_id: &str, page: Option<u32>, per_page: Option<u32>) -> Result<crate::model::AssetListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}/contents", self.base_url, tenant_id, folder_id);
        
        // Build query parameters
        let mut query_params = vec![("contentType", "assets")];
        
        // Store page and per_page as strings to avoid temporary value issues
        let page_str = page.map(|p| p.to_string());
        let per_page_str = per_page.map(|pp| pp.to_string());
        
        if let Some(ref page_val) = page_str {
            query_params.push(("page", page_val.as_str()));
        }
        if let Some(ref per_page_val) = per_page_str {
            query_params.push(("per_page", per_page_val.as_str()));
        }
        
        // Add query parameters to URL
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);
        
        self.get(&url).await
    }
    
    /// Get the folder ID for a given path by traversing the folder structure efficiently
    /// 
    /// This method efficiently resolves a folder path to its corresponding folder ID
    /// by using the root/content and folderId/contents API endpoints, with content filtering.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `folder_path` - The path to resolve (e.g., "Root/Child/Grandchild" or "/Root/Child/Grandchild")
    /// 
    /// # Returns
    /// * `Ok(Some(String))` - The folder ID if found
    /// * `Ok(None)` - If the path doesn't exist
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_folder_id_by_path(&mut self, tenant_id: &str, folder_path: &str) -> Result<Option<String>, ApiError> {
        debug!("Resolving folder path: {} for tenant: {}", folder_path, tenant_id);
        
        // Normalize path by removing leading slash
        // Treat both "path" and "/path" as equivalent (absolute from root)
        let normalized_path = folder_path.strip_prefix('/').unwrap_or(folder_path);
        trace!("Normalized path: '{}' (original: '{}')", normalized_path, folder_path);
        
        if normalized_path.is_empty() {
            // For root path (empty or just "/"), get root contents
            let root_response = self.get_root_contents(tenant_id, "folders", Some(1), Some(1000)).await?;
            if root_response.folders.len() == 1 {
                return Ok(Some(root_response.folders[0].id.clone()));
            } else if root_response.folders.is_empty() {
                return Ok(None); // No root folders
            } else {
                // Multiple root folders - return the first one
                return Ok(Some(root_response.folders[0].id.clone()));
            }
        }
        
        // Split the path into components
        let path_parts: Vec<&str> = normalized_path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        
        // Start with root level
        let mut current_folder_id: Option<String> = None;
        let mut current_path_index = 0;
        
        while current_path_index < path_parts.len() {
            let target_name = path_parts[current_path_index];
            trace!("Looking for folder '{}' at index {} (current parent: {:?})", target_name, current_path_index, current_folder_id);
            
            // Get contents of current folder, filtered to folders only
            let child_folders = if current_folder_id.is_none() {
                // Root level - use get_root_contents
                trace!("Getting root contents for tenant: {}", tenant_id);
                match self.get_root_contents(tenant_id, "folders", Some(1), Some(1000)).await {
                    Ok(root_response) => {
                        debug!("Got {} folders at root level", root_response.folders.len());
                        for folder in &root_response.folders {
                            debug!("Root folder: '{}' (id: {})", folder.name, folder.id);
                        }
                        root_response.folders
                    }
                    Err(e) => {
                        trace!("Failed to get root contents: {}", e);
                        return Ok(None); // Return None to indicate path not found
                    }
                }
            } else {
                // Subfolder level - use get_folder_contents
                trace!("Getting folder contents for folder: {}", current_folder_id.as_ref().unwrap());
                match self.get_folder_contents(tenant_id, &current_folder_id.as_ref().unwrap(), "folders", Some(1), Some(1000)).await {
                    Ok(folder_response) => {
                        trace!("Got {} folders in folder {}", folder_response.folders.len(), current_folder_id.as_ref().unwrap());
                        for folder in &folder_response.folders {
                            trace!("  Subfolder: '{}' (id: {})", folder.name, folder.id);
                        }
                        folder_response.folders
                    }
                    Err(e) => {
                        trace!("Failed to get folder contents: {}", e);
                        return Ok(None); // Return None to indicate path not found
                    }
                }
            };
            
            debug!("Found {} child folders at this level", child_folders.len());
            for folder in &child_folders {
                debug!("Child folder: '{}' (id: {})", folder.name, folder.id);
            }
            
            // Find the folder with the target name
            let target_folder = child_folders.iter()
                .find(|folder| folder.name == target_name);
            
            if let Some(folder) = target_folder {
                debug!("Found target folder: '{}' (id: {})", folder.name, folder.id);
                if current_path_index == path_parts.len() - 1 {
                    // This is the final component, return its ID
                    debug!("This is the final component, returning ID: {}", folder.id);
                    return Ok(Some(folder.id.clone()));
                } else {
                    // Move to this folder as the parent for the next iteration
                    debug!("Moving to next level with parent ID: {}", folder.id);
                    current_folder_id = Some(folder.id.clone());
                    current_path_index += 1;
                }
            } else {
                // Folder not found in the path
                debug!("Folder '{}' not found in path: {}", target_name, normalized_path);
                return Ok(None);
            }
        }
        
        Ok(None)
    }
    
    /// List folders in a specific parent folder
    /// 
    /// This method lists folders that have a specific parent folder, allowing
    /// for efficient traversal of the folder hierarchy without fetching all folders.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `parent_folder_id` - The ID of the parent folder (None for root level)
    /// * `page` - Page number for pagination (optional)
    /// * `per_page` - Number of items per page for pagination (optional)
    /// 
    /// # Returns
    /// * `Ok(FolderListResponse)` - List of folders in the parent
    /// * `Err(ApiError)` - If there was an error during API calls
    
    /// Get contents of root folder by tenant ID, filtered by content type
    /// 
    /// This method gets contents of the root folder with a specific content type (folders only, assets only, or all).
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `content_type` - The type of content to return ("all", "assets", "folders")
    /// * `page` - Page number for pagination (optional)
    /// * `per_page` - Number of items per page for pagination (optional)
    /// 
    /// # Returns
    /// * `Ok(FolderListResponse)` - List of contents in the root folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_root_contents(&mut self, tenant_id: &str, _content_type: &str, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        // Use list_folders_in_parent with None parent to get root contents
        self.list_folders_in_parent(tenant_id, None, page, per_page).await
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
    pub async fn get_folder_contents(&mut self, tenant_id: &str, folder_id: &str, content_type: &str, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}/contents", self.base_url, tenant_id, folder_id);
        
        // Build query parameters
        let mut query_params = vec![("contentType", content_type)];
        
        // Store page and per_page as strings to avoid temporary value issues
        let page_str = page.map(|p| p.to_string());
        let per_page_str = per_page.map(|pp| pp.to_string());
        
        if let Some(ref page_val) = page_str {
            query_params.push(("page", page_val.as_str()));
        }
        if let Some(ref per_page_val) = per_page_str {
            query_params.push(("per_page", per_page_val.as_str()));
        }
        
        // Add query parameters to URL
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);
        
        self.get(&url).await
    }
    
    /// List assets in a specific folder by path
    /// 
    /// This method efficiently lists assets in a specific folder by first
    /// resolving the folder path to a folder ID and then listing assets in that folder.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `folder_path` - The path of the folder to list assets from
    /// 
    /// # Returns
    /// * `Ok(AssetListResponse)` - List of assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_assets_by_path(&mut self, tenant_id: &str, folder_path: &str) -> Result<crate::model::AssetListResponse, ApiError> {
        debug!("Listing assets by path: {} for tenant: {}", folder_path, tenant_id);
        
        let folder_id = match self.get_folder_id_by_path(tenant_id, folder_path).await {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => {
                return Err(ApiError::ConflictError(format!("Folder path '{}' not found", folder_path)));
            }
        };
        
        // Now list assets in this specific folder using the efficient API endpoint
        self.list_assets_in_folder(tenant_id, &folder_id, Some(1), Some(1000)).await
    }
    
    /// Get contents (both folders and assets) of a specific folder path
    /// 
    /// This method efficiently gets both subfolders and assets within a specific folder
    /// by first resolving the path and then making separate API calls for each.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// * `folder_path` - The path of the folder to get contents from
    /// 
    /// # Returns
    /// * `Ok((Vec<FolderResponse>, Vec<AssetResponse>))` - Folders and assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_folder_contents_by_path(&mut self, tenant_id: &str, folder_path: &str) -> Result<(Vec<crate::model::FolderResponse>, Vec<crate::model::AssetResponse>), ApiError> {
        debug!("Getting contents by path: {} for tenant: {}", folder_path, tenant_id);
        
        let folder_id = match self.get_folder_id_by_path(tenant_id, folder_path).await {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => {
                return Err(ApiError::ConflictError(format!("Folder path '{}' not found", folder_path)));
            }
        };
        
        // Get subfolders in the folder using the more efficient content API
        let subfolders_response = self.get_folder_contents(tenant_id, &folder_id, "folders", Some(1), Some(1000)).await?;
        let subfolders = subfolders_response.folders;
        
        // Get assets in the folder
        let assets_response = self.list_assets(tenant_id, Some(folder_id), Some(1), Some(1000)).await?;
        let assets = assets_response.assets;
        
        Ok((subfolders, assets))
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
        debug!("Getting asset details for tenant_id: {}, asset_id: {}", tenant_id, asset_id);
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_id, asset_id);
        let response: crate::model::SingleAssetResponse = self.get(&url).await?;
        debug!("Successfully retrieved asset details for asset_id: {}", asset_id);
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
    
    /// Update an asset's metadata fields
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to update
    /// * `metadata` - A map of metadata key-value pairs to update
    /// 
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully updated asset with new metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn update_asset_metadata(&mut self, tenant_id: &str, asset_id: &str, metadata: &std::collections::HashMap<String, serde_json::Value>) -> Result<(), ApiError> {
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_id, asset_id);
        
        let body = serde_json::json!({
            "metadata": metadata
        });
        
        // Log the request body for debugging
        debug!("Updating asset metadata with JSON body: {}", serde_json::to_string_pretty(&body).unwrap_or_else(|_| "Unable to serialize body".to_string()));
        
        self.patch_no_response(&url, &body).await
    }
    
    /// Delete specific metadata fields from an asset
    /// 
    /// This method deletes specific metadata fields from the specified asset.
    /// The metadata keys are sent as a direct array in the request body.
    /// 
    /// # Arguments
    /// Delete specific metadata fields from an asset
    /// 
    /// This method deletes specific metadata fields from the specified asset.
    /// The metadata keys are sent as an object with a "metadataFieldNames" array.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to update
    /// * `metadata_keys` - A vector of metadata field names to delete
    /// 
    /// # Returns
    /// * `Ok(())` - Successfully deleted metadata from the asset
    /// * `Err(ApiError)` - HTTP error or other error occurred
    pub async fn delete_asset_metadata(&mut self, tenant_id: &str, asset_id: &str, metadata_keys: Vec<&str>) -> Result<(), ApiError> {
        let url = format!("{}/tenants/{}/assets/{}/metadata", self.base_url, tenant_id, asset_id);
        
        // Send metadata keys wrapped in "metadataFieldNames" object as required by API
        let body = serde_json::json!({
            "metadataFieldNames": metadata_keys
        });
        
        // Log the request body for debugging
        debug!("Deleting asset metadata with JSON body: {}", serde_json::to_string_pretty(&body).unwrap_or_else(|_| "Unable to serialize body".to_string()));
        
        self.delete_with_body(&url, &body).await
    }
    /// * `field_name` - The name of the metadata field to create
    /// 
    /// # Returns
    /// * `Ok(serde_json::Value)` - Response from the API confirming the field was created
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn create_metadata_field(&mut self, tenant_id: &str, field_name: &str) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/tenants/{}/metadata-fields", self.base_url, tenant_id);
        
        let body = serde_json::json!({
            "name": field_name,
            "type": "text"  // Default to text as specified
        });
        
        self.post(&url, &body).await
    }
    
    /// Generic method to build and execute PATCH requests that may return empty responses
    /// 
    /// This method is similar to the standard patch method but handles empty responses gracefully.
    /// It's useful for API endpoints that return 204 No Content or empty bodies on success.
    /// 
    /// # Type Parameters
    /// * `B` - The type of the request body (must implement `Serialize`)
    /// 
    /// # Arguments
    /// * `url` - The URL to send the PATCH request to
    /// * `body` - The request body to send with the PATCH request
    /// 
    /// # Returns
    /// * `Ok(())` - Successfully executed request (empty response is considered success)
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    async fn patch_no_response<B>(&mut self, url: &str, body: &B) -> Result<(), ApiError>
    where
        B: serde::Serialize,
    {
        self.execute_request_no_response(|client| client.patch(url).json(body)).await
    }
    
    /// Generic method to build and execute DELETE requests that may have a request body and return empty responses
    /// 
    /// This method is similar to the standard delete method but allows request bodies for DELETE operations.
    /// It's useful for API endpoints like deleting specific metadata that require a body.
    /// 
    /// # Type Parameters
    /// * `B` - The type of the request body (must implement `Serialize`)
    /// 
    /// # Arguments
    /// * `url` - The URL to send the DELETE request to
    /// * `body` - The request body to send with the DELETE request
    /// 
    /// # Returns
    /// * `Ok(())` - Successfully executed request (empty response is considered success)
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    async fn delete_with_body<B>(&mut self, url: &str, body: &B) -> Result<(), ApiError>
    where
        B: serde::Serialize,
    {
        self.execute_request_no_response(|client| client.delete(url).json(body)).await
    }
    
    /// Generic method to execute requests that may return empty responses
    /// 
    /// This method is similar to execute_request but handles empty responses gracefully.
    /// It's useful for API endpoints that return 204 No Content or empty bodies on success.
    /// 
    /// # Type Parameters
    /// * `F` - A closure that builds the HTTP request
    /// 
    /// # Arguments
    /// * `request_builder` - A closure that takes a `reqwest::Client` and returns a `RequestBuilder`
    /// 
    /// # Returns
    /// * `Ok(())` - Successfully executed request (empty response is considered success)
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    async fn execute_request_no_response<F>(&mut self, request_builder: F) -> Result<(), ApiError>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        // Build and execute the initial request
        let mut request = request_builder(&self.http_client);
        
        // Add access token header if available for authentication
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;

        // Check if we should retry due to authentication issues (401 Unauthorized or 403 Forbidden)
        // We retry on both 401 and 403 as they can both indicate authentication issues
        // A 401 clearly indicates an invalid token
        // A 403 can also indicate an expired token in some cases
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || response.status() == reqwest::StatusCode::FORBIDDEN {
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
                // For empty responses, we consider success as a successful update
                Ok(())
            } else {
                // Retry failed - provide clear error information
                let status = retry_response.status();
                let error_text = retry_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!("API request failed after retry. Original error: {}, Retry failed with status: {} and body: {}", 
                    response.status(), status, error_text);
                Err(ApiError::RetryFailed(format!(
                    "Original error: {}, Retry failed with status: {} and body: {}", 
                    response.status(), status, error_text
                )))
            }
        } else if response.status().is_success() {
            // Initial request was successful - for empty responses, we consider this a success
            Ok(())
        } else {
            // For all other errors, return the error status
            Err(ApiError::HttpError(response.error_for_status().unwrap_err()))
        }
    }
    
    /// Get all metadata fields for a tenant
    /// 
    /// This method retrieves the list of all metadata fields defined for the specified tenant.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    /// 
    /// # Returns
    /// * `Ok(MetadataFieldListResponse)` - List of metadata fields for the tenant
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_metadata_fields(&mut self, tenant_id: &str) -> Result<crate::model::MetadataFieldListResponse, ApiError> {
        let url = format!("{}/tenants/{}/metadata-fields", self.base_url, tenant_id);
        
        self.get(&url).await
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
    /// * `folder_path` - Optional folder path where to place the asset (e.g., "/Root/Folder/Subfolder")
    /// * `folder_id` - Optional folder ID where to place the asset (takes precedence if both path and ID are provided)
    /// 
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully created asset details from the API
    /// * `Err(ApiError)` - If there's an HTTP error, IO error, authentication issue, or other API error
    ///                     including conflict errors if the asset already exists
    /// 
    /// # Example
    /// ```no_run
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let asset = client.create_asset("tenant-uuid", "/path/to/file.stl", Some("/Root/MyFolder"), None).await?;
    ///     println!("Created asset with UUID: {}", asset.id);
    ///     Ok(())
    /// }
    /// ```
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
    
    /// Perform a geometric search for similar assets
    /// 
    /// This method searches for assets that are geometrically similar to the reference asset.
    /// It uses Physna's advanced geometric matching algorithms to find assets with similar
    /// shapes, regardless of orientation, scale, or position differences.
    /// 
    /// The method includes automatic retry logic for handling conflict errors (HTTP 409),
    /// which can occur when the search service is temporarily busy.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the reference asset
    /// * `asset_id` - The UUID of the reference asset to search for similar matches
    /// * `threshold` - The similarity threshold as a percentage (0.00 to 100.00)
    ///                Lower values return more matches, higher values return fewer but more similar matches
    /// 
    /// # Returns
    /// * `Ok(crate::model::GeometricSearchResponse)` - The search results containing similar assets
    /// * `Err(ApiError)` - If there's an HTTP error, authentication issue, or other API error
    /// 
    /// # Example
    /// ```no_run
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let matches = client.geometric_search("tenant-uuid", "asset-uuid", 85.0).await?;
    ///     for match_result in &matches.matches {
    ///         println!("Found match: {} ({}% similar)", match_result.path(), match_result.score());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn geometric_search(&mut self, tenant_id: &str, asset_id: &str, threshold: f64) -> Result<crate::model::GeometricSearchResponse, ApiError> {
        debug!("Starting geometric search for tenant_id: {}, asset_id: {}, threshold: {}", tenant_id, asset_id, threshold);
        let url = format!("{}/tenants/{}/assets/{}/geometric-search", self.base_url, tenant_id, asset_id);
        
        // Build request body with the correct structure
        let body = serde_json::json!({
            "page": 1,
            "perPage": 20,
            "searchQuery": "",
            "filters": {
                "folders": [],
                "metadata": {},
                "extensions": []  // Empty array as requested
            },
            "minThreshold": threshold  // Use threshold directly as percentage
        });
        
        debug!("Sending geometric search request to: {}", url);
        // Execute POST request
        let result = self.post(&url, &body).await;
        debug!("Geometric search completed for asset_id: {}", asset_id);
        result
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
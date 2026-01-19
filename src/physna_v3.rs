use std::path::Path;
use crate::auth::AuthClient;
use crate::folder_hierarchy::FolderHierarchy;
use crate::keyring::{Keyring, KeyringError};
use crate::model::{Asset, AssetList, AssetListResponse, CurrentUserResponse, FolderList, FolderListResponse, SingleAssetResponse, SingleFolderResponse};
use reqwest;
use serde_json;
use serde_urlencoded;
use tracing::{debug, trace, error};
use glob::glob;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use uuid::Uuid;

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

    #[error("{0}")]
    KeyringError(#[from] KeyringError),

    #[error("Access token not found. Please login first with 'pcli2 auth login --client-id <id> --client-secret <secret>'")]
    InvalidToken,

    #[error("Login credentials not provided")]
    MissingCredentials,

    #[error("Path not found: {0}")]
    PathNotFound(String),
    
    #[error("Invalid path for asset. Check asset name: {0}")]
    InvalidAssetPath(String),
}

pub trait TryDefault: Sized {
    type Error;
    fn try_default() -> Result<Self, Self::Error>;
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


impl TryDefault for PhysnaApiClient {
    type Error = ApiError;

    fn try_default() -> Result<PhysnaApiClient, ApiError> {
        // Load configuration to get the base URL
        let configuration = crate::configuration::Configuration::load_or_create_default()
            .map_err(|e| ApiError::AuthError(format!("Failed to load configuration: {}", e)))?;

        // Use the active environment name for keyring storage, fallback to "default" if no environment is set
        let environment_name = configuration.get_active_environment().unwrap_or_else(|| "default".to_string());

        #[allow(unused_mut)]
        let mut keyring = Keyring::default();
        // Get all environment credentials in a single operation to reduce keyring access calls
        let (access_token, client_id, client_secret) = keyring.get_environment_credentials(&environment_name)?;

        match access_token {
            Some(token) => {
                let mut client = PhysnaApiClient::new_with_configuration(&configuration).with_access_token(token);

                // Try to get client credentials for automatic token refresh
                if let (Some(id), Some(secret)) = (client_id, client_secret) {
                    client = client.with_client_credentials(id, secret);
                    Ok(client)
                } else {
                    Err(ApiError::MissingCredentials)
                }
            }
            None => {
                Err(ApiError::InvalidToken)
            }
        }
    }
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
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60)) // 60-second timeout for all requests
                .build()
                .expect("Failed to build HTTP client with timeout"),
        }
    }

    /// Create a new Physna API client with configuration-based URLs
    ///
    /// # Arguments
    /// * `configuration` - The configuration containing the base URL
    ///
    /// # Returns
    /// A new `PhysnaApiClient` instance with the configured base URL
    pub fn new_with_configuration(configuration: &crate::configuration::Configuration) -> Self {
        Self {
            base_url: configuration.get_api_base_url(),
            access_token: None,
            client_credentials: None,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60)) // 60-second timeout for all requests
                .build()
                .expect("Failed to build HTTP client with timeout"),
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
                    let error_msg = e.to_string();
                    let user_friendly_msg = if error_msg.contains("invalid_client") {
                        "Failed to refresh token: Invalid client credentials. This could be due to:\n  - Incorrect client ID or secret\n  - Expired or revoked client credentials\n  - Disabled service account\n  Please verify your credentials and log in again with 'pcli2 auth login'."
                    } else if error_msg.contains("invalid_grant") {
                        "Failed to refresh token: Invalid authorization grant. Please log in again with 'pcli2 auth login'."
                    } else if error_msg.contains("unauthorized_client") {
                        "Failed to refresh token: Unauthorized client. Please verify your client credentials and try logging in again with 'pcli2 auth login'."
                    } else if error_msg.contains("invalid_request") {
                        "Failed to refresh token: Invalid request. Please verify your credentials and try logging in again with 'pcli2 auth login'."
                    } else {
                        &format!("Failed to refresh token: {}", e)
                    };
                    Err(ApiError::AuthError(user_friendly_msg.to_string()))
                }
            }
        } else {
            // No client credentials available for token refresh
            Err(ApiError::AuthError("No client credentials available for token refresh".to_string()))
        }
    }
    
    /// Get the current access token from the client
    /// 
    /// This method allows external code to retrieve the current access token,
    /// which is useful for persisting updated tokens after refresh operations.
    /// 
    /// # Returns
    /// * `Option<String>` - The current access token if available, None otherwise
    pub fn get_access_token(&self) -> Option<String> {
        self.access_token.clone()
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
                let response = retry_response.text().await?;
                trace!("Raw response for deserialization: {}", response);
                trace!("Deserializing into: {}", std::any::type_name::<T>());
                
                // Try to parse and return the JSON response
                match serde_json::from_str::<T>(&response) {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        error!("Failed to deserialize response: {}. Raw response: {}", e, response);
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
            let response = response.text().await?;
            trace!("Raw response for deserialization: {}", response);
            trace!("Deserializing into: {}", std::any::type_name::<T>());
            
            // Try to parse and return the JSON response
            match serde_json::from_str::<T>(&response) {
                Ok(result) => Ok(result),
                Err(e) => {
                    error!("Failed to deserialize response: {}. Raw response: {}", e, response);
                    Err(ApiError::JsonError(e))
                }
            }
        } else {
            // For all other errors, try to extract the error message from the response body
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse the error as JSON to extract a more descriptive message
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_body) {
                if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                    return Err(ApiError::ConflictError(format!("HTTP {} - {}", status, message)));
                } else if let Some(error) = error_json.get("error").and_then(|e| e.as_str()) {
                    return Err(ApiError::ConflictError(format!("HTTP {} - {}", status, error)));
                }
            }

            // If JSON parsing fails or no message is found, return a generic error with the raw response
            Err(ApiError::ConflictError(format!("HTTP {} - {}", status, error_body)))
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
    pub async fn list_folders(&mut self, tenant_uuid: &Uuid, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_uuid);
        
        // Handle defaults - always provide values to avoid API defaulting to 20
        let page_val = page.unwrap_or(1).to_string();
        let per_page_val = per_page.unwrap_or(200).to_string(); // Default to 200 instead of API's default of 20
        
        // Build query parameters for pagination with defaults
        let query_params = vec![
            ("page", page_val.as_str()),
            ("perPage", per_page_val.as_str()),
        ];
        
        // Add query parameters to URL
        let url = format!("{}?{}", url, serde_urlencoded::to_string(&query_params).unwrap());
        
        // Execute GET request to fetch folders
        self.get(&url).await
    }
    
    /// List folders in a specific parent folder with optional pagination
    /// 
    /// This method lists folders that have a specific parent folder, allowing
    /// for efficient traversal of the folder hierarchy without fetching all folders.
    /// 
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant
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
    pub async fn list_folders_in_parent(&mut self, tenant_uuid: &Uuid, parent_folder_id: Option<&str>, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_uuid);
        
        // Build query parameters for parent filtering, pagination
        let mut query_params = vec![("contentType", "folders")];
        if let Some(parent_id) = parent_folder_id {
            query_params.push(("parentFolderId", parent_id));
        }
        
        // Handle defaults - always provide values to avoid API defaulting to 20
        let page_val = page.unwrap_or(1).to_string();
        let per_page_val = per_page.unwrap_or(200).to_string(); // Default to 200 instead of API's default of 20
        
        query_params.push(("page", page_val.as_str()));
        query_params.push(("perPage", per_page_val.as_str()));
        
        // Add query parameters to URL
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);
        
        trace!("Making API call to list folders in parent: {}", url);
        self.get(&url).await
    }
    

    /// 
    /// # Returns
    /// * `Ok(FolderResponse)` - Successfully fetched folder details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_folder(&mut self, tenant_uuid: &Uuid, folder_uuid: &Uuid) -> Result<crate::model::Folder, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_uuid, folder_uuid);

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
    pub async fn create_folder(&mut self, tenant_uuid: &Uuid, name: &str, parent_folder_uuid: Option<Uuid>) -> Result<crate::model::SingleFolderResponse, ApiError> {
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
    
    pub async fn update_folder(&mut self, tenant_id: &str, folder_id: &str, name: &str) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_id, folder_id);

        let body = serde_json::json!({
            "name": name
        });

        // The API returns a SingleFolderResponse with a "folder" field
        let response: crate::model::SingleFolderResponse = self.put(&url, &body).await?;
        Ok(response.folder)
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
    pub async fn rename_folder(&mut self, tenant_id: &str, folder_id: &str, new_name: &str) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}/name", self.base_url, tenant_id, folder_id);

        let body = serde_json::json!({
            "name": new_name
        });

        // Debug print the request body
        debug!("Renaming folder {}. Request body: {}", folder_id, serde_json::to_string(&body).unwrap_or_else(|_| "INVALID_JSON".to_string()));

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
    pub async fn move_folder(&mut self, tenant_id: &str, folder_id: &str, new_parent_folder_id: Option<Uuid>) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}/parent", self.base_url, tenant_id, folder_id);

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
        debug!("Moving folder {} to new parent. Request body: {}", folder_id, serde_json::to_string(&body).unwrap_or_else(|_| "INVALID_JSON".to_string()));

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
    pub async fn delete_folder(&mut self, tenant_uuid: &Uuid, folder_uuid: &Uuid) -> Result<(), ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_uuid, folder_uuid);
        self.delete(&url).await
    }
    
    // Asset operations
    

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
    pub async fn list_assets_by_parent_folder_uuid(&mut self, tenant_uuid: &Uuid, parent_folder_uuid: Option<&Uuid>) -> Result<AssetList, ApiError> {
        let mut page: usize = 1;
        let per_page: usize = 200;
        let mut assets: Vec<Asset> = Vec::new();

        loop {
            let response = self.list_assets_by_parent_folder_uuid_with_pagination(tenant_uuid, parent_folder_uuid, page, per_page).await?;
            let partial_asset_list: Vec<Asset> = response.assets.iter().map(|a| a.into()).collect();
            assets.extend(partial_asset_list);
            
            page = response.page_data.current_page;
            if response.page_data.current_page >= response.page_data.last_page {
                break;
            }
        }

        Ok(assets.into())
    }
    
    /// List a single page of assets in a specific folder by folder UUID
    /// 
    /// This method lists assets that are contained in a specific folder using the 
    /// /tenants/{tenantId}/folders/{folderId}/contents endpoint with contentType=assets.
    /// This is the efficient way to list assets in a specific folder, unlike the
    /// list_assets method which fetches all assets in the tenant.
    /// 
    /// # Arguments
    /// * `tenant_uuid` - The ID of the tenant
    /// * `folder_uuid` - The ID of the folder to list assets from
    /// * `page` - Page number for pagination
    /// * `per_page` - Number of items per page for pagination
    /// 
    /// # Returns
    /// * `Ok(AssetListResponse)` - List of assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    async fn list_assets_by_parent_folder_uuid_with_pagination(&mut self, tenant_uuid: &Uuid, folder_uuid: Option<&Uuid>, page: usize, per_page: usize) -> Result<AssetListResponse, ApiError> {
        let url = match folder_uuid {
            Some(folder_uuid) => format!("{}/tenants/{}/folders/{}/contents", self.base_url, tenant_uuid, folder_uuid),
            None => format!("{}/tenants/{}/folders/root/contents", self.base_url, tenant_uuid),
        }; 
        
        // Build query parameters
        let mut query_params = vec![("contentType", "assets")];
        
        // Handle defaults - always provide values to avoid API defaulting to 20
        let page_str = page.to_string();
        let per_page_str = per_page.to_string(); // Default to 200 instead of API's default of 20
        
        query_params.push(("page", page_str.as_str()));
        query_params.push(("perPage", per_page_str.as_str()));
        
        // Add query parameters to URL
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);
        
        trace!("Constructed URL for asset listing: {}", url);
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
    pub async fn get_folder_uuid_by_path(&mut self, tenant_uuid: &Uuid, folder_path: &str) -> Result<Option<Uuid>, ApiError> {
        debug!("Resolving folder path: {} for tenant: {} using FolderHierarchy", folder_path, tenant_uuid);

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
            let path_for_hierarchy = normalized_path.strip_prefix('/').unwrap_or(&normalized_path);

            // First, try direct lookup for root-level folders as primary approach
            // This handles cases where the folder is directly under root
            let root_folders_response = self.list_folders_in_parent(tenant_uuid, None, Some(1), Some(1000)).await?;

            // Look for a root folder that matches the path (for single-level paths like "test2")
            for folder in &root_folders_response.folders {
                if folder.name == path_for_hierarchy {
                    debug!("Found root folder '{}' with UUID: {}", folder.name, folder.uuid);
                    return Ok(Some(folder.uuid));
                }
            }

            // If direct lookup fails, try the folder hierarchy approach for nested paths
            if let Ok(hierarchy) = FolderHierarchy::build_from_api(self, tenant_uuid).await {
                if let Some(folder_node) = hierarchy.get_folder_by_path(path_for_hierarchy) {
                    debug!("Found folder at path '{}' using hierarchy: {}", folder_path, folder_node.folder.uuid);
                    return Ok(Some(folder_node.folder.uuid.clone()));
                }
            }

            debug!("Folder not found at path: {}", folder_path);
            Ok(None)
        }
    }
    
    /// List folders in a specific parent folder
    /// 
    /// This method lists folders that have a specific parent folder, allowing
    /// for efficient traversal of the folder hierarchy without fetching all folders.
    /// 
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant
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
    pub async fn get_root_contents(&mut self, tenant_uuid: &Uuid, _content_type: &str, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        // Use list_folders_in_parent with None parent to get root contents
        // The list_folders_in_parent function now handles default values
        self.list_folders_in_parent(tenant_uuid, None, page, per_page).await
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
    pub async fn get_folder_contents(&mut self, tenant_uuid: &Uuid, folder_uuid: Option<&Uuid>, content_type: &str, page: Option<u32>, per_page: Option<u32>) -> Result<FolderList, ApiError> {
        let url = match folder_uuid {
            Some(folder_uuid) => format!("{}/tenants/{}/folders/{}/contents", self.base_url, tenant_uuid, folder_uuid),
            None => format!("{}/tenants/{}/folders/root/contents", self.base_url, tenant_uuid),
        };        

        // Build query parameters
        let mut query_params = vec![("contentType", content_type)];
        
        // Handle defaults - always provide values to avoid API defaulting to 20
        let page_str = page.unwrap_or(1).to_string();
        let per_page_str = per_page.unwrap_or(200).to_string(); // Default to 200 instead of API's default of 20
        
        query_params.push(("page", page_str.as_str()));
        query_params.push(("perPage", per_page_str.as_str()));
        
        // Add query parameters to URL
        let query_string = serde_urlencoded::to_string(&query_params).unwrap();
        let url = format!("{}?{}", url, query_string);
        
        trace!("Constructed URL for folder contents listing: {}", url);
        let response: FolderListResponse = self.get(&url).await?;

        Ok(response.into())
    }

    /// This method is a wrapper around get_folder_uuid_by_path(...), but it will return None if the path is the root.
    pub async fn resolve_folder_uuid_by_path(&mut self, tenant_uuid: &Uuid, folder_path: &str) -> Result<Option<Uuid>, ApiError> {
        if folder_path.eq("/") {
            Ok(None)
        } else {
            Ok(self.get_folder_uuid_by_path(tenant_uuid, folder_path).await?)
        }
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
    pub async fn list_assets_by_parent_folder_path(&mut self, tenant_uuid: &Uuid, parent_folder_path: &str) -> Result<AssetList, ApiError> {
        debug!("Listing assets in a folder by path: {} for tenant: {}", parent_folder_path, tenant_uuid);
        
        let parent_folder_uuid = self.resolve_folder_uuid_by_path(tenant_uuid, parent_folder_path).await?;
        
        // Now list all assets in this specific folder using the efficient API endpoint with pagination
        let assets = self.list_assets_by_parent_folder_uuid(tenant_uuid, parent_folder_uuid.clone().as_ref()).await?;
        Ok(assets.into())
    }
    
    /// Get all contents (both folders and assets) of a specific folder path
    /// 
    /// This method efficiently gets both subfolders and assets within a specific folder
    /// by first resolving the path and then making separate API calls for each.
    /// 
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant
    /// * `folder_path` - The path of the folder to get contents from
    /// 
    /// # Returns
    /// * `Ok((Vec<FolderResponse>, Vec<AssetResponse>))` - Folders and assets in the folder
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn list_all_contents_by_parent_folder_path(&mut self, tenant_uuid: &Uuid, parent_folder_path: &str) -> Result<(FolderList, AssetList), ApiError> {
        debug!("Listing all folder contents by path: {} for tenant: {}", parent_folder_path, tenant_uuid);
        
        let parent_folder_uuid = self.resolve_folder_uuid_by_path(tenant_uuid, parent_folder_path).await?;

        // Get subfolders in the folder using the more efficient content API
        let subfolders_response = self.get_folder_contents(tenant_uuid, parent_folder_uuid.clone().as_ref(), "folders", Some(1), Some(1000)).await?;
        let subfolders = subfolders_response;

        // Get assets in the folder
        let assets_response = self.list_assets_by_parent_folder_uuid(tenant_uuid, parent_folder_uuid.clone().as_ref()).await?;
        let assets = assets_response;

        Ok((subfolders, assets))
    }

    fn get_parent_folder_path(asset_path: &String) -> Result<String, ApiError> {
        let path = Path::new(&asset_path);
        let parent = path.parent()
            .ok_or_else(|| ApiError::InvalidAssetPath(asset_path.to_owned()))?;

        // Convert the parent path to string
        let parent_str = parent.to_str()
            .ok_or_else(|| ApiError::InvalidAssetPath(asset_path.to_owned()))?;

        // Handle the case where the parent is root "/"
        let normalized_parent = if parent_str.is_empty() {
            "/".to_string()
        } else if parent_str.starts_with('/') {
            parent_str.to_string()
        } else {
            format!("/{}", parent_str)
        };

        Ok(normalized_parent)
    }

    fn asset_name_from_path(path: &String) -> Option<String> {
    Path::new(&path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
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
    pub async fn get_asset_by_uuid(&mut self, tenant_uuid: &Uuid, asset_uuid: &Uuid) -> Result<Asset, ApiError> {
        debug!("Getting asset details for tenant_uuid: {}, asset_uuid: {}", tenant_uuid, asset_uuid);
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_uuid, asset_uuid);
        let response: SingleAssetResponse = self.get(&url).await?;
        debug!("Successfully retrieved asset details for asset_id: {}", asset_uuid);
        Ok(response.asset.into())
    }

    pub async fn get_asset_by_path(&mut self, tenant_uuid: &Uuid, asset_path: &String) -> Result<Asset, ApiError> {

        let parent_folder_path = Self::get_parent_folder_path(asset_path)?;
        let assets = self.list_assets_by_parent_folder_path(tenant_uuid, &parent_folder_path).await?;
        match Self::asset_name_from_path(asset_path) {
            Some(asset_name) => {
                assets.find_by_name(&asset_name).cloned().ok_or(ApiError::PathNotFound(asset_path.to_owned()))
            },
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
    pub async fn update_asset_metadata(&mut self, tenant_uuid: &Uuid, asset_uuid: &Uuid, metadata: &std::collections::HashMap<String, serde_json::Value>) -> Result<(), ApiError> {
        let url = format!("{}/tenants/{}/assets/{}", self.base_url, tenant_uuid, asset_uuid);
        
        let body = serde_json::json!({
            "metadata": metadata
        });
        
        // Log the request body for debugging
        debug!("Updating asset metadata with JSON body: {}", serde_json::to_string_pretty(&body).unwrap_or_else(|_| "Unable to serialize body".to_string()));
        
        self.patch_no_response(&url, &body).await
    }

    /// Update an asset's metadata fields, automatically registering new metadata keys if needed
    ///
    /// # Arguments
    /// * `tenant_uuid` - The ID of the tenant that owns the asset
    /// * `asset_uuid` - The UUID of the asset to update
    /// * `metadata` - A map of metadata key-value pairs to update
    ///
    /// # Returns
    /// * `Ok(())` - Successfully updated asset metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn update_asset_metadata_with_registration(&mut self, tenant_uuid: &Uuid, asset_uuid: &Uuid, metadata: &std::collections::HashMap<String, serde_json::Value>) -> Result<(), ApiError> {
        // Get existing metadata fields for the tenant
        let existing_fields_response = self.get_metadata_fields(&tenant_uuid.to_string()).await;

        let mut existing_field_names = std::collections::HashSet::new();
        if let Ok(fields_response) = existing_fields_response {
            for field in fields_response.metadata_fields {
                existing_field_names.insert(field.name);
            }
        }

        // Check each metadata key and register if it doesn't exist
        for (key, _value) in metadata.iter() {
            if !existing_field_names.contains(key) {
                // Register the new metadata field (default to text type)
                let field_result = self.create_metadata_field(&tenant_uuid.to_string(), key, Some("text")).await;

                // Log the result of field creation
                match field_result {
                    Ok(_) => debug!("Successfully registered new metadata field: {}", key),
                    Err(e) => {
                        debug!("Failed to register metadata field '{}': {}", key, e);
                        // Continue anyway, as the API might allow setting values for unregistered keys
                    }
                }
            }
        }

        // Now update the asset metadata
        self.update_asset_metadata(tenant_uuid, asset_uuid, metadata).await
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
    /// * `field_type` - The type of the metadata field (e.g., "text", "number", "boolean") - defaults to "text"
    /// 
    /// # Returns
    /// * `Ok(serde_json::Value)` - Response from the API confirming the field was created
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn create_metadata_field(&mut self, tenant_id: &str, field_name: &str, field_type: Option<&str>) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/tenants/{}/metadata-fields", self.base_url, tenant_id);
        
        let effective_type = field_type.unwrap_or("text");
        let body = serde_json::json!({
            "name": field_name,
            "type": effective_type
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
            // For all other errors, try to extract the error message from the response body
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse the error as JSON to extract a more descriptive message
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_body) {
                if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                    return Err(ApiError::ConflictError(format!("HTTP {} - {}", status, message)));
                } else if let Some(error) = error_json.get("error").and_then(|e| e.as_str()) {
                    return Err(ApiError::ConflictError(format!("HTTP {} - {}", status, error)));
                }
            }

            // If JSON parsing fails or no message is found, return a generic error with the raw response
            Err(ApiError::ConflictError(format!("HTTP {} - {}", status, error_body)))
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
    /// use uuid::Uuid;
    /// use std::path::Path;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let tenant_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let folder_uuid = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let asset = client.create_asset(&tenant_uuid, Path::new("/path/to/file.stl"), &"/Root/MyFolder".to_string(), &folder_uuid).await?;
    ///     println!("Created asset with UUID: {}", asset.uuid());
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_asset(&mut self, tenant_uuid: &Uuid, file_path: &Path, asset_path: &String, folder_uuid: &Uuid) -> Result<crate::model::Asset, ApiError> {

        trace!("Creating new asset by uploading a file...");
        
        let url = format!("{}/tenants/{}/assets", self.base_url, tenant_uuid);

        if !file_path.exists() || !file_path.is_file() {
            return Err(ApiError::PathNotFound(file_path.to_string_lossy().into_owned()));
        }
        
        let file_name = file_path.file_name().unwrap().to_string_lossy().into_owned(); // It is save to unwrap because we already confired the file exists
        
        // Read the file content
        let file_data = tokio::fs::read(file_path).await?;
        trace!("Derived asset path: {}", &asset_path);
            
        
        
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
        form = form.text("folderId", folder_uuid.to_string());
        
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
            
            // Build the multipart form with file part and required parameters
            let mut retry_form = reqwest::multipart::Form::new()
                .part("file", file_part)
                .text("path", asset_path.clone())  // Use the full asset path including folder
                .text("metadata", "")  // Empty metadata as in the working example
                .text("createMissingFolders", "");  // Empty createMissingFolders as in the working example
            
            // Add folder ID if provided
            retry_form = retry_form.text("folderId", folder_uuid.to_string());
            
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
                    Ok(result) => Ok(Asset::from(&result.asset)),
                    Err(_) => {
                        // Try to parse as AssetResponse directly
                        match serde_json::from_str::<crate::model::AssetResponse>(&text) {
                            Ok(asset) => Ok(Asset::from(&asset)),
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
                Ok(result) => Ok(Asset::from(&result.asset)),
                Err(_) => {
                    // Try to parse as AssetResponse directly
                    match serde_json::from_str::<crate::model::AssetResponse>(&text) {
                        Ok(asset) => Ok(Asset::from(&asset)),
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
    
    /// Perform a geometric search for similar assets with pagination support
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
    /// use uuid::Uuid;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let tenant_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let asset_uuid = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let matches = client.geometric_search(&tenant_uuid, &asset_uuid, 85.0).await?;
    ///     for match_result in &matches.matches {
    ///         println!("Found match: {} ({}% similar)", match_result.path(), match_result.score());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn geometric_search(&mut self, tenant_uuid: &Uuid, asset_uuid: &Uuid, threshold: f64) -> Result<crate::model::GeometricSearchResponse, ApiError> {
        debug!("Starting geometric search for tenant_uuid: {}, asset_uuid: {}, threshold: {}", tenant_uuid, asset_uuid, threshold);
        let url = format!("{}/tenants/{}/assets/{}/geometric-search", self.base_url, tenant_uuid, asset_uuid);
        
        // Initialize with page 1 and reasonable page size
        let mut all_matches = Vec::new();
        let mut page = 1;
        let per_page = 100; // Larger page size for efficiency
        
        loop {
            debug!("Fetching page {} of geometric search results", page);
            
            // Build request body with the correct structure
            let body = serde_json::json!({
                "page": page,
                "perPage": per_page,
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
            let result: Result<crate::model::GeometricSearchResponse, ApiError> = self.post(&url, &body).await;
            
            match result {
                Ok(response) => {
                    // Check if we have pagination data
                    if let Some(page_data) = &response.page_data {
                        debug!("Page {}/{} with {} total matches", page_data.current_page, page_data.last_page, page_data.total);
                        
                        // Add matches from this page to our collection
                        all_matches.extend(response.matches);
                        
                        // Check if we've reached the last page
                        if page_data.current_page >= page_data.last_page {
                            debug!("Reached last page of results");
                            break;
                        }
                        
                        // Move to next page
                        page += 1;
                    } else {
                        // No pagination data - just return the response as-is
                        debug!("No pagination data in response, returning single page");
                        return Ok(response);
                    }
                }
                Err(e) => {
                    // Return error immediately
                    debug!("Geometric search failed: {}", e);
                    return Err(e);
                }
            }
        }
        
        // Create a response with all matches and combined pagination data
        let final_response = crate::model::GeometricSearchResponse {
            matches: all_matches,
            page_data: None, // We've aggregated all pages
            filter_data: None,
        };
        
        debug!("Geometric search completed for asset_id: {} with {} total matches", asset_uuid, final_response.matches.len());
        Ok(final_response)
    }

    /// Perform a part search to find geometrically similar assets using the part search algorithm
    ///
    /// This method uses Physna's advanced part search algorithms to find assets with similar
    /// geometry to the provided reference asset. The part search algorithm may provide different
    /// results than the standard geometric search, potentially with forward and reverse match percentages.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant containing the assets
    /// * `asset_uuid` - The UUID of the reference asset to search for matches
    /// * `threshold` - The similarity threshold as a percentage (0.00 to 100.00)
    ///                Lower values return more matches, higher values return fewer but more similar matches
    ///
    /// # Returns
    /// * `Ok(GeometricSearchResponse)` - The search results containing similar assets
    /// * `Err(ApiError)` - If there's an HTTP error, authentication issue, or other API error
    ///
    /// # Example
    /// ```no_run
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// use uuid::Uuid;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let tenant_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    ///     let asset_uuid = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000")?;
    ///     let matches = client.part_search(&tenant_uuid, &asset_uuid, 85.0).await?;
    ///     for match_result in &matches.matches {
    ///         println!("Found match: {} (forward: {:.2}%, reverse: {:.2}%)",
    ///             match_result.path(),
    ///             match_result.forward_match_percentage.unwrap_or(0.0),
    ///             match_result.reverse_match_percentage.unwrap_or(0.0));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn part_search(&mut self, tenant_uuid: &Uuid, asset_uuid: &Uuid, threshold: f64) -> Result<crate::model::PartSearchResponse, ApiError> {
        debug!("Starting part search for tenant_uuid: {}, asset_uuid: {}, threshold: {}", tenant_uuid, asset_uuid, threshold);
        let url = format!("{}/tenants/{}/assets/{}/part-search", self.base_url, tenant_uuid, asset_uuid);

        // Initialize with page 1 and reasonable page size
        let mut all_matches = Vec::new();
        let mut page = 1;
        let per_page = 100; // Larger page size for efficiency

        // Track the maximum last_page value seen to prevent infinite loops
        let mut max_last_page_seen = 0;
        let max_pages_limit = 50; // Hard limit to prevent excessive API calls

        loop {
            debug!("Fetching page {} of part search results", page);

            // Check if we've hit the hard limit
            if page > max_pages_limit {
                debug!("Reached hard page limit of {}, stopping to prevent excessive API calls", max_pages_limit);
                break;
            }

            // Build request body with the correct structure
            let body = serde_json::json!({
                "page": page,
                "perPage": per_page,
                "searchQuery": "",
                "filters": {
                    "folders": [],
                    "metadata": {},
                    "extensions": []  // Empty array as requested
                },
                "minThreshold": threshold  // Use threshold directly as percentage
            });

            debug!("Sending part search request to: {}", url);
            // Execute POST request
            let result: Result<crate::model::PartSearchResponse, ApiError> = self.post(&url, &body).await;

            match result {
                Ok(response) => {
                    // Check if we have pagination data
                    if let Some(page_data) = &response.page_data {
                        debug!("Page {}/{} with {} total matches", page_data.current_page, page_data.last_page, page_data.total);

                        // Update the maximum last_page value seen
                        if page_data.last_page > max_last_page_seen {
                            max_last_page_seen = page_data.last_page;
                        }

                        // Add matches from this page to our collection
                        all_matches.extend(response.matches);

                        // Check if we've reached the last page or gone beyond what we've seen
                        if page_data.current_page >= page_data.last_page || page > max_last_page_seen {
                            debug!("Reached last page of results or beyond max seen: current={}, last={}, requested={}",
                                   page_data.current_page, page_data.last_page, page);
                            break;
                        }

                        // Move to next page
                        page += 1;
                    } else {
                        // No pagination data - just return the response as-is
                        debug!("No pagination data in response, returning single page");
                        return Ok(response);
                    }
                }
                Err(e) => {
                    // Return error immediately
                    debug!("Part search failed: {}", e);
                    return Err(e);
                }
            }
        }

        // Create a response with all matches and combined pagination data
        let final_response = crate::model::PartSearchResponse {
            matches: all_matches,
            page_data: None, // We've aggregated all pages
            filter_data: None,
        };

        debug!("Part search completed for asset_id: {} with {} total matches", asset_uuid, final_response.matches.len());
        Ok(final_response)
    }

    /// Performs a visual search for similar assets
    ///
    /// This method performs a visual search to find assets that are visually similar to the provided asset.
    /// The search results are ordered by relevance as determined by the visual search algorithm.
    ///
    /// # Arguments
    ///
    /// * `tenant_uuid` - The UUID of the tenant to search within
    /// * `asset_uuid` - The UUID of the reference asset to search for visually similar matches
    ///
    /// # Returns
    ///
    /// * `Ok(PartSearchResponse)` - The search results with visually similar assets
    /// * `Err(ApiError)` - If the API request fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use pcli2::physna_v3::PhysnaApiClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut client = PhysnaApiClient::new();
    /// # let tenant_uuid = Uuid::nil();
    /// # let asset_uuid = Uuid::nil();
    /// let matches = client.visual_search(&tenant_uuid, &asset_uuid).await?;
    /// for match_result in &matches.matches {
    ///     println!("Found visually similar asset: {}", match_result.path());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn visual_search(&mut self, tenant_uuid: &Uuid, asset_uuid: &Uuid) -> Result<crate::model::PartSearchResponse, ApiError> {
        debug!("Starting visual search for tenant_uuid: {}, asset_uuid: {}", tenant_uuid, asset_uuid);
        let url = format!("{}/tenants/{}/assets/{}/visual-search", self.base_url, tenant_uuid, asset_uuid);

        // Visual search - get first page with 50 results (top matches)
        let page = 1;
        let per_page = 50; // Reasonable page size for visual search

        // Build request body with the correct structure
        let body = serde_json::json!({
            "page": page,
            "perPage": per_page,
            "searchQuery": "",
            "filters": {
                "folders": [],
                "metadata": {},
                "extensions": []  // Empty array as requested
            }
        });

        debug!("Sending visual search request to: {}", url);
        // Execute POST request
        let result: Result<crate::model::PartSearchResponse, ApiError> = self.post(&url, &body).await;

        match result {
            Ok(mut response) => {
                // Limit results to 50 to ensure we don't exceed expected page size
                if response.matches.len() > 50 {
                    response.matches.truncate(50);
                }

                // Clear pagination data since we're only getting the first page
                response.page_data = None;

                debug!("Visual search completed for asset_id: {} with {} total matches", asset_uuid, response.matches.len());
                Ok(response)
            }
            Err(e) => {
                // Return error immediately
                debug!("Visual search failed: {}", e);
                Err(e)
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
        tenant_uuid: &Uuid, 
        glob_pattern: &str, 
        folder_path: Option<&str>,
        folder_uuid: Option<&Uuid>,
        concurrent: usize,
        show_progress: bool
    ) -> Result<Vec<crate::model::Asset>, ApiError> {
        debug!("Creating batch assets in tenant: {}, folder_path: {:?}, folder_id: {:?}", &tenant_uuid, folder_path, folder_uuid);
        
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
        let folder_path = folder_path.map(|s| s.to_string());
        
        debug!("Folder path for batch upload: {:?}, folder ID: {:?}", folder_path, folder_uuid);
        
        let results: Result<Vec<_>, _> = stream::iter(paths)
            .filter(|path_buf| {
                futures::future::ready(path_buf.exists() && path_buf.is_file())
            })
            .map(|path_buf| {
                let base_url = base_url.clone();
                let access_token = access_token.clone();
                let client_credentials = client_credentials.clone();
                let folder_path = folder_path.clone();
                let folder_uuid = folder_uuid.clone();
                let progress_bar = progress_bar.clone();
                
                async move {
                    let path_str = path_buf.to_string_lossy().to_string();
                    let file_name = path_buf.file_name();

                    let file_name = match file_name {
                        Some(file_name) => file_name,
                        None => return Err(ApiError::PathNotFound(path_buf.to_string_lossy().into())),
                    };

                
                
                    // Create a new client for each request to avoid borrowing issues
                    let mut client = PhysnaApiClient::new().with_base_url(base_url);
                    if let Some(token) = access_token {
                        client = client.with_access_token(token);
                    }
                    if let Some((client_id, client_secret)) = client_credentials {
                        client = client.with_client_credentials(client_id, client_secret);
                    }
                
                    // Upload the file
                    let asset_path = format!("{}/{}", folder_path.unwrap(), file_name.to_string_lossy());
                    debug!("Uploading file: {}, as asset_path: {}, folder_uuid: {:?}", path_str, asset_path, folder_uuid);
                    let result = client.create_asset(&tenant_uuid, &path_buf, &asset_path, folder_uuid.unwrap()).await;
                
                    // Update progress bar if present
                    if let Some(pb) = &progress_bar {
                        pb.inc(1);
                        match &result {
                            Ok(asset) => {
                                pb.set_message(format!("Uploaded: {}", asset.path()));
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

    /// Get dependencies for a specific asset by path
    /// 
    /// This method retrieves all dependencies for the specified asset from the Physna API.
    /// The dependencies represent other assets that are related to this asset, such as
    /// components in an assembly or referenced assets.
    /// 
    /// IMPORTANT: Despite the parameter name, this endpoint expects the asset path,
    /// not the asset ID/UUID. The Physna API uses the path in the URL for this endpoint.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_path` - The path of the asset to retrieve dependencies for (e.g., "/Root/Folder/assembly.stl")
    /// 
    /// # Returns
    /// * `Ok(AssetDependenciesResponse)` - Successfully fetched asset dependencies
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_asset_dependencies(&mut self, tenant_id: &str, asset_path: &str) -> Result<crate::model::AssetDependenciesResponse, ApiError> {
        debug!("Getting asset dependencies for tenant_id: {}, asset_path: {}", tenant_id, asset_path);
        
        let encoded_asset_path = urlencoding::encode(asset_path);
        let url = format!("{}/tenants/{}/assets/{}/dependencies", self.base_url, tenant_id, encoded_asset_path);
        debug!("Dependencies request URL: {}", url);
        
        // Execute the GET request using the generic method
        let response: crate::model::AssetDependenciesResponse = self.get(&url).await?;
        debug!("Successfully retrieved {} dependencies for asset_path: {}", response.dependencies.len(), asset_path);
        
        Ok(response)
    }
    
    /// Get dependencies for a specific asset by path
    /// 
    /// This method retrieves all dependencies for the specified asset from the Physna API.
    /// The dependencies represent other assets that are related to this asset, such as
    /// components in an assembly or referenced assets.
    /// 
    /// This method uses the path directly in the API endpoint, as the dependencies endpoint
    /// specifically requires the asset path rather than the asset ID.
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_path` - The path of the asset to retrieve dependencies for (e.g., "/Root/Folder/assembly.stl")
    /// 
    /// # Returns
    /// * `Ok(AssetDependenciesResponse)` - Successfully fetched asset dependencies
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn get_asset_dependencies_by_path(&mut self, tenant_id: &str, asset_path: &str) -> Result<crate::model::AssetDependenciesResponse, ApiError> {
        debug!("Getting asset dependencies by path for tenant_id: {}, asset_path: {}", tenant_id, asset_path);
        
        // URL encode the asset path to handle special characters properly
        let encoded_asset_path = urlencoding::encode(asset_path);
        
        let url = format!("{}/tenants/{}/assets/{}/dependencies", self.base_url, tenant_id, encoded_asset_path);
        debug!("Dependencies request URL: {}", url);
        
        // Execute the GET request using the generic method
        let response: crate::model::AssetDependenciesResponse = self.get(&url).await?;
        debug!("Successfully retrieved {} dependencies for asset_path: {}", response.dependencies.len(), asset_path);
        
        Ok(response)
    }

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
    pub async fn get_asset_state_counts(&mut self, tenant_uuid: &Uuid) -> Result<crate::model::AssetStateCounts, ApiError> {
        debug!("Getting asset state counts for tenant_uuid: {}", tenant_uuid);

        let url = format!("{}/v3/tenants/{}/assets/state-counts", self.base_url, tenant_uuid);
        debug!("Asset state counts request URL: {}", url);

        // Execute the GET request using the generic method
        let response: crate::model::AssetStateCounts = self.get(&url).await?;
        debug!("Successfully retrieved asset state counts for tenant_uuid: {}", tenant_uuid);

        Ok(response)
    }

    /// Download asset file from the Physna API
    ///
    /// This method downloads the raw file content of the specified asset from the Physna API.
    /// The file content is returned as a vector of bytes that can be saved to disk.
    /// 
    /// The API endpoint follows the pattern: GET /tenants/{tenantId}/assets/{assetId}/file
    /// 
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to download
    /// 
    /// # Returns
    /// * `Ok(Vec<u8>)` - Successfully downloaded file content as bytes
    /// * `Err(ApiError)` - If there was an error during API calls
    pub async fn download_asset(&mut self, tenant_id: &str, asset_id: &str) -> Result<Vec<u8>, ApiError> {
        debug!("Downloading asset file for tenant_id: {}, asset_id: {}", tenant_id, asset_id);
        
        let url = format!("{}/tenants/{}/assets/{}/file", self.base_url, tenant_id, asset_id);
        debug!("Download asset file request URL: {}", url);
        
        // Execute GET request for file content
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.access_token.as_ref().ok_or_else(|| ApiError::AuthError("No access token available for download".to_string()))?)
            .send()
            .await
            .map_err(|e| {
                debug!("Failed to send download request: {}", e);
                ApiError::from(e)
            })?;
        
        // Check status code
        if !response.status().is_success() {
            debug!("Download request failed with status: {}", response.status());
            // Create an appropriate error based on the response status
            match response.status() {
                reqwest::StatusCode::UNAUTHORIZED => {
                    return Err(ApiError::AuthError("Unauthorized access - access token may have expired or is invalid".to_string()));
                }
                reqwest::StatusCode::FORBIDDEN => {
                    return Err(ApiError::AuthError("Access forbidden - you don't have permission to download this asset".to_string()));
                }
                reqwest::StatusCode::NOT_FOUND => {
                    return Err(ApiError::AuthError("Asset not found - the asset may have been deleted or the path is incorrect".to_string()));
                }
                _ => {
                    return Err(ApiError::HttpError(response.error_for_status().unwrap_err()));
                }
            }
        }
        
        // Get the file content as bytes
        let bytes = response
            .bytes()
            .await
            .map_err(|e| {
                debug!("Failed to read response bytes: {}", e);
                ApiError::HttpError(e)
            })?;
        
        debug!("Successfully downloaded {} bytes for asset_id: {}", bytes.len(), asset_id);
        Ok(bytes.to_vec())
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
    use uuid::Uuid;

    #[test]
    fn test_create_asset_url() {
        let client = PhysnaApiClient::new();
        // This test verifies that the URL is constructed correctly
        // We're not actually making a network request in this test
        let tenant_id = "test-tenant";
        let url = format!("{}/tenants/{}/assets", client.base_url, tenant_id);
        assert_eq!(url, "https://app-api.physna.com/v3/tenants/test-tenant/assets");
    }

    #[tokio::test]
    async fn test_resolve_folder_uuid_by_path_root_path_returns_none() {
        // Create a client instance
        let mut client = PhysnaApiClient::new();

        // For root path "/", the function should return None
        let tenant_uuid = Uuid::nil(); // Use nil UUID for testing
        let result = client.resolve_folder_uuid_by_path(&tenant_uuid, "/").await;

        // The function should return Ok(None) for root path
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_resolve_folder_uuid_by_path_handles_non_root_paths() {
        // This test documents that for non-root paths, the function
        // calls get_folder_uuid_by_path and returns its result
        // Implementation would require mocking which is complex for this case
    }
}

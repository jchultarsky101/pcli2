use crate::auth::AuthClient;
use crate::model::{CurrentUserResponse, FolderListResponse};
use reqwest;
use serde_json;
use tracing::{debug, trace};

/// Error emitted by the Physna V3 Api
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Request failed after retry: {0}")]
    RetryFailed(String),
}

pub struct PhysnaApiClient {
    base_url: String,
    access_token: Option<String>,
    client_credentials: Option<(String, String)>, // (client_id, client_secret)
    http_client: reqwest::Client,
}

impl PhysnaApiClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://app-api.physna.com/v3".to_string(),
            access_token: None,
            client_credentials: None,
            http_client: reqwest::Client::new(),
        }
    }
    
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn with_access_token(mut self, token: String) -> Self {
        self.access_token = Some(token);
        self
    }
    
    pub fn with_client_credentials(mut self, client_id: String, client_secret: String) -> Self {
        self.client_credentials = Some((client_id, client_secret));
        self
    }
    
    /// Attempt to refresh the access token using client credentials
    async fn refresh_token(&mut self) -> Result<(), ApiError> {
        if let Some((client_id, client_secret)) = &self.client_credentials {
            trace!("Refreshing access token");
            let auth_client = AuthClient::new(client_id.clone(), client_secret.clone());
            match auth_client.get_access_token().await {
                Ok(new_token) => {
                    debug!("Successfully refreshed access token");
                    self.access_token = Some(new_token);
                    Ok(())
                }
                Err(e) => {
                    Err(ApiError::AuthError(format!("Failed to refresh token: {}", e)))
                }
            }
        } else {
            Err(ApiError::AuthError("No client credentials available for token refresh".to_string()))
        }
    }
    
    /// Execute an HTTP request with automatic token refresh on 401/403 errors
    async fn execute_request<T, F>(&mut self, request_builder: F) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        // First attempt with current token
        let mut request = request_builder(&self.http_client);
        
        // Add access token if available
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        trace!("Executing request");
        let response = request.send().await?;
        
        // Check if we need to retry due to authentication issues
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || 
           response.status() == reqwest::StatusCode::FORBIDDEN {
            debug!("Received authentication error ({}), attempting token refresh", response.status());
            
            // Try to refresh the token
            if let Err(refresh_error) = self.refresh_token().await {
                return Err(ApiError::RetryFailed(format!(
                    "Original error: {}, Refresh failed: {}", 
                    response.status(), 
                    refresh_error
                )));
            }
            
            // Retry the request with the new token
            debug!("Retrying request with refreshed token");
            let mut retry_request = request_builder(&self.http_client);
            
            if let Some(token) = &self.access_token {
                retry_request = retry_request.header("Authorization", format!("Bearer {}", token));
            }
            
            let retry_response = retry_request.send().await?;
            
            if retry_response.status().is_success() {
                let result: T = retry_response.json().await?;
                Ok(result)
            } else {
                Err(ApiError::RetryFailed(format!(
                    "Original error: {}, Retry failed with status: {}", 
                    response.status(), 
                    retry_response.status()
                )))
            }
        } else if response.status().is_success() {
            match response.json::<T>().await {
                Ok(result) => Ok(result),
                Err(e) => Err(ApiError::HttpError(e.into()))
            }
        } else {
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

    pub async fn get_current_user(&mut self) -> Result<CurrentUserResponse, ApiError> {
        let url = format!("{}/users/me", self.base_url);
        self.get(&url).await
    }

    pub async fn list_tenants(&mut self) -> Result<Vec<crate::model::TenantSetting>, ApiError> {
        let user = self.get_current_user().await?;
        Ok(user.user.settings)
    }
    
    pub async fn list_folders(&mut self, tenant_id: &str, page: Option<u32>, per_page: Option<u32>) -> Result<FolderListResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_id);
        
        // Add query parameters if provided
        let mut query_params = Vec::new();
        if let Some(page) = page {
            query_params.push(("page", page.to_string()));
        }
        if let Some(per_page) = per_page {
            query_params.push(("per_page", per_page.to_string()));
        }
        
        let url = if !query_params.is_empty() {
            format!("{}?{}", url, serde_urlencoded::to_string(query_params).unwrap())
        } else {
            url
        };
        
        self.get(&url).await
    }
    
    pub async fn get_folder(&mut self, tenant_id: &str, folder_id: &str) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_id, folder_id);
        // The API returns a SingleFolderResponse with a "folder" field
        let response: crate::model::SingleFolderResponse = self.get(&url).await?;
        Ok(response.folder)
    }
    
    pub async fn create_folder(&mut self, tenant_id: &str, name: &str, parent_folder_id: Option<&str>) -> Result<crate::model::FolderResponse, ApiError> {
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_id);
        
        let mut body = serde_json::json!({
            "name": name
        });
        
        // Add parent folder ID if provided
        if let Some(parent_id) = parent_folder_id {
            body["parentFolderId"] = serde_json::Value::String(parent_id.to_string());
        }
        
        // The API returns a SingleFolderResponse with a "folder" field
        let response: crate::model::SingleFolderResponse = self.post(&url, &body).await?;
        Ok(response.folder)
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
    
    pub async fn delete_folder(&mut self, tenant_id: &str, folder_id: &str) -> Result<(), ApiError> {
        let url = format!("{}/tenants/{}/folders/{}", self.base_url, tenant_id, folder_id);
        self.delete(&url).await
    }
}

impl Default for PhysnaApiClient {
    fn default() -> Self {
        Self::new()
    }
}
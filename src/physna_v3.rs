use crate::model::{CurrentUserResponse, FolderListResponse};
use reqwest;
use serde_json;

/// Error emitted by the Physna V3 Api
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub struct PhysnaApiClient {
    base_url: String,
    access_token: Option<String>,
}

impl PhysnaApiClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://app-api.physna.com/v3".to_string(),
            access_token: None,
        }
    }

    pub fn with_access_token(mut self, token: String) -> Self {
        self.access_token = Some(token);
        self
    }

    pub async fn get_current_user(&self) -> Result<CurrentUserResponse, ApiError> {
        let client = reqwest::Client::new();
        let url = format!("{}/users/me", self.base_url);
        
        let mut request = client.get(&url);
        
        // Add access token if available
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;

        if response.status().is_success() {
            let user_response: CurrentUserResponse = response.json().await?;
            Ok(user_response)
        } else {
            Err(ApiError::HttpError(response.error_for_status().unwrap_err()))
        }
    }

    pub async fn list_tenants(&self) -> Result<Vec<crate::model::TenantSetting>, ApiError> {
        let user = self.get_current_user().await?;
        Ok(user.user.settings)
    }
    
    pub async fn list_folders(&self, tenant_id: &str) -> Result<FolderListResponse, ApiError> {
        let client = reqwest::Client::new();
        let url = format!("{}/tenants/{}/folders", self.base_url, tenant_id);
        
        let mut request = client.get(&url);
        
        // Add access token if available
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;

        if response.status().is_success() {
            let folder_list_response: FolderListResponse = response.json().await?;
            Ok(folder_list_response)
        } else {
            Err(ApiError::HttpError(response.error_for_status().unwrap_err()))
        }
    }
}

impl Default for PhysnaApiClient {
    fn default() -> Self {
        Self::new()
    }
}
use reqwest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Token validation error: {0}")]
    TokenValidationError(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenResponse {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "expires_in")]
    pub expires_in: u64,
    #[serde(rename = "token_type")]
    pub token_type: String,
}

pub struct AuthClient {
    token_url: String,
    client_id: String,
    client_secret: String,
}

impl AuthClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            token_url: "https://physna-app.auth.us-east-2.amazoncognito.com/oauth2/token".to_string(),
            client_id,
            client_secret,
        }
    }

    pub async fn get_access_token(&self) -> Result<String, AuthError> {
        let client = reqwest::Client::new();
        
        // For client credentials flow, we don't need to specify a scope
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
        ];

        let response = client
            .post(&self.token_url)
            .form(&params)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .send()
            .await?;

        if response.status().is_success() {
            match response.json::<TokenResponse>().await {
                Ok(token_response) => Ok(token_response.access_token),
                Err(e) => Err(AuthError::HttpError(e.into()))
            }
        } else {
            let status = response.status();
            // Try to get error details - first as text, then try to parse as JSON if needed
            let error_text = match response.text().await {
                Ok(text) => text,
                Err(_) => "Unknown error".to_string()
            };
            
            // Try to parse as JSON for better formatting
            let error_details = match serde_json::from_str::<serde_json::Value>(&error_text) {
                Ok(error_json) => format!("{:?}", error_json),
                Err(_) => error_text
            };
            
            Err(AuthError::AuthFailed(format!("HTTP {}: {}", status, error_details)))
        }
    }
}
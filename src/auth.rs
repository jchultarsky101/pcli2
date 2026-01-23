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

    pub fn new_with_configuration(client_id: String, client_secret: String, configuration: &crate::configuration::Configuration) -> Self {
        Self {
            token_url: configuration.get_auth_base_url(),
            client_id,
            client_secret,
        }
    }

    pub async fn get_access_token(&self) -> Result<String, AuthError> {
        let client = reqwest::Client::builder()
            .user_agent("PCLI2")
            .build()?;

        // Add tracing to see which URL is being used
        tracing::debug!("Authenticating with token URL: {}", &self.token_url);
        tracing::debug!("Client ID: {}", &self.client_id);

        // For OAuth 2.0 client credentials flow, use basic authentication with client credentials
        // This is the standard approach for OAuth 2.0 client credentials grant
        let params = [
            ("grant_type", "client_credentials"),
            // Some OAuth 2.0 implementations expect client_id in the body as well
            ("client_id", &self.client_id),
        ];

        let response = client
            .post(&self.token_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .send()
            .await?;

        tracing::debug!("Authentication response status: {}", response.status());

        if response.status().is_success() {
            match response.json::<TokenResponse>().await {
                Ok(token_response) => {
                    tracing::debug!("Authentication successful, received token");
                    Ok(token_response.access_token)
                },
                Err(e) => Err(AuthError::HttpError(e))
            }
        } else {
            let status = response.status();
            // Try to get error details - first as text, then try to parse as JSON if needed
            let error_text = match response.text().await {
                Ok(text) => text,
                Err(_) => "Unknown error".to_string()
            };

            tracing::debug!("Authentication failed with error text: {}", &error_text);

            // Try to parse as JSON for better formatting
            let error_details = match serde_json::from_str::<serde_json::Value>(&error_text) {
                Ok(error_json) => {
                    // Extract specific error information for better user messages
                    if let Some(error_val) = error_json.get("error") {
                        let error_str = error_val.as_str().unwrap_or("unknown");

                        match error_str {
                            "invalid_client" => {
                                "Invalid client credentials. Please check your client ID and secret.".to_string()
                            },
                            "invalid_grant" => {
                                "Invalid grant. The authorization grant or refresh token is invalid.".to_string()
                            },
                            "unauthorized_client" => {
                                "Unauthorized client. The client is not authorized to use this authorization grant type.".to_string()
                            },
                            "invalid_request" => {
                                "Invalid request. The request is missing required parameters or contains invalid parameters.".to_string()
                            },
                            _ => format!("{:?}", error_json),
                        }
                    } else {
                        format!("{:?}", error_json)
                    }
                },
                Err(_) => error_text
            };

            Err(AuthError::AuthFailed(format!("HTTP {} {}", status, error_details)))
        }
    }
}


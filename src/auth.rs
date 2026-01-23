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
            // Get the full error response body for detailed logging
            let error_body = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    tracing::error!("Failed to read error response body: {}", e);
                    "Unable to read error response body".to_string()
                }
            };

            // Log the full error response for debugging
            tracing::error!("Authentication request failed with status {}: {}", status, &error_body);

            // Try to parse as JSON for better formatting
            let error_details = match serde_json::from_str::<serde_json::Value>(&error_body) {
                Ok(error_json) => {
                    tracing::debug!("Parsed error JSON: {:?}", error_json);

                    // Extract specific error information for better user messages
                    if let Some(error_val) = error_json.get("error") {
                        let error_str = error_val.as_str().unwrap_or("unknown");

                        // Also extract error description if available
                        let error_description = if let Some(desc_val) = error_json.get("error_description") {
                            format!(" - {}", desc_val.as_str().unwrap_or(""))
                        } else {
                            "".to_string()
                        };

                        match error_str {
                            "invalid_client" => {
                                tracing::error!("Invalid client credentials. Client ID: {}", &self.client_id);
                                format!("Invalid client credentials{}. Please check your client ID and secret.", error_description)
                            },
                            "invalid_grant" => {
                                format!("Invalid grant{}. The authorization grant or refresh token is invalid.", error_description)
                            },
                            "unauthorized_client" => {
                                format!("Unauthorized client{}. The client is not authorized to use this authorization grant type.", error_description)
                            },
                            "invalid_request" => {
                                format!("Invalid request{}. The request is missing required parameters or contains invalid parameters.", error_description)
                            },
                            _ => format!("{}{}", error_str, error_description),
                        }
                    } else {
                        error_body.clone() // Return the raw error body if no specific error field
                    }
                },
                Err(json_err) => {
                    tracing::warn!("Failed to parse error response as JSON: {}. Raw error: {}", json_err, &error_body);
                    error_body.clone() // Return the raw error body if JSON parsing fails
                }
            };

            Err(AuthError::AuthFailed(format!("HTTP {} {}", status, error_details)))
        }
    }
}


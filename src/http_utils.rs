//! HTTP utilities for the Physna CLI client.
//!
//! This module provides common HTTP request handling utilities to improve
//! code reuse and consistency across different API clients in the application.

use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::{debug, error, trace};

/// Configuration for HTTP requests with common settings
#[derive(Debug, Clone)]
pub struct HttpRequestConfig {
    /// Base URL for the API
    pub base_url: String,
    /// Default headers to include with all requests
    pub default_headers: HashMap<String, String>,
    /// Request timeout in seconds
    pub timeout: u64,
    /// Whether to automatically retry on certain error codes
    pub retry_on_auth_error: bool,
    /// Timeout for upload operations in seconds (defaults to timeout if not set)
    pub upload_timeout: Option<u64>,
    /// Timeout for download operations in seconds (defaults to timeout if not set)
    pub download_timeout: Option<u64>,
    /// Timeout for search operations in seconds (defaults to timeout if not set)
    pub search_timeout: Option<u64>,
}

impl Default for HttpRequestConfig {
    fn default() -> Self {
        let mut default_headers = HashMap::new();
        default_headers.insert("User-Agent".to_string(), "PCLI2".to_string());

        Self {
            base_url: "https://app-api.physna.com/v3".to_string(),
            default_headers,
            timeout: 1800, // 30 minutes (1800 seconds)
            retry_on_auth_error: true,
            upload_timeout: Some(1800), // 30 minutes (1800 seconds) for upload operations
            download_timeout: Some(1800), // 30 minutes (1800 seconds) for download operations
            search_timeout: Some(1800), // 30 minutes (1800 seconds) for search operations
        }
    }
}

impl HttpRequestConfig {
    pub fn from_configuration(configuration: &crate::configuration::Configuration) -> Self {
        let mut default_headers = HashMap::new();
        default_headers.insert("User-Agent".to_string(), "PCLI2".to_string());

        Self {
            base_url: configuration.get_api_base_url(),
            default_headers,
            timeout: 1800, // 30 minutes (1800 seconds)
            retry_on_auth_error: true,
            upload_timeout: Some(1800), // 30 minutes (1800 seconds) for upload operations
            download_timeout: Some(1800), // 30 minutes (1800 seconds) for download operations
            search_timeout: Some(1800), // 30 minutes (1800 seconds) for search operations
        }
    }
}

use std::sync::Arc;

/// HTTP client wrapper with common request handling logic
#[derive(Clone)]
pub struct HttpClient {
    /// The reqwest client instance
    pub client: Arc<Client>,
    /// Configuration for the HTTP client
    config: HttpRequestConfig,
}

impl HttpClient {
    /// Get a reference to the HTTP client configuration
    pub fn config(&self) -> &HttpRequestConfig {
        &self.config
    }

    /// Create a new HTTP client with the given configuration
    pub fn new(
        config: HttpRequestConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(Self {
            client: Arc::new(client),
            config,
        })
    }

    /// Create a new HTTP client with a specific timeout
    pub fn new_with_timeout(
        timeout: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let config = HttpRequestConfig {
            base_url: "https://app-api.physna.com/v3".to_string(), // Default base URL
            default_headers: {
                let mut headers = std::collections::HashMap::new();
                headers.insert("User-Agent".to_string(), "PCLI2".to_string());
                headers
            },
            timeout,
            retry_on_auth_error: true,
            upload_timeout: None,
            download_timeout: None,
            search_timeout: None,
        };

        Ok(Self {
            client: Arc::new(client),
            config,
        })
    }

    /// Make a GET request to the specified path with automatic error handling
    pub async fn get<T>(
        &self,
        path: &str,
        auth_token: Option<&str>,
    ) -> Result<T, crate::physna_v3::ApiError>
    where
        T: DeserializeOwned,
    {
        self.execute_request(
            |client_builder| client_builder.get(format!("{}{}", self.config.base_url, path)),
            auth_token,
        )
        .await
    }

    /// Make a POST request to the specified path with JSON body and automatic error handling
    pub async fn post<T, B>(
        &self,
        path: &str,
        body: &B,
        auth_token: Option<&str>,
    ) -> Result<T, crate::physna_v3::ApiError>
    where
        T: DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(
            |client_builder| {
                client_builder
                    .post(format!("{}{}", self.config.base_url, path))
                    .json(body)
            },
            auth_token,
        )
        .await
    }

    /// Make a PUT request to the specified path with JSON body and automatic error handling
    pub async fn put<T, B>(
        &self,
        path: &str,
        body: &B,
        auth_token: Option<&str>,
    ) -> Result<T, crate::physna_v3::ApiError>
    where
        T: DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(
            |client_builder| {
                client_builder
                    .put(format!("{}{}", self.config.base_url, path))
                    .json(body)
            },
            auth_token,
        )
        .await
    }

    /// Make a DELETE request to the specified path with automatic error handling
    pub async fn delete(
        &self,
        path: &str,
        auth_token: Option<&str>,
    ) -> Result<(), crate::physna_v3::ApiError> {
        let mut request = self
            .client
            .delete(format!("{}{}", self.config.base_url, path));

        // Add authorization header if available
        if let Some(token) = auth_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // Add default headers
        for (key, value) in &self.config.default_headers {
            request = request.header(key, value);
        }

        let response = request
            .send()
            .await
            .map_err(crate::physna_v3::ApiError::HttpError)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(crate::physna_v3::ApiError::HttpError(
                response.error_for_status().unwrap_err(),
            ))
        }
    }

    /// Execute an HTTP request with common error handling and optional authentication
    async fn execute_request<F, T>(
        &self,
        request_builder: F,
        auth_token: Option<&str>,
    ) -> Result<T, crate::physna_v3::ApiError>
    where
        F: FnOnce(&Client) -> reqwest::RequestBuilder,
        T: DeserializeOwned,
    {
        let mut request = request_builder(&self.client);

        // Add authorization header if available
        if let Some(token) = auth_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // Add default headers
        for (key, value) in &self.config.default_headers {
            request = request.header(key, value);
        }

        let response = request
            .send()
            .await
            .map_err(crate::physna_v3::ApiError::HttpError)?;

        // Check if we should retry due to authentication issues (401 Unauthorized or 403 Forbidden)
        // We retry on both 401 and 403 as they can both indicate authentication issues
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            debug!(
                "Received authentication error ({}), request should be retried with fresh token",
                response.status()
            );
            Err(crate::physna_v3::ApiError::HttpError(
                response.error_for_status().unwrap_err(),
            ))
        } else if response.status().is_success() {
            // Try to get the raw response text for debugging
            let response_text = response
                .text()
                .await
                .map_err(crate::physna_v3::ApiError::HttpError)?;
            trace!("Raw response text for deserialization: {}", response_text);

            // Try to parse and return the JSON response
            match serde_json::from_str::<T>(&response_text) {
                Ok(result) => Ok(result),
                Err(e) => {
                    error!(
                        "Failed to deserialize response: {}. Raw response: {}",
                        e, response_text
                    );
                    Err(crate::physna_v3::ApiError::JsonError(e))
                }
            }
        } else {
            // For all other errors, return the error status
            Err(crate::physna_v3::ApiError::HttpError(
                response.error_for_status().unwrap_err(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_config_default() {
        let config = HttpRequestConfig::default();
        assert_eq!(config.base_url, "https://app-api.physna.com/v3");
        assert_eq!(config.timeout, 60);
        assert!(config.retry_on_auth_error);
    }

    #[test]
    fn test_http_client_config() {
        let config = HttpRequestConfig::default();
        assert_eq!(config.timeout, 60);
        assert!(config.retry_on_auth_error);
    }
}

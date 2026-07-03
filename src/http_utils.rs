//! HTTP utilities for the Physna CLI client.
//!
//! This module provides common HTTP request handling utilities to improve
//! code reuse and consistency across different API clients in the application.

use rand::Rng;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, trace, warn};

/// Default number of retries for transient failures (after the first attempt).
///
/// Can be overridden with the PCLI2_MAX_RETRIES environment variable;
/// set it to 0 to disable retries entirely.
fn default_max_retries() -> u32 {
    std::env::var("PCLI2_MAX_RETRIES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2)
}

/// HTTP status codes that indicate a transient condition worth retrying:
/// request timeout, rate limiting, and upstream gateway failures.
fn is_transient_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 502 | 503 | 504)
}

/// Network-level errors that are safe to retry for the given request.
///
/// A connect error means the request never reached the server, so it is
/// safe to retry regardless of method. A timeout may fire after the server
/// has started processing the request, so only idempotent requests (GETs)
/// are retried on timeouts - retrying a timed-out POST could apply the
/// operation twice.
fn is_retryable_network_error(error: &reqwest::Error, idempotent: bool) -> bool {
    error.is_connect() || (idempotent && error.is_timeout())
}

/// Compute the delay before the next retry attempt.
///
/// Honors the server's Retry-After header (seconds form, capped at 60s)
/// when present; otherwise applies exponential backoff with jitter
/// starting at 500ms and capped at 10s.
fn retry_delay(response: Option<&reqwest::Response>, attempt: u32) -> Duration {
    if let Some(response) = response {
        if let Some(retry_after) = response.headers().get(reqwest::header::RETRY_AFTER) {
            if let Ok(seconds) = retry_after.to_str().unwrap_or_default().parse::<u64>() {
                return Duration::from_secs(seconds.min(60));
            }
        }
    }

    let base_ms = 500u64.saturating_mul(1u64 << attempt.min(5));
    let jitter_ms = rand::thread_rng().gen_range(0..=250);
    Duration::from_millis(base_ms.min(10_000) + jitter_ms)
}

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
    /// Maximum number of retries for transient failures (0 disables retries)
    pub max_retries: u32,
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
            max_retries: default_max_retries(),
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
            max_retries: default_max_retries(),
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
            max_retries: default_max_retries(),
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
            true,
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
            false,
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
            false,
        )
        .await
    }

    /// Make a DELETE request to the specified path with automatic error handling
    pub async fn delete(
        &self,
        path: &str,
        auth_token: Option<&str>,
    ) -> Result<(), crate::physna_v3::ApiError> {
        let response = self
            .send_with_retry(
                |client| client.delete(format!("{}{}", self.config.base_url, path)),
                auth_token,
                false,
            )
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            // error_for_status() only yields Err for 4xx/5xx; a 1xx/3xx
            // (e.g. an unfollowed redirect) must not panic on unwrap_err.
            match response.error_for_status() {
                Err(e) => Err(crate::physna_v3::ApiError::HttpError(e)),
                Ok(response) => Err(crate::physna_v3::ApiError::ConflictError(format!(
                    "Unexpected HTTP status: {}",
                    response.status()
                ))),
            }
        }
    }

    /// Send a request, retrying transient failures with exponential backoff.
    ///
    /// Transient failures are connection errors, network timeouts (idempotent
    /// requests only - see `is_retryable_network_error`), and the
    /// 408/429/502/503/504 status codes. The Retry-After header is honored
    /// when the server provides one. Non-transient responses (including
    /// other error statuses) are returned to the caller for handling.
    async fn send_with_retry<F>(
        &self,
        request_builder: F,
        auth_token: Option<&str>,
        idempotent: bool,
    ) -> Result<reqwest::Response, crate::physna_v3::ApiError>
    where
        F: Fn(&Client) -> reqwest::RequestBuilder,
    {
        let max_retries = self.config.max_retries;
        let mut attempt: u32 = 0;

        loop {
            let mut request = request_builder(&self.client);

            // Add authorization header if available
            if let Some(token) = auth_token {
                request = request.header("Authorization", format!("Bearer {}", token));
            }

            // Add default headers
            for (key, value) in &self.config.default_headers {
                request = request.header(key, value);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    if is_retryable_network_error(&e, idempotent) && attempt < max_retries {
                        let delay = retry_delay(None, attempt);
                        attempt += 1;
                        warn!(
                            "Transient network error ({}); retrying in {:.1}s (attempt {}/{})",
                            e,
                            delay.as_secs_f32(),
                            attempt,
                            max_retries
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(crate::physna_v3::ApiError::HttpError(e));
                }
            };

            if is_transient_status(response.status()) && attempt < max_retries {
                let delay = retry_delay(Some(&response), attempt);
                attempt += 1;
                warn!(
                    "Server responded with {}; retrying in {:.1}s (attempt {}/{})",
                    response.status(),
                    delay.as_secs_f32(),
                    attempt,
                    max_retries
                );
                tokio::time::sleep(delay).await;
                continue;
            }

            return Ok(response);
        }
    }

    /// Execute an HTTP request with common error handling and optional authentication
    async fn execute_request<F, T>(
        &self,
        request_builder: F,
        auth_token: Option<&str>,
        idempotent: bool,
    ) -> Result<T, crate::physna_v3::ApiError>
    where
        F: Fn(&Client) -> reqwest::RequestBuilder,
        T: DeserializeOwned,
    {
        let response = self
            .send_with_retry(request_builder, auth_token, idempotent)
            .await?;

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
            // For all other errors, return the error status.
            // error_for_status() only yields Err for 4xx/5xx; a 1xx/3xx
            // (e.g. an unfollowed redirect) must not panic on unwrap_err.
            match response.error_for_status() {
                Err(e) => Err(crate::physna_v3::ApiError::HttpError(e)),
                Ok(response) => Err(crate::physna_v3::ApiError::ConflictError(format!(
                    "Unexpected HTTP status: {}",
                    response.status()
                ))),
            }
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
        assert_eq!(config.timeout, 1800);
        assert!(config.retry_on_auth_error);
    }

    #[test]
    fn test_http_client_config() {
        let config = HttpRequestConfig::default();
        assert_eq!(config.timeout, 1800);
        assert!(config.retry_on_auth_error);
    }

    #[test]
    fn test_transient_status_detection() {
        assert!(is_transient_status(reqwest::StatusCode::REQUEST_TIMEOUT));
        assert!(is_transient_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_transient_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_transient_status(
            reqwest::StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(is_transient_status(reqwest::StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_transient_status(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!is_transient_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!is_transient_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
    }

    #[test]
    fn test_retry_delay_backoff_bounds() {
        // Without a Retry-After header, backoff grows exponentially
        // (500ms base doubling per attempt) plus up to 250ms of jitter,
        // capped at 10s base.
        let first = retry_delay(None, 0);
        assert!(first >= Duration::from_millis(500));
        assert!(first <= Duration::from_millis(750));

        let second = retry_delay(None, 1);
        assert!(second >= Duration::from_millis(1000));
        assert!(second <= Duration::from_millis(1250));

        let capped = retry_delay(None, 30);
        assert!(capped <= Duration::from_millis(10_250));
    }
}

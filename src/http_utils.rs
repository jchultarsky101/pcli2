//! HTTP transport for the Physna CLI client.
//!
//! One place owns how a request goes out: the client construction (timeouts, user
//! agent), and [`HttpClient::send_with_retry`], which retries transient failures
//! with backoff. Every request the API client makes goes through it, so a 503 or a
//! `Retry-After` is handled the same way for a search, an upload and a download.

use rand::Rng;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Identifies pcli2 traffic to the API, with the version so support can tell builds apart.
pub const USER_AGENT: &str = concat!("PCLI2/", env!("CARGO_PKG_VERSION"));

/// How long to wait for a TCP/TLS connection before giving up.
///
/// Separate from the total request timeout, which has to allow for multi-gigabyte
/// uploads: a host that does not answer at all should fail in seconds, not in the
/// thirty minutes a legitimate transfer is allowed.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a connection may sit with no bytes arriving before it is treated as dead.
///
/// A slow transfer keeps delivering bytes and never trips this; a server that has
/// stopped responding mid-request does, well before the total timeout.
const READ_TIMEOUT: Duration = Duration::from_secs(300);

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

/// Default total request timeout in seconds.
///
/// The default is intentionally long (30 minutes) because uploads and
/// downloads of very large model files legitimately take that long. Users
/// working with small files can lower it with the PCLI2_TIMEOUT environment
/// variable (seconds). A hung connection is caught much earlier by the
/// connect and read timeouts, which are not affected by this value.
fn default_timeout() -> u64 {
    std::env::var("PCLI2_TIMEOUT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&seconds| seconds > 0)
        .unwrap_or(1800)
}

/// HTTP status codes that indicate a transient condition worth retrying:
/// request timeout, rate limiting, and upstream gateway failures.
pub(crate) fn is_transient_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 502 | 503 | 504)
}

/// Network-level errors that are safe to retry for the given request.
///
/// A connect error means the request never reached the server, so it is
/// safe to retry regardless of method. A timeout may fire after the server
/// has started processing the request, so only idempotent requests are
/// retried on timeouts - retrying a timed-out POST could apply the
/// operation twice.
fn is_retryable_network_error(error: &reqwest::Error, idempotent: bool) -> bool {
    error.is_connect() || (idempotent && error.is_timeout())
}

/// Compute the delay before the next retry attempt.
///
/// Honors the server's Retry-After header when present, in either the
/// delay-seconds or the HTTP-date form, capped at 60s; otherwise applies
/// exponential backoff with jitter starting at 500ms and capped at 10s.
fn retry_delay(response: Option<&reqwest::Response>, attempt: u32) -> Duration {
    if let Some(retry_after) = response
        .and_then(|r| r.headers().get(reqwest::header::RETRY_AFTER))
        .and_then(|v| v.to_str().ok())
        .and_then(parse_retry_after)
    {
        return retry_after.min(Duration::from_secs(60));
    }

    let base_ms = 500u64.saturating_mul(1u64 << attempt.min(5));
    let jitter_ms = rand::thread_rng().gen_range(0..=250);
    Duration::from_millis(base_ms.min(10_000) + jitter_ms)
}

/// Parse a `Retry-After` value: a number of seconds, or an HTTP date.
fn parse_retry_after(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let at = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let delta = at.signed_duration_since(chrono::Utc::now());
    Some(Duration::from_secs(delta.num_seconds().max(0) as u64))
}

/// Configuration for HTTP requests with common settings
#[derive(Debug, Clone)]
pub struct HttpRequestConfig {
    /// Base URL for the API
    pub base_url: String,
    /// Default headers to include with all requests
    pub default_headers: HashMap<String, String>,
    /// Total request timeout in seconds
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
        let timeout = default_timeout();
        Self {
            base_url: "https://app-api.physna.com/v3".to_string(),
            default_headers: HashMap::new(),
            timeout,
            retry_on_auth_error: true,
            upload_timeout: Some(timeout),
            download_timeout: Some(timeout),
            search_timeout: Some(timeout),
            max_retries: default_max_retries(),
        }
    }
}

impl HttpRequestConfig {
    pub fn from_configuration(configuration: &crate::configuration::Configuration) -> Self {
        Self {
            base_url: configuration.get_api_base_url(),
            ..Self::default()
        }
    }
}

/// Build the underlying reqwest client with the timeouts and identity every
/// pcli2 request should carry.
fn build_client(total_timeout: u64) -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .timeout(Duration::from_secs(total_timeout))
        .build()
}

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
        let client = build_client(config.timeout)
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
        let client = build_client(timeout)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let config = HttpRequestConfig {
            timeout,
            upload_timeout: None,
            download_timeout: None,
            search_timeout: None,
            ..HttpRequestConfig::default()
        };

        Ok(Self {
            client: Arc::new(client),
            config,
        })
    }

    /// Send a request, retrying transient failures with exponential backoff.
    ///
    /// `request_builder` is called once per attempt and must produce a fresh
    /// request each time (a streamed body cannot be replayed). It may fail, for
    /// example when the file to upload cannot be opened, and that error is
    /// returned as-is.
    ///
    /// Transient failures are connection errors, network timeouts (idempotent
    /// requests only - see `is_retryable_network_error`), and the
    /// 408/429/502/503/504 status codes. The Retry-After header is honored
    /// when the server provides one. Every other response, including 401/403
    /// and other error statuses, is returned to the caller for handling.
    pub(crate) async fn send_with_retry<F>(
        &self,
        mut request_builder: F,
        auth_token: Option<&str>,
        idempotent: bool,
    ) -> Result<reqwest::Response, crate::physna_v3::ApiError>
    where
        F: FnMut(&Client) -> Result<reqwest::RequestBuilder, crate::physna_v3::ApiError>,
    {
        let max_retries = self.config.max_retries;
        let mut attempt: u32 = 0;

        loop {
            let mut request = request_builder(&self.client)?;
            crate::stats::record_request();

            if let Some(token) = auth_token {
                request = request.header("Authorization", format!("Bearer {}", token));
            }

            for (key, value) in &self.config.default_headers {
                request = request.header(key, value);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    if is_retryable_network_error(&e, idempotent) && attempt < max_retries {
                        let delay = retry_delay(None, attempt);
                        attempt += 1;
                        crate::stats::record_retry();
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
                crate::stats::record_retry();
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

            if attempt > 0 {
                debug!("Request succeeded after {} retry attempt(s)", attempt);
            }
            return Ok(response);
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
    fn user_agent_carries_the_version() {
        assert!(USER_AGENT.starts_with("PCLI2/"));
        assert!(USER_AGENT.ends_with(env!("CARGO_PKG_VERSION")));
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
        let first = retry_delay(None, 0);
        assert!(first >= Duration::from_millis(500));
        assert!(first <= Duration::from_millis(750));

        let second = retry_delay(None, 1);
        assert!(second >= Duration::from_millis(1000));
        assert!(second <= Duration::from_millis(1250));

        let capped = retry_delay(None, 30);
        assert!(capped <= Duration::from_millis(10_250));
    }

    #[test]
    fn retry_after_accepts_seconds_and_http_dates() {
        assert_eq!(parse_retry_after("7"), Some(Duration::from_secs(7)));
        assert_eq!(parse_retry_after(" 3 "), Some(Duration::from_secs(3)));
        let soon = chrono::Utc::now() + chrono::Duration::seconds(30);
        let parsed = parse_retry_after(&soon.to_rfc2822()).expect("http date parses");
        assert!(parsed <= Duration::from_secs(30));
        assert!(parsed >= Duration::from_secs(28));
        // A date in the past means "now", never a negative duration.
        let past = chrono::Utc::now() - chrono::Duration::seconds(30);
        assert_eq!(parse_retry_after(&past.to_rfc2822()), Some(Duration::ZERO));
        assert_eq!(parse_retry_after("garbage"), None);
    }

    #[tokio::test]
    async fn transient_status_is_retried_then_succeeds() {
        let mut server = mockito::Server::new_async().await;
        let flaky = server
            .mock("GET", "/flaky")
            .with_status(503)
            .expect(1)
            .create_async()
            .await;
        let ok = server
            .mock("GET", "/flaky")
            .with_status(200)
            .with_body("fine")
            .expect(1)
            .create_async()
            .await;
        // mockito serves mocks in registration order for identical matchers,
        // so the first call sees 503 and the retry sees 200.
        let client = HttpClient::new(HttpRequestConfig {
            max_retries: 2,
            ..HttpRequestConfig::default()
        })
        .unwrap();
        let url = format!("{}/flaky", server.url());
        let response = client
            .send_with_retry(|c| Ok(c.get(&url)), None, true)
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        flaky.assert_async().await;
        ok.assert_async().await;
    }

    #[tokio::test]
    async fn retries_are_exhausted_and_the_last_response_is_returned() {
        let mut server = mockito::Server::new_async().await;
        let m = server
            .mock("GET", "/down")
            .with_status(503)
            .expect(3)
            .create_async()
            .await;
        let client = HttpClient::new(HttpRequestConfig {
            max_retries: 2,
            ..HttpRequestConfig::default()
        })
        .unwrap();
        let url = format!("{}/down", server.url());
        let response = client
            .send_with_retry(|c| Ok(c.get(&url)), None, true)
            .await
            .unwrap();
        assert_eq!(response.status(), 503);
        m.assert_async().await;
    }

    #[tokio::test]
    async fn non_transient_errors_are_not_retried() {
        let mut server = mockito::Server::new_async().await;
        let m = server
            .mock("GET", "/nope")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;
        let client = HttpClient::new(HttpRequestConfig {
            max_retries: 2,
            ..HttpRequestConfig::default()
        })
        .unwrap();
        let url = format!("{}/nope", server.url());
        let response = client
            .send_with_retry(|c| Ok(c.get(&url)), None, true)
            .await
            .unwrap();
        assert_eq!(response.status(), 500);
        m.assert_async().await;
    }

    #[tokio::test]
    async fn bearer_token_and_user_agent_are_sent() {
        let mut server = mockito::Server::new_async().await;
        let m = server
            .mock("GET", "/who")
            .match_header("authorization", "Bearer tok")
            .match_header("user-agent", USER_AGENT)
            .with_status(200)
            .create_async()
            .await;
        let client = HttpClient::new(HttpRequestConfig::default()).unwrap();
        let url = format!("{}/who", server.url());
        client
            .send_with_retry(|c| Ok(c.get(&url)), Some("tok"), true)
            .await
            .unwrap();
        m.assert_async().await;
    }
}

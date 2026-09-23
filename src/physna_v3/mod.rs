use crate::auth::AuthClient;
use crate::http_utils::HttpClient;
use crate::keyring::{Keyring, KeyringError};
use crate::model::{
    AssemblyNode, AssemblyTree, Asset, AssetDependenciesResponse, AssetList, AssetListResponse,
    AssetStateCounts, CurrentUserResponse, FolderList, FolderListResponse, SingleAssetResponse,
    SingleFolderResponse,
};
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use mime_guess;
use reqwest;
use serde_json;
use serde_urlencoded;
use std::path::Path;
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

mod assets;
mod downloads;
mod folders;
mod metadata;
mod reports;
mod search;
mod tenants;

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

    /// The server refused the request with 409. The search endpoints use it to say an
    /// asset is not in a searchable state, which callers treat as a property of the
    /// tenant rather than of the run - so it must never be produced for any other status.
    #[error("Conflict: {0}")]
    ConflictError(String),

    /// The server answered with an error status that has no case of its own: 400, 422,
    /// 429, 5xx and so on. `message` is what the API said (its JSON `message`/`error`
    /// field when present, otherwise the body), and `status` is kept so callers can
    /// decide on the number rather than on the text.
    #[error("HTTP {status} - {message}")]
    HttpStatus { status: u16, message: String },

    #[error("{0}")]
    KeyringError(#[from] KeyringError),

    #[error("Access token not found. Please login first with 'pcli2 auth login'")]
    InvalidToken,

    #[error("Login credentials not provided")]
    MissingCredentials,

    #[error("Path not found: {0}")]
    PathNotFound(String),

    /// A folder path did not resolve to an existing folder
    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    #[error("Invalid path for asset. Check asset name: {0}")]
    InvalidAssetPath(String),

    /// Not found error with a descriptive message
    #[error("Not found error: {0}")]
    NotFoundError(String),

    /// The folder hierarchy needed to enumerate subfolders could not be obtained, so a
    /// recursive listing cannot be performed.
    #[error(
        "could not load the folder hierarchy needed by --recursive: {0}. \
         Without it only the assets directly in the folder can be listed, which is not \
         what was asked for."
    )]
    FolderHierarchyUnavailable(String),

    #[error(
        "Attempting to delete folder that is not empty. If you are sure, use the --force flag"
    )]
    FolderNotEmptyError,

    /// Invalid parameter error with a descriptive message
    #[error("Invalid parameter: {0}")]
    InvalidParameterError(String),

    /// Metadata type mismatch error when trying to update a metadata field with an incompatible type
    #[error("Metadata type mismatch: Cannot update metadata field '{field_name}' with a value of type '{provided_type}'. The field was defined as type '{expected_type}'. Please use a value that matches the field's defined type, or delete and recreate the field with the desired type.")]
    MetadataTypeMismatch {
        field_name: String,
        expected_type: String,
        provided_type: String,
    },
}

impl PhysnaApiClient {
    /// The API base URL this client talks to.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// `try_default` for diagnostics: the same client, but an error is returned as
    /// a plain `ApiError` rather than reported.
    pub fn try_default_quiet() -> Result<Self, ApiError> {
        <Self as TryDefault>::try_default()
    }
}

impl ApiError {
    /// The HTTP status behind this error, when there is one.
    pub fn http_status(&self) -> Option<u16> {
        match self {
            ApiError::HttpStatus { status, .. } => Some(*status),
            ApiError::HttpError(e) => e.status().map(|s| s.as_u16()),
            ApiError::ConflictError(_) => Some(409),
            ApiError::NotFoundError(_) => Some(404),
            ApiError::RetryFailed(message) => Self::retry_status(message)
                .and_then(|status| status.split_whitespace().next()?.parse().ok()),
            _ => None,
        }
    }

    /// True when the request was authenticated and the server still said 403,
    /// whether on the first try or after the token had been renewed.
    pub fn is_forbidden(&self) -> bool {
        self.is_authorization_failure() || self.http_status() == Some(403)
    }

    /// The exit code that describes this error to a script.
    pub fn exit_code(&self) -> crate::exit_codes::PcliExitCode {
        use crate::exit_codes::PcliExitCode;
        match self {
            ApiError::AuthError(_)
            | ApiError::InvalidToken
            | ApiError::MissingCredentials
            | ApiError::KeyringError(_) => PcliExitCode::AuthError,
            ApiError::HttpError(e)
                if e.is_connect() || e.is_timeout() || e.is_request() || e.is_body() =>
            {
                PcliExitCode::NetworkError
            }
            // Rate limited or a server-side failure that outlasted every retry:
            // "try again later", which is what 69 means, rather than a request the
            // API rejected.
            ApiError::HttpStatus { status, .. } if *status == 429 || *status >= 500 => {
                PcliExitCode::TempFail
            }
            ApiError::HttpError(_)
            | ApiError::RetryFailed(_)
            | ApiError::ConflictError(_)
            | ApiError::HttpStatus { .. }
            | ApiError::FolderNotEmptyError
            | ApiError::FolderHierarchyUnavailable(_)
            | ApiError::MetadataTypeMismatch { .. } => PcliExitCode::ApiError,
            ApiError::NotFoundError(_)
            | ApiError::PathNotFound(_)
            | ApiError::FolderNotFound(_)
            | ApiError::InvalidAssetPath(_) => PcliExitCode::NotFound,
            ApiError::JsonError(_) => PcliExitCode::DataError,
            ApiError::IoError(e) => PcliExitCode::for_io_error(e),
            ApiError::GlobError(_)
            | ApiError::GlobPatternError(_)
            | ApiError::InvalidParameterError(_) => PcliExitCode::UsageError,
        }
    }

    /// True when the error indicates an authentication/authorization failure,
    /// including a 401/403 that persisted through the automatic token-refresh
    /// retry (which surfaces as `RetryFailed` rather than `AuthError`).
    /// True when the credentials themselves are unusable, so no later request can
    /// succeed.
    ///
    /// Narrower than [`Self::is_authentication_failure`] on purpose, and the two are
    /// not interchangeable:
    ///
    /// - Use *this* to decide whether to abandon an operation. These three are raised
    ///   when the token *renewal* failed, which is about the session rather than any
    ///   one request.
    /// - Use `is_authentication_failure` to decide whether an error message should
    ///   mention authentication. It also matches a `RetryFailed` whose text contains a
    ///   401/403, which is a useful hint but a poor basis for giving up: a retried
    ///   request can fail 403 because the caller genuinely lacks permission for that
    ///   one asset, or 409 because that one asset is not indexed. Neither says
    ///   anything about the next asset.
    pub fn is_credential_failure(&self) -> bool {
        matches!(
            self,
            ApiError::AuthError(_) | ApiError::InvalidToken | ApiError::MissingCredentials
        )
    }

    pub fn is_authentication_failure(&self) -> bool {
        match self {
            ApiError::AuthError(_) | ApiError::InvalidToken | ApiError::MissingCredentials => true,
            // A `RetryFailed` is only ever produced inside the 401/403 branch of the
            // request path, and its text always embeds that original status - so
            // searching the whole message for "401"/"403" matched *every* one of them
            // unconditionally. What actually matters is how the retry fared: the token
            // was renewed in between, so a retry that is *still* unauthorized means the
            // credentials are the problem, while a retry that failed some other way
            // (409 not indexed, a type conflict, a 5xx) says nothing about them.
            ApiError::RetryFailed(msg) => Self::retry_status_is_unauthorized(msg),
            _ => false,
        }
    }

    /// The status the *retry* came back with, lowercased, excluding the response body.
    ///
    /// `None` when the message does not have the expected shape, which callers should
    /// read conservatively - anything reaching `RetryFailed` got there through the
    /// 401/403 path to begin with.
    fn retry_status(message: &str) -> Option<String> {
        const RETRY_MARKER: &str = "retry failed with status:";
        let lowered = message.to_lowercase();
        let after_retry = lowered.split_once(RETRY_MARKER).map(|(_, rest)| rest)?;
        // Only the status itself - a response body can quote anything.
        Some(
            after_retry
                .split(" and body:")
                .next()
                .unwrap_or(after_retry)
                .to_string(),
        )
    }

    /// Whether the retry was itself rejected as unauthorized or forbidden.
    fn retry_status_is_unauthorized(message: &str) -> bool {
        match Self::retry_status(message) {
            Some(status) => {
                status.contains("401")
                    || status.contains("403")
                    || status.contains("unauthorized")
                    || status.contains("forbidden")
            }
            None => true,
        }
    }

    /// True when the request was authenticated but not *permitted*.
    ///
    /// Physna has no per-asset permissions: an account is an Author, who may write any
    /// asset, or a Viewer, who may write none. So a 403 that survives a token renewal
    /// says something about the account rather than the asset, and every subsequent
    /// write will fail identically - which makes it worth stopping on, and worth
    /// reporting as a role problem rather than a credential one.
    ///
    /// Deliberately excludes a retry that came back 401. That is the token being
    /// rejected outright, which is [`Self::is_credential_failure`] territory and calls
    /// for logging in again rather than for a conversation about roles.
    pub fn is_authorization_failure(&self) -> bool {
        match self {
            ApiError::RetryFailed(msg) => match Self::retry_status(msg) {
                Some(status) => {
                    (status.contains("403") || status.contains("forbidden"))
                        && !status.contains("401")
                        && !status.contains("unauthorized")
                }
                // An unrecognized shape says nothing specific about authorization.
                None => false,
            },
            _ => false,
        }
    }
}

#[cfg(test)]
mod credential_failure_tests {
    use super::*;

    #[test]
    fn only_renewal_failures_count_as_credential_failures() {
        // These are raised when the token renewal itself failed, so nothing later can
        // succeed - the only safe basis for abandoning an operation.
        assert!(ApiError::AuthError("renewal failed".into()).is_credential_failure());
        assert!(ApiError::InvalidToken.is_credential_failure());
        assert!(ApiError::MissingCredentials.is_credential_failure());

        // A retry outcome never is: the renewal demonstrably worked to get that far.
        assert!(!ApiError::RetryFailed("anything".into()).is_credential_failure());
        assert!(!ApiError::ConflictError("Asset not indexed yet".into()).is_credential_failure());
    }

    #[test]
    fn a_retry_that_failed_for_a_non_auth_reason_is_not_an_auth_failure() {
        // The bug: `RetryFailed` is only ever built inside the 401/403 branch, and its
        // text always embeds that original status - so searching the whole message
        // matched every one of them unconditionally. Observed on a live tenant: a
        // stale-token 403 triggers a renewal, and the retry then fails 409 because that
        // asset is not indexed. Nothing to do with credentials.
        let observed = ApiError::RetryFailed(
            "Original error: 403 Forbidden, Retry failed with status: 409 Conflict \
             and body: {\"message\":\"Asset not indexed yet\"}"
                .into(),
        );
        assert!(!observed.is_authentication_failure());
        assert!(!observed.is_credential_failure());
    }

    #[test]
    fn a_retry_that_is_still_unauthorized_is_an_auth_failure() {
        // Renewal succeeded but the fresh token was rejected anyway. This is the case
        // the predicate exists for, and the one a narrow `matches!` on the renewal
        // variants would miss.
        for message in [
            "Original error: 401 Unauthorized, Retry failed with status: 401 Unauthorized and body: {}",
            "Original error: 403 Forbidden, Retry failed with status: 403 Forbidden and body: {}",
        ] {
            assert!(
                ApiError::RetryFailed(message.into()).is_authentication_failure(),
                "{}",
                message
            );
        }
    }

    #[test]
    fn the_response_body_does_not_decide_it() {
        // Only the retry's status counts. A body is free to quote "403" or the word
        // "forbidden" in prose without that meaning the request was unauthorized.
        let noisy_body = ApiError::RetryFailed(
            "Original error: 403 Forbidden, Retry failed with status: 409 Conflict \
             and body: {\"message\":\"forbidden characters in name; see error 403 docs\"}"
                .into(),
        );
        assert!(!noisy_body.is_authentication_failure());
    }

    #[test]
    fn an_unrecognized_retry_message_is_treated_as_auth_related() {
        // If the message shape ever changes, fall back to the conservative reading -
        // it reached `RetryFailed` through the 401/403 path to begin with.
        assert!(ApiError::RetryFailed("something else entirely".into()).is_authentication_failure());
    }

    #[test]
    fn a_persistent_403_is_an_authorization_failure_not_a_credential_one() {
        // Physna has no per-asset permissions, so a 403 that survives a renewal is
        // about the account: a Viewer trying to write. It will fail identically for
        // every remaining asset, and "re-authenticate" is useless advice.
        let forbidden = ApiError::RetryFailed(
            "Original error: 403 Forbidden, Retry failed with status: 403 Forbidden and body: {}"
                .into(),
        );
        assert!(forbidden.is_authorization_failure());
        assert!(
            !forbidden.is_credential_failure(),
            "the credentials worked - the account is not allowed"
        );
        assert!(
            forbidden.is_authentication_failure(),
            "still auth-adjacent for message shaping"
        );
    }

    #[test]
    fn a_persistent_401_is_a_credential_problem_not_an_authorization_one() {
        // The token was rejected outright even after renewal. That calls for logging
        // in again, not for a conversation about roles.
        let unauthorized = ApiError::RetryFailed(
            "Original error: 401 Unauthorized, Retry failed with status: 401 Unauthorized and body: {}"
                .into(),
        );
        assert!(!unauthorized.is_authorization_failure());
        assert!(unauthorized.is_authentication_failure());
    }

    #[test]
    fn a_retry_that_left_the_403_behind_is_not_an_authorization_failure() {
        // The 403 was a stale token; the renewal fixed it and the retry reached the
        // server, which refused for an unrelated reason. Nothing to do with roles.
        let not_indexed = ApiError::RetryFailed(
            "Original error: 403 Forbidden, Retry failed with status: 409 Conflict \
             and body: {\"message\":\"Asset not indexed yet\"}"
                .into(),
        );
        assert!(!not_indexed.is_authorization_failure());
        assert!(!not_indexed.is_credential_failure());
        assert!(!not_indexed.is_authentication_failure());
    }

    #[test]
    fn a_renewal_failure_is_not_an_authorization_failure() {
        // Never route a dead credential to "ask for the Author role".
        assert!(!ApiError::AuthError("renewal failed".into()).is_authorization_failure());
        assert!(!ApiError::InvalidToken.is_authorization_failure());
        assert!(!ApiError::MissingCredentials.is_authorization_failure());
    }

    #[test]
    fn ordinary_errors_are_neither() {
        let conflict = ApiError::ConflictError("Asset not indexed yet".into());
        assert!(!conflict.is_credential_failure());
        assert!(!conflict.is_authentication_failure());
    }
}

#[cfg(test)]
mod shared_token_tests {
    use super::*;

    #[test]
    fn a_clone_sees_a_token_stored_by_the_original() {
        // The bug this replaces: the concurrent commands clone the client once per
        // task, and a renewal inside one task updated only that task's copy - so every
        // task still holding the expired token renewed again for itself.
        let client = PhysnaApiClient::default();
        let clone = client.clone();
        assert!(!clone.has_token());

        client.store_token("renewed".to_string());
        assert_eq!(
            clone.current_token().as_deref(),
            Some("renewed"),
            "a renewal must be visible to clones made before it"
        );
    }

    #[test]
    fn a_token_stored_by_a_clone_is_seen_by_its_siblings() {
        // Renewal happens inside a task, on a clone - so the direction that actually
        // matters is clone-to-everyone-else, not original-to-clone.
        let client = PhysnaApiClient::default();
        let first = client.clone();
        let second = client.clone();

        first.store_token("renewed-by-a-task".to_string());
        assert_eq!(second.current_token().as_deref(), Some("renewed-by-a-task"));
        assert_eq!(client.current_token().as_deref(), Some("renewed-by-a-task"));
    }

    #[test]
    fn variant_clients_share_the_same_token() {
        // The upload/download builders construct a new client with different timeouts.
        // They must not fork the token, or a renewal during an upload would be lost.
        let client = PhysnaApiClient::default();
        let variant = client.for_upload_operations();
        client.store_token("shared".to_string());
        assert_eq!(variant.current_token().as_deref(), Some("shared"));
    }

    #[test]
    fn with_access_token_publishes_to_the_shared_slot() {
        let client = PhysnaApiClient::default().with_access_token("initial".to_string());
        assert_eq!(client.current_token().as_deref(), Some("initial"));
        assert!(client.has_token());
    }
}

pub trait TryDefault: Sized {
    type Error;
    fn try_default() -> Result<Self, Self::Error>;
}

/// The access token, shared by a client and all of its clones.
///
/// A `std::sync` lock rather than a `tokio` one because it is only ever held for a
/// clone or a store, never across an await.
type SharedToken = std::sync::Arc<std::sync::RwLock<Option<String>>>;

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
/// What a batch upload managed to do. Failures are returned, not logged: a caller
/// that only sees the successes cannot tell a complete run from a partial one.
#[derive(Debug, Default)]
pub struct BatchUploadOutcome {
    pub assets: Vec<crate::model::Asset>,
    pub failures: Vec<(std::path::PathBuf, ApiError)>,
}

#[derive(Clone)]
pub struct PhysnaApiClient {
    /// Base URL for the Physna V3 API (e.g., "https://app-api.physna.com/v3")
    base_url: String,

    /// Current access token for API authentication.
    ///
    /// Shared by every clone of this client. The concurrent commands clone it once per
    /// task, and a token renewed inside one task has to be visible to the rest: with a
    /// per-clone copy, every task that started before the renewal kept the stale token
    /// and renewed again for itself, so one expiry produced tens of thousands of
    /// redundant token requests.
    access_token: SharedToken,

    /// Serializes renewals, so a burst of tasks all meeting the same expired token
    /// performs one renewal between them rather than one each.
    renewal: std::sync::Arc<tokio::sync::Mutex<()>>,

    /// Client credentials (client_id, client_secret) for token refresh
    client_credentials: Option<(String, String)>, // (client_id, client_secret)

    /// Auth URL for token refresh operations
    auth_url: String,

    /// HTTP client for making requests
    http_client: HttpClient,

    /// Environment name for keyring storage
    environment_name: String,
}

impl TryDefault for PhysnaApiClient {
    type Error = ApiError;

    fn try_default() -> Result<PhysnaApiClient, ApiError> {
        // Load configuration to get the base URL
        let configuration = crate::configuration::Configuration::load_or_create_default()
            .map_err(|e| ApiError::AuthError(format!("Failed to load configuration: {}", e)))?;

        // Use the active environment name for keyring storage, fallback to "default" if no environment is set
        let environment_name = configuration
            .get_active_environment()
            .unwrap_or_else(|| "default".to_string());

        #[allow(unused_mut)]
        let mut keyring = Keyring::default();
        // Get all environment credentials in a single operation to reduce keyring access calls
        let (access_token, client_id, client_secret) =
            keyring.get_environment_credentials(&environment_name)?;

        match access_token {
            Some(token) => {
                let mut client = PhysnaApiClient::new_with_configuration_and_environment(
                    &configuration,
                    environment_name,
                )
                .with_access_token(token);

                // Try to get client credentials for automatic token refresh
                if let (Some(id), Some(secret)) = (client_id, client_secret) {
                    client = client.with_client_credentials(id, secret);
                    Ok(client)
                } else {
                    Err(ApiError::MissingCredentials)
                }
            }
            None => Err(ApiError::InvalidToken),
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
        let config = crate::http_utils::HttpRequestConfig::default();
        let http_client =
            HttpClient::new(config).expect("Failed to build HTTP client with timeout");

        Self {
            base_url: "https://app-api.physna.com/v3".to_string(),
            access_token: SharedToken::default(),
            renewal: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            client_credentials: None,
            auth_url: "https://physna-app.auth.us-east-2.amazoncognito.com/oauth2/token"
                .to_string(),
            http_client,
            environment_name: "default".to_string(),
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
        let config = crate::http_utils::HttpRequestConfig::from_configuration(configuration);
        let http_client =
            HttpClient::new(config).expect("Failed to build HTTP client with timeout");

        Self {
            base_url: configuration
                .get_api_base_url()
                .trim_end_matches('/')
                .to_string(),
            access_token: SharedToken::default(),
            renewal: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            client_credentials: None,
            auth_url: configuration.get_auth_base_url(),
            http_client,
            environment_name: "default".to_string(), // Default environment name for backward compatibility
        }
    }

    pub fn new_with_configuration_and_environment(
        configuration: &crate::configuration::Configuration,
        environment_name: String,
    ) -> Self {
        let config = crate::http_utils::HttpRequestConfig::from_configuration(configuration);
        let http_client =
            HttpClient::new(config).expect("Failed to build HTTP client with timeout");

        Self {
            base_url: configuration
                .get_api_base_url()
                .trim_end_matches('/')
                .to_string(),
            access_token: SharedToken::default(),
            renewal: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            client_credentials: None,
            auth_url: configuration.get_auth_base_url(),
            http_client,
            environment_name,
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
        // Paths are appended with their own leading slash; a configured URL ending
        // in one produced `https://host/v3//tenants/...`.
        self.base_url = base_url.trim_end_matches('/').to_string();
        self
    }

    /// Set the access token for API authentication
    ///
    /// # Arguments
    /// * `token` - The access token to use for API requests
    ///
    /// # Returns
    /// The updated `PhysnaApiClient` instance with the access token set
    pub fn with_access_token(self, token: String) -> Self {
        self.store_token(token);
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

    /// Set the OAuth token endpoint used for automatic token renewal.
    ///
    /// Tests point this at a mock server; production clients take it from the
    /// configuration.
    pub fn with_auth_url(mut self, auth_url: String) -> Self {
        self.auth_url = auth_url;
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
    pub async fn refresh_token(&mut self) -> Result<(), ApiError> {
        let current = self.current_token();
        self.refresh_token_after(current.as_deref()).await
    }

    /// Renew the access token, unless it has already changed since `used` was sent.
    ///
    /// `used` is the token the failed request carried. Comparing against *that*,
    /// rather than against whatever is current when the renewal is requested, is what
    /// collapses a burst of concurrent 401s into one renewal: a task whose request
    /// went out with the old token, and which only reads the shared slot after
    /// another task has already renewed, sees the new token and stops.
    pub async fn refresh_token_after(&mut self, used: Option<&str>) -> Result<(), ApiError> {
        // Since the token refresh mechanism is not working reliably with this Cognito setup,
        // we'll automatically attempt to re-authenticate using the cached client credentials.
        // If this automatic re-authentication fails, we'll prompt the user to run 'pcli2 auth login'.

        debug!(
            "Attempting to automatically refresh the access token using the cached credentials..."
        );

        // Whatever we were holding when we decided a renewal was needed. If it has
        // changed by the time we get the lock, someone else renewed while we waited and
        // there is nothing left to do - without this, a burst of tasks all meeting the
        // same expired token would queue up and re-authenticate one after another.
        let _renewing = self.renewal.lock().await;
        if self.current_token().as_deref() != used {
            debug!("Another task renewed the access token while we waited; using theirs");
            return Ok(());
        }

        if let Some((client_id, client_secret)) = &self.client_credentials {
            debug!("Attempting automatic re-authentication with cached client credentials");

            // Create a new auth client with the stored credentials and the correct auth URL
            let auth_client = AuthClient::new_with_auth_url(
                client_id.clone(),
                client_secret.clone(),
                &self.auth_url,
            );

            // Attempt to get a new access token
            match auth_client.get_access_token().await {
                Ok(new_token) => {
                    debug!("Successfully obtained new access token automatically");
                    // Update the stored access token
                    self.store_token(new_token.clone());
                    crate::stats::record_renewal();

                    // Save the new token to the keyring immediately to ensure subsequent commands use the fresh token
                    if let Err(e) = self.save_current_token_to_keyring(&self.environment_name) {
                        debug!("Failed to save refreshed token to keyring: {}", e);
                        // Continue anyway - the in-memory token is still valid for this session
                    }

                    Ok(())
                }
                Err(e) => {
                    // If automatic re-authentication fails, prompt the user to log in manually
                    debug!("Automatic re-authentication failed: {}", e);
                    // Keep the cause: a rotated secret, a rate-limited auth endpoint
                    // and a DNS failure all used to print the same "log in again",
                    // which is the right advice for only one of them.
                    Err(ApiError::AuthError(format!(
                        "Automatic re-authentication failed ({}). Please log in again with 'pcli2 auth login'.",
                        e
                    )))
                }
            }
        } else {
            // No client credentials available for automatic re-authentication
            Err(ApiError::AuthError(
                "No client credentials available for automatic re-authentication. Please log in again with 'pcli2 auth login'.".to_string()
            ))
        }
    }

    /// The access token as it stands right now.
    ///
    /// Returns a clone rather than a guard: the lock must never be held across an
    /// await, and every caller wants an owned value for a header anyway.
    fn current_token(&self) -> Option<String> {
        self.access_token
            .read()
            .ok()
            .and_then(|token| token.clone())
    }

    /// Whether a token is currently held.
    fn has_token(&self) -> bool {
        self.current_token().is_some()
    }

    /// Publish a renewed token to this client and every clone sharing it.
    fn store_token(&self, token: String) {
        if let Ok(mut slot) = self.access_token.write() {
            *slot = Some(token);
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
        self.current_token()
    }

    /// Decode the expiration time from a JWT access token
    ///
    /// # Arguments
    /// * `token` - The JWT access token to decode
    ///
    /// # Returns
    /// * `Ok(i64)` - The expiration timestamp (Unix epoch seconds)
    /// * `Err(ApiError)` - If the token cannot be decoded
    pub fn decode_token_expiration(token: &str) -> Result<i64, ApiError> {
        use base64::Engine;

        // Split the JWT into its three parts: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(ApiError::AuthError(
                "Invalid JWT format: token must have exactly 3 parts".to_string(),
            ));
        }

        // Decode the payload (the middle part)
        let payload = parts[1];

        // Add padding if necessary (JWTs use base64url encoding without padding)
        let mut padded_payload = payload.to_string();
        match payload.len() % 4 {
            2 => padded_payload.push_str("=="),
            3 => padded_payload.push('='),
            _ => {}
        }

        // Decode the base64url-encoded payload
        let decoded_bytes = base64::engine::general_purpose::URL_SAFE
            .decode(&padded_payload)
            .map_err(|e| ApiError::AuthError(format!("Failed to decode token payload: {}", e)))?;

        let payload_str = String::from_utf8_lossy(&decoded_bytes);

        // Parse the JSON payload
        let payload_json: serde_json::Value = serde_json::from_str(&payload_str).map_err(|e| {
            ApiError::AuthError(format!("Failed to parse token payload JSON: {}", e))
        })?;

        // Extract the 'exp' claim (expiration time)
        payload_json
            .get("exp")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                ApiError::AuthError(
                    "Token does not contain an 'exp' (expiration) claim".to_string(),
                )
            })
    }

    /// Check if the current access token is expired or about to expire within a threshold
    ///
    /// # Arguments
    /// * `threshold_seconds` - The time threshold in seconds before expiration to consider the token as expiring soon
    ///
    /// # Returns
    /// * `Ok(bool)` - true if the token is expired or will expire within the threshold, false otherwise
    /// * `Err(ApiError)` - If there's no token or it cannot be decoded
    pub fn is_token_expiring_soon(&self, threshold_seconds: u64) -> Result<bool, ApiError> {
        let token = self
            .current_token()
            .ok_or_else(|| ApiError::AuthError("No access token available".to_string()))?;

        let exp_timestamp = Self::decode_token_expiration(&token)?;
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| ApiError::AuthError(format!("Failed to get current time: {}", e)))?
            .as_secs() as i64;

        let time_remaining = exp_timestamp - current_timestamp;

        // Token is expiring soon if time remaining is less than the threshold
        Ok(time_remaining <= threshold_seconds as i64)
    }

    /// Get the time remaining until token expiration in seconds
    ///
    /// # Returns
    /// * `Ok(i64)` - Seconds remaining until expiration (negative if expired)
    /// * `Err(ApiError)` - If there's no token or it cannot be decoded
    pub fn get_token_time_remaining(&self) -> Result<i64, ApiError> {
        let token = self
            .current_token()
            .ok_or_else(|| ApiError::AuthError("No access token available".to_string()))?;

        let exp_timestamp = Self::decode_token_expiration(&token)?;
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| ApiError::AuthError(format!("Failed to get current time: {}", e)))?
            .as_secs() as i64;

        Ok(exp_timestamp - current_timestamp)
    }

    /// Proactively refresh the token if it's about to expire
    ///
    /// This method checks if the token will expire within the given threshold and
    /// refreshes it if necessary. This is useful for long-running operations.
    ///
    /// # Arguments
    /// * `threshold_seconds` - The time threshold in seconds before expiration to trigger a refresh
    ///
    /// # Returns
    /// * `Ok(bool)` - true if the token was refreshed, false if it's still valid
    /// * `Err(ApiError)` - If the refresh failed or there's no token
    pub async fn refresh_token_if_expiring_soon(
        &mut self,
        threshold_seconds: u64,
    ) -> Result<bool, ApiError> {
        if self.is_token_expiring_soon(threshold_seconds)? {
            debug!(
                "Token is expiring soon (within {} seconds), refreshing proactively",
                threshold_seconds
            );
            self.refresh_token().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Seconds of remaining validity below which a token is renewed before use rather
    /// than after a request has already been rejected.
    ///
    /// Comfortably longer than any single request, so a token that passes this check
    /// will still be valid when the response comes back.
    const PROACTIVE_RENEWAL_THRESHOLD_SECONDS: u64 = 60;

    /// Renew the access token before it expires, rather than waiting for a 401.
    ///
    /// Deliberately best-effort: a failure here is swallowed, because the 401 path
    /// still handles renewal and the current token may well work regardless. This can
    /// only avoid work, never cause a request to fail that would otherwise have
    /// succeeded.
    ///
    /// Combined with the shared token and the renewal lock, a burst of concurrent
    /// tasks meeting the same soon-to-expire token performs one renewal between them
    /// instead of each taking its own 401-and-retry round trip.
    async fn renew_token_if_expiring(&mut self) {
        if !self.has_token() {
            return;
        }
        match self.is_token_expiring_soon(Self::PROACTIVE_RENEWAL_THRESHOLD_SECONDS) {
            Ok(true) => {
                if let Err(e) = self.refresh_token().await {
                    debug!(
                        "Proactive token renewal failed ({}); continuing with the current token",
                        e
                    );
                }
            }
            Ok(false) => {}
            Err(e) => {
                debug!("Could not determine token expiry ({}); continuing", e);
            }
        }
    }

    /// Send one authenticated request and hand back its successful response.
    ///
    /// This is the single path every API call takes. It renews the token shortly
    /// before expiry, retries transient failures (connection errors, 408/429/5xx
    /// gateway statuses, `Retry-After`), and on a 401/403 renews the token once and
    /// resends. `build` is called for every attempt so a streamed body starts from
    /// the beginning again.
    ///
    /// Any non-2xx outcome is returned classified: 404 as `NotFoundError`, 409 as
    /// `ConflictError`, a 401/403 that survives renewal as `RetryFailed` (any other
    /// status on the retry is classified as usual), and every
    /// other status as `HttpStatus`, so callers decide on the status rather than on
    /// the text of a message.
    async fn request_with_auth<F>(
        &mut self,
        mut build: F,
        idempotent: bool,
    ) -> Result<reqwest::Response, ApiError>
    where
        F: FnMut(&reqwest::Client) -> Result<reqwest::RequestBuilder, ApiError>,
    {
        // With credentials but no token yet, authenticate up front rather than
        // spending a request to be told 401. Without either, the request still goes
        // out: the server decides, and a 401 then produces the "log in" advice.
        if !self.has_token() && self.client_credentials.is_some() {
            debug!("No access token held; authenticating before the first request");
            self.refresh_token_after(None).await?;
        }
        self.renew_token_if_expiring().await;

        let used_token = self.current_token();
        let response = self
            .http_client
            .send_with_retry(&mut build, used_token.as_deref(), idempotent)
            .await?;
        let first_status = response.status();

        if first_status != reqwest::StatusCode::UNAUTHORIZED
            && first_status != reqwest::StatusCode::FORBIDDEN
        {
            return classify_response(response).await;
        }

        debug!(
            "Received {}; renewing the access token and retrying once",
            first_status
        );
        self.refresh_token_after(used_token.as_deref()).await?;

        let token = self.current_token();
        let retry = self
            .http_client
            .send_with_retry(&mut build, token.as_deref(), idempotent)
            .await?;
        if retry.status().is_success() {
            return Ok(retry);
        }

        // The renewal worked and the retry failed for some other reason: a 404, a
        // 409, a 5xx. That is an ordinary API answer and is classified like one.
        // It used to become a `RetryFailed` carrying raw text, so a 404 exited 102
        // instead of 67 and a 409 no longer read as a conflict to the callers that
        // handle one (a folder that already exists during `folder upload`).
        if retry.status() != reqwest::StatusCode::UNAUTHORIZED
            && retry.status() != reqwest::StatusCode::FORBIDDEN
        {
            return classify_response(retry).await;
        }

        // The wording of this message is part of the contract with
        // `is_authentication_failure` / `is_authorization_failure`, which read the
        // retry's status out of it.
        let retry_status = retry.status();
        let body = read_error_body(retry).await;
        error!(
            "API request failed after retry. Original error: {}, Retry failed with status: {} and body: {}",
            first_status, retry_status, body
        );
        Err(ApiError::RetryFailed(format!(
            "Original error: {}, Retry failed with status: {} and body: {}",
            first_status, retry_status, body
        )))
    }

    /// Execute a request and deserialize its JSON body.
    ///
    /// `idempotent` says whether a timed-out attempt may be resent; a GET may, a
    /// POST that creates something may not.
    async fn execute_request<T, F>(
        &mut self,
        request_builder: F,
        idempotent: bool,
    ) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        F: FnMut(&reqwest::Client) -> Result<reqwest::RequestBuilder, ApiError>,
    {
        let response = self.request_with_auth(request_builder, idempotent).await?;
        let text = response.text().await?;
        trace!("Raw response for deserialization: {}", text);
        trace!("Deserializing into: {}", std::any::type_name::<T>());
        serde_json::from_str::<T>(&text).map_err(|e| {
            error!(
                "Failed to deserialize response into {}: {}. Response starts: {}",
                std::any::type_name::<T>(),
                e,
                excerpt(&text)
            );
            ApiError::JsonError(e)
        })
    }

    /// Generic method to build and execute GET requests
    async fn get<T>(&mut self, url: &str) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.execute_request(|client| Ok(client.get(url)), true)
            .await
    }

    /// POST a request that creates or changes something.
    ///
    /// Not resent after a timeout or a gateway error (502/504), because the first
    /// attempt may already have done the work.
    async fn post<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.post_with(url, body, false).await
    }

    /// POST a request that only reads: a search, a batch lookup.
    ///
    /// These use POST to carry a body, but repeating one changes nothing, so it is
    /// retried like a GET.
    async fn post_query<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.post_with(url, body, true).await
    }

    async fn post_with<T, B>(
        &mut self,
        url: &str,
        body: &B,
        idempotent: bool,
    ) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        // Serialising the body only to log it is wasted work on every search page
        // unless trace logging is actually on.
        if tracing::enabled!(tracing::Level::TRACE) {
            let body_json = serde_json::to_string_pretty(body)
                .unwrap_or_else(|_| "Unable to serialize body".to_string());
            trace!("POST request to {}: {}", url, body_json);
        }

        let result = self
            .execute_request(|client| Ok(client.post(url).json(body)), idempotent)
            .await;

        // Log the response for debugging
        match &result {
            Ok(_) => trace!("POST request to {} succeeded", url),
            Err(e) => trace!("POST request to {} failed: {}", url, e),
        }

        result
    }

    /// Generic method to build and execute PATCH requests
    #[allow(dead_code)]
    async fn patch<T, B>(&mut self, url: &str, body: &B) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        self.execute_request(|client| Ok(client.patch(url).json(body)), false)
            .await
    }

    /// Generic method to build and execute DELETE requests with automatic token refresh
    async fn delete(&mut self, url: &str) -> Result<(), ApiError> {
        // Callers pass a path relative to the API base (`/tenants/...`); an absolute
        // URL is used as given.
        let url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("{}{}", self.base_url, url)
        };
        self.request_with_auth(|client| Ok(client.delete(&url)), true)
            .await
            .map(|_| ())
    }

    // Asset operations

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
    async fn list_assets_by_parent_folder_uuid_with_pagination(
        &mut self,
        tenant_uuid: &Uuid,
        folder_uuid: Option<&Uuid>,
        page: usize,
        per_page: usize,
    ) -> Result<AssetListResponse, ApiError> {
        let url = match folder_uuid {
            Some(folder_uuid) => format!(
                "{}/tenants/{}/folders/{}/contents",
                self.base_url, tenant_uuid, folder_uuid
            ),
            None => format!(
                "{}/tenants/{}/folders/root/contents",
                self.base_url, tenant_uuid
            ),
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

    pub(crate) fn get_parent_folder_path<S: AsRef<str>>(asset_path: S) -> Result<String, ApiError> {
        let asset_path = asset_path.as_ref();
        let path = Path::new(asset_path);
        let parent = path
            .parent()
            .ok_or_else(|| ApiError::InvalidAssetPath(asset_path.to_owned()))?;

        // Convert the parent path to string
        let parent_str = parent
            .to_str()
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

    pub(crate) fn asset_name_from_path(path: &str) -> Option<String> {
        Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }

    /// The type a new metadata field is registered with: the declared type when
    /// the row gives one, else inferred from the value (defaulting to text).
    fn new_field_type<'a>(
        key: &str,
        value: &serde_json::Value,
        declared_types: Option<&'a std::collections::HashMap<String, String>>,
    ) -> &'a str {
        declared_types
            .and_then(|m| m.get(key))
            .map(|s| s.as_str())
            .unwrap_or_else(|| match Self::infer_json_value_type(value).as_str() {
                "number" => "number",
                "boolean" => "boolean",
                _ => "text",
            })
    }

    /// Infer the type of a JSON value for metadata type checking
    ///
    /// # Arguments
    /// * `value` - The JSON value to infer the type from
    ///
    /// # Returns
    /// * `String` - The inferred type ("text", "number", or "boolean")
    fn infer_json_value_type(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(_) => "text".to_string(),
            serde_json::Value::Number(_) => "number".to_string(),
            serde_json::Value::Bool(_) => "boolean".to_string(),
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Array(_) => "array".to_string(),
            serde_json::Value::Object(_) => "object".to_string(),
        }
    }

    /// Coerce a JSON value so it matches a metadata field's declared type, or
    /// return `None` when the value cannot be represented as that type.
    ///
    /// The field's type (as registered in Physna) is authoritative, so this is
    /// how a batch CSV — which carries only string values — gets its values
    /// turned into the JSON scalars the API requires:
    ///
    /// - `number`: an existing JSON number is kept; a string that parses as an
    ///   integer or float becomes a JSON number; anything else is a conflict.
    /// - `boolean`: an existing JSON bool is kept; `true/false/1/0/yes/no/on/off`
    ///   (case-insensitive) parse to a bool; anything else is a conflict.
    /// - `text`, `url`, and any other string-backed type: the value is stored as
    ///   a JSON string, stringifying numbers and bools as needed. This is why a
    ///   URL string is accepted by a `url`-typed field even though the JSON type
    ///   is "string".
    ///
    /// `Null` is never produced here; empty values are handled as deletes before
    /// reaching this function.
    fn coerce_value_to_type(
        value: &serde_json::Value,
        field_type: &str,
    ) -> Option<serde_json::Value> {
        use serde_json::Value;
        match field_type {
            "number" => match value {
                Value::Number(_) => Some(value.clone()),
                Value::String(s) => {
                    let s = s.trim();
                    if let Ok(i) = s.parse::<i64>() {
                        Some(Value::Number(i.into()))
                    } else if let Ok(f) = s.parse::<f64>() {
                        if f.fract() == 0.0 && f.abs() < i64::MAX as f64 {
                            Some(Value::Number((f as i64).into()))
                        } else {
                            serde_json::Number::from_f64(f).map(Value::Number)
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            },
            "boolean" => match value {
                Value::Bool(_) => Some(value.clone()),
                Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => Some(Value::Bool(true)),
                    "false" | "0" | "no" | "off" => Some(Value::Bool(false)),
                    _ => None,
                },
                _ => None,
            },
            // text, url, and anything else are stored as JSON strings.
            _ => match value {
                Value::String(_) => Some(value.clone()),
                Value::Number(n) => Some(Value::String(n.to_string())),
                Value::Bool(b) => Some(Value::String(b.to_string())),
                _ => None,
            },
        }
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
        self.execute_request_no_response(|client| Ok(client.patch(url).json(body)), false)
            .await
    }

    /// POST a JSON body to an endpoint whose success response has no body (204).
    async fn post_no_response<B>(&mut self, url: &str, body: &B) -> Result<(), ApiError>
    where
        B: serde::Serialize,
    {
        self.execute_request_no_response(|client| Ok(client.post(url).json(body)), false)
            .await
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
        self.execute_request_no_response(|client| Ok(client.delete(url).json(body)), true)
            .await
    }

    /// Execute a request whose success response has no body worth reading.
    async fn execute_request_no_response<F>(
        &mut self,
        request_builder: F,
        idempotent: bool,
    ) -> Result<(), ApiError>
    where
        F: FnMut(&reqwest::Client) -> Result<reqwest::RequestBuilder, ApiError>,
    {
        self.request_with_auth(request_builder, idempotent)
            .await
            .map(|_| ())
    }

    /// One page of an asset's direct dependencies.
    ///
    /// Uses `GET /tenants/{tenantId}/assets/{assetId}/dependencies-by-id`, which
    /// looks the asset up by its stored path internally, so it keeps working after
    /// folders are moved or renamed. (The older `/assets/{assetPath}/dependencies`
    /// is deprecated in the API specification.)
    async fn get_asset_dependencies_by_uuid_with_pagination(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        page: usize,
        per_page: usize,
    ) -> Result<AssetDependenciesResponse, ApiError> {
        debug!(
            "Getting asset dependencies by UUID for tenant UUID: {}, asset UUID: {}",
            tenant_uuid, asset_uuid
        );

        let url = format!(
            "{}/tenants/{}/assets/{}/dependencies-by-id?page={}&perPage={}",
            self.base_url, tenant_uuid, asset_uuid, page, per_page
        );
        debug!("Dependencies request URL: {}", url);

        // Execute the GET request using the generic method
        // Handle the case where an asset has no dependencies (which may return 404)
        // The API returns 404 when no dependencies exist, which we now handle as a NotFoundError
        match self.get(&url).await {
            Ok(response) => Ok(response),
            Err(ApiError::NotFoundError(error_msg)) => {
                // Check if this is a "no dependencies found" error which is a valid response
                if error_msg.contains("No dependencies found for asset") {
                    debug!("Asset has no dependencies (404 with 'No dependencies found' message), returning empty response");
                    Ok(AssetDependenciesResponse {
                        dependencies: vec![],
                        page_data: crate::model::PageData {
                            current_page: page,
                            per_page,
                            total: 0,
                            last_page: 1,
                            start_index: 0,
                            end_index: 0,
                        },
                        original_asset_path: asset_uuid.to_string(), // Store UUID as string for consistency
                    })
                } else {
                    // Re-raise the original error if it's not related to missing dependencies
                    Err(ApiError::NotFoundError(error_msg))
                }
            }
            // A genuine 404 (e.g. the asset itself no longer exists) and auth
            // errors propagate as errors. They must NOT be swallowed into an
            // empty dependency list: only the explicit "No dependencies found"
            // response above means "this asset has no dependencies".
            Err(e) => Err(e),
        }
    }

    async fn populate_asset_dependencies_recursive_by_uuid(
        &mut self,
        tenant_uuid: &Uuid,
        root: &mut AssemblyNode,
        root_uuid: &Uuid,
        ancestors: &mut std::collections::HashSet<Uuid>,
    ) -> Result<(), ApiError> {
        // The assemblies on the way down from the top. An assembly that (through
        // the server's data) contains itself would otherwise be expanded forever.
        // A sub-assembly used in two different branches is not an ancestor of
        // itself and is expanded in both.
        if !ancestors.insert(*root_uuid) {
            warn!(
                "Assembly {} contains itself; its dependencies are listed once",
                root_uuid
            );
            return Ok(());
        }
        let per_page: usize = 1000; // the API maximum for this endpoint
        let mut pager = crate::paging::Pager::new("dependency listing");

        loop {
            let page = pager.page();
            // Use the UUID-based pagination method
            let response = self
                .get_asset_dependencies_by_uuid_with_pagination(
                    tenant_uuid,
                    root_uuid,
                    page,
                    per_page,
                )
                .await?;

            for dependency in response.dependencies {
                // Decided before `dependency.asset` is moved below.
                let missing = dependency.is_missing();

                // Convert dependency asset once, but only if it exists
                let child_asset: Asset = if let Some(asset_response) = dependency.asset {
                    asset_response.into()
                } else {
                    // Create a minimal Asset when full details are not available
                    // Use the path to extract a name
                    let name = dependency
                        .path
                        .split('/')
                        .next_back()
                        .unwrap_or(&dependency.path)
                        .to_string();
                    Asset::new(
                        Uuid::nil(), // Use nil UUID when not available
                        name,
                        dependency.path.clone(),
                        None,                        // file_size
                        None,                        // file_type
                        Some("missing".to_string()), // processing_status
                        None,                        // created_at
                        None,                        // updated_at
                        None,                        // metadata
                        false, // is_assembly - default to false for missing dependencies
                    )
                };

                // Insert into tree and get a mutable reference to the stored node
                let child_node: &mut AssemblyNode =
                    root.add_dependency_mut(child_asset.clone(), dependency.occurrences);

                // Recurse on the stored child node if it has dependencies. A
                // missing dependency has no asset, so there is nothing to ask
                // the API about (and asking about the nil UUID would be a 404).
                if dependency.has_dependencies && !missing {
                    Box::pin(self.populate_asset_dependencies_recursive_by_uuid(
                        tenant_uuid,
                        child_node,
                        &child_asset.uuid(),
                        ancestors,
                    ))
                    .await?;
                }
            }

            if !pager.advance(
                response.page_data.current_page,
                response.page_data.last_page,
                root.children().count(),
            ) {
                break;
            }
        }

        ancestors.remove(root_uuid);
        Ok(())
    }

    /// Send any request to the API and return the status and body (`pcli2 api`).
    ///
    /// `path` is relative to the API base URL (`/tenants/.../folders?page=2`). The
    /// request goes through the same path as every other call: token renewal on a
    /// 401, retries of transient failures (a GET, PUT or DELETE may be resent; a
    /// POST or PATCH only when the server did not act on it), and a non-2xx answer
    /// comes back as the usual classified error.
    pub async fn raw_request(
        &mut self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(reqwest::StatusCode, String), ApiError> {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let idempotent = matches!(
            method,
            reqwest::Method::GET
                | reqwest::Method::HEAD
                | reqwest::Method::PUT
                | reqwest::Method::DELETE
                | reqwest::Method::OPTIONS
        );
        debug!("API passthrough: {} {}", method, url);
        let response = self
            .request_with_auth(
                |client| {
                    let request = client.request(method.clone(), &url);
                    Ok(match body {
                        Some(body) => request.json(body),
                        None => request,
                    })
                },
                idempotent,
            )
            .await?;
        let status = response.status();
        let text = response.text().await?;
        Ok((status, text))
    }

    /// Walk a paged asset listing, `perPage=1000` (the API maximum), until the
    /// last page or `limit` assets. `url` carries the endpoint and any filter
    /// query; the page parameters are appended.
    async fn collect_asset_pages(
        &mut self,
        url: &str,
        limit: Option<usize>,
    ) -> Result<AssetList, ApiError> {
        const PER_PAGE: usize = 1000;
        let separator = if url.contains('?') { '&' } else { '?' };
        // One page size for the whole walk. It used to shrink on the last request
        // to fit --limit, but the page number still counted in the old size, so
        // `page=2&perPage=500` after a first page of 1000 returned records 501-1000
        // again and never reached 1001-1500.
        let per_page = limit.map_or(PER_PAGE, |limit| PER_PAGE.min(limit.max(1)));
        let mut pager = crate::paging::Pager::new("asset listing");
        let mut assets: Vec<Asset> = Vec::new();
        loop {
            let page = pager.page();
            let page_url = format!("{url}{separator}page={page}&perPage={per_page}");
            debug!("Asset listing request URL: {}", page_url);
            let response: AssetListResponse = self.get(&page_url).await?;
            let current_page = response.page_data.current_page;
            let last_page = response.page_data.last_page;
            assets.extend(response.assets.iter().map(Asset::from));
            let enough = limit.is_some_and(|limit| assets.len() >= limit);
            if enough || !pager.advance(current_page, last_page, assets.len()) {
                break;
            }
        }
        if let Some(limit) = limit {
            assets.truncate(limit);
        }
        Ok(AssetList::from(assets))
    }

    /// Stream a GET response body to `dest` through `<dest>.part`.
    ///
    /// `what` names the thing being downloaded in error messages. The part file
    /// is removed on any failure; an empty body is a failure.
    async fn download_url_to_file(
        &mut self,
        url: &str,
        what: &str,
        dest: &std::path::Path,
    ) -> Result<u64, ApiError> {
        debug!("Download request URL: {}", url);
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        let part_name = format!(
            "{}.part",
            dest.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "download".to_string())
        );
        let part_path = dest.with_file_name(part_name);

        // The retry layer covers a request that fails before its headers arrive. A
        // connection that drops while the body is streaming - a reset twenty minutes
        // into a multi-gigabyte file - used to fail the item outright; the download
        // is a GET, so it is started again from the beginning, up to the same number
        // of retries.
        let max_retries = self.http_client.config().max_retries;
        let mut attempt: u32 = 0;
        let written = loop {
            let response = self
                .request_with_auth(|client| Ok(client.get(url)), true)
                .await
                .map_err(|e| e.about(what))?;
            match stream_body_to_file(response, &part_path).await {
                Ok(written) => break written,
                Err(BodyError::Network(e)) if attempt < max_retries => {
                    attempt += 1;
                    crate::stats::record_retry();
                    warn!(
                        "Connection lost while downloading {} ({}); starting again (attempt {}/{})",
                        what, e, attempt, max_retries
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(500 * u64::from(attempt)))
                        .await;
                }
                Err(e) => {
                    let _ = tokio::fs::remove_file(&part_path).await;
                    return Err(e.into());
                }
            }
        };

        if written == 0 {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(ApiError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("the server returned an empty file for {}", what),
            )));
        }
        tokio::fs::rename(&part_path, dest).await?;
        debug!(
            "Downloaded {} bytes for {} to {}",
            written,
            what,
            dest.display()
        );
        Ok(written)
    }

    /// Create a specialized client for upload operations with appropriate timeout.
    ///
    /// The clone shares the token slot and the renewal lock with `self`, so a
    /// renewal in either is seen by both. When no separate upload timeout is
    /// configured (the default today) the plain clone is returned, so the
    /// connection pool is shared too.
    pub fn for_upload_operations(&self) -> Self {
        let timeout = self
            .http_client
            .config()
            .upload_timeout
            .unwrap_or(self.http_client.config().timeout);
        if timeout == self.http_client.config().timeout {
            return self.clone();
        }
        let http_client_with_upload_timeout =
            match crate::http_utils::HttpClient::new_with_timeout(timeout) {
                Ok(client) => client,
                Err(_) => self.http_client.clone(), // Fall back to original client if timeout creation fails
            };

        Self {
            base_url: self.base_url.clone(),
            access_token: self.access_token.clone(),
            renewal: self.renewal.clone(),
            client_credentials: self.client_credentials.clone(),
            auth_url: self.auth_url.clone(),
            http_client: http_client_with_upload_timeout,
            environment_name: self.environment_name.clone(),
        }
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
        assert_eq!(
            url,
            "https://app-api.physna.com/v3/tenants/test-tenant/assets"
        );
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

    #[test]
    fn test_infer_json_value_type_string() {
        let value = serde_json::Value::String("test".to_string());
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "text");
    }

    #[test]
    fn test_infer_json_value_type_number_integer() {
        let value = serde_json::Value::Number(serde_json::Number::from(42));
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "number");
    }

    #[test]
    fn test_infer_json_value_type_number_float() {
        let value = serde_json::Value::Number(serde_json::Number::from_f64(2.71).unwrap());
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "number");
    }

    #[test]
    fn test_infer_json_value_type_boolean() {
        let value = serde_json::Value::Bool(true);
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "boolean");
    }

    #[test]
    fn test_infer_json_value_type_null() {
        let value = serde_json::Value::Null;
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "null");
    }

    #[test]
    fn test_infer_json_value_type_array() {
        let value = serde_json::Value::Array(vec![]);
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "array");
    }

    #[test]
    fn test_infer_json_value_type_object() {
        let value = serde_json::json!({});
        let inferred_type = PhysnaApiClient::infer_json_value_type(&value);
        assert_eq!(inferred_type, "object");
    }

    #[test]
    fn test_coerce_string_to_number() {
        // A batch CSV carries "18" as a string; a number-typed field must
        // receive the JSON number 18, not the string "18".
        let v = serde_json::Value::String("18".to_string());
        assert_eq!(
            PhysnaApiClient::coerce_value_to_type(&v, "number"),
            Some(serde_json::Value::Number(18.into()))
        );

        let v = serde_json::Value::String("84.50".to_string());
        assert_eq!(
            PhysnaApiClient::coerce_value_to_type(&v, "number"),
            serde_json::Number::from_f64(84.5).map(serde_json::Value::Number)
        );
    }

    #[test]
    fn test_coerce_non_numeric_string_to_number_is_conflict() {
        let v = serde_json::Value::String("N/A".to_string());
        assert_eq!(PhysnaApiClient::coerce_value_to_type(&v, "number"), None);
    }

    #[test]
    fn test_coerce_string_to_boolean() {
        for (input, expected) in [
            ("true", true),
            ("FALSE", false),
            ("yes", true),
            ("off", false),
        ] {
            let v = serde_json::Value::String(input.to_string());
            assert_eq!(
                PhysnaApiClient::coerce_value_to_type(&v, "boolean"),
                Some(serde_json::Value::Bool(expected)),
                "input: {input}"
            );
        }

        let v = serde_json::Value::String("maybe".to_string());
        assert_eq!(PhysnaApiClient::coerce_value_to_type(&v, "boolean"), None);
    }

    #[test]
    fn test_coerce_url_and_text_keep_string() {
        // A url-typed field stores a plain JSON string; the value must not be
        // rejected just because its declared type is "url" rather than "text".
        let v = serde_json::Value::String("https://example.com/".to_string());
        assert_eq!(
            PhysnaApiClient::coerce_value_to_type(&v, "url"),
            Some(v.clone())
        );
        assert_eq!(PhysnaApiClient::coerce_value_to_type(&v, "text"), Some(v));
    }

    #[test]
    fn test_coerce_number_to_text_stringifies() {
        let v = serde_json::Value::Number(18.into());
        assert_eq!(
            PhysnaApiClient::coerce_value_to_type(&v, "text"),
            Some(serde_json::Value::String("18".to_string()))
        );
    }

    #[test]
    fn test_metadata_type_mismatch_error_display() {
        let error = ApiError::MetadataTypeMismatch {
            field_name: "test_field".to_string(),
            expected_type: "number".to_string(),
            provided_type: "text".to_string(),
        };
        let error_str = error.to_string();
        assert!(error_str.contains("Metadata type mismatch"));
        assert!(error_str.contains("test_field"));
        assert!(error_str.contains("number"));
        assert!(error_str.contains("text"));
    }
}

#[allow(
    clippy::items_after_test_module,
    clippy::doc_lazy_continuation,
    clippy::doc_overindented_list_items,
    clippy::empty_line_after_doc_comments
)]
impl PhysnaApiClient {
    /// Save the current access token to the keyring
    ///
    /// This method allows the current access token to be persisted to the keyring
    /// after it has been refreshed automatically. This ensures that subsequent
    /// requests will use the fresh token instead of the expired one.
    ///
    /// # Arguments
    /// * `environment_name` - The environment name to use as the keyring service name
    ///
    /// # Returns
    /// * `Ok(())` - Token successfully saved to keyring
    /// * `Err(ApiError)` - Failed to save token to keyring
    pub fn save_current_token_to_keyring(&self, environment_name: &str) -> Result<(), ApiError> {
        if let Some(token) = self.current_token() {
            let mut keyring = crate::keyring::Keyring::default();
            keyring
                .put(environment_name, "access-token".to_string(), token.clone())
                .map_err(|e| {
                    ApiError::AuthError(format!("Failed to save token to keyring: {}", e))
                })?;
        }
        Ok(())
    }
}

/// Read an error response's body for a message, tolerating an unreadable one.
async fn read_error_body(response: reqwest::Response) -> String {
    response
        .text()
        .await
        .unwrap_or_else(|_| "Unknown error".to_string())
}

/// What the API said, for a message: its JSON `message` or `error` field when the
/// body is JSON, otherwise the body itself (trimmed, and cut short - an HTML error
/// page from a gateway can run to kilobytes).
fn api_message(body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        for key in ["message", "error"] {
            if let Some(text) = json.get(key).and_then(|v| v.as_str()) {
                return text.to_string();
            }
        }
    }
    excerpt(body)
}

/// The first part of a body, for logs and messages.
fn excerpt(text: &str) -> String {
    const LIMIT: usize = 600;
    let trimmed = text.trim();
    if trimmed.chars().count() <= LIMIT {
        trimmed.to_string()
    } else {
        let cut: String = trimmed.chars().take(LIMIT).collect();
        format!("{}... ({} bytes)", cut, trimmed.len())
    }
}

/// Turn a response into `Ok` for 2xx and a classified `ApiError` otherwise.
/// Why streaming a response body to disk stopped.
enum BodyError {
    /// The connection failed mid-body; starting the request again may succeed.
    Network(reqwest::Error),
    /// Writing the local file failed; a retry would fail the same way.
    Local(std::io::Error),
}

impl From<BodyError> for ApiError {
    fn from(error: BodyError) -> Self {
        match error {
            BodyError::Network(e) => ApiError::HttpError(e),
            BodyError::Local(e) => ApiError::IoError(e),
        }
    }
}

/// Stream a response body into `path` (created or truncated) through a buffer, and
/// return how many bytes were written.
///
/// Network chunks are small (8-16 KiB); written straight to a `tokio::fs::File` each
/// one is a separate trip to the blocking pool. A 1 MiB buffer turns that into a
/// handful of large writes.
async fn stream_body_to_file(
    response: reqwest::Response,
    path: &std::path::Path,
) -> Result<u64, BodyError> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let file = tokio::fs::File::create(path)
        .await
        .map_err(BodyError::Local)?;
    let mut file = tokio::io::BufWriter::with_capacity(1 << 20, file);
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(BodyError::Network)?;
        file.write_all(&chunk).await.map_err(BodyError::Local)?;
        written += chunk.len() as u64;
    }
    file.flush().await.map_err(BodyError::Local)?;
    Ok(written)
}

async fn classify_response(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = read_error_body(response).await;
    debug!(
        "HTTP request failed with status: {}, body: {}",
        status, body
    );
    let message = api_message(&body);
    Err(match status {
        reqwest::StatusCode::NOT_FOUND => {
            ApiError::NotFoundError(format!("HTTP {} - {}", status, message))
        }
        reqwest::StatusCode::CONFLICT => {
            ApiError::ConflictError(format!("HTTP {} - {}", status, message))
        }
        _ => ApiError::HttpStatus {
            status: status.as_u16(),
            message,
        },
    })
}

impl ApiError {
    /// Attach what the request was about to the message, for errors that carry one.
    fn about(self, what: &str) -> ApiError {
        match self {
            ApiError::HttpStatus { status, message } => ApiError::HttpStatus {
                status,
                message: format!("{} ({})", message, what),
            },
            ApiError::NotFoundError(message) => {
                ApiError::NotFoundError(format!("{} ({})", message, what))
            }
            ApiError::ConflictError(message) => {
                ApiError::ConflictError(format!("{} ({})", message, what))
            }
            other => other,
        }
    }
}

/// `name (ID: uuid)` when the name is known, else the id.
fn describe_asset(asset_id: &str, asset_name: Option<&str>) -> String {
    match asset_name {
        Some(name) => format!("{} (ID: {})", name, asset_id),
        None => asset_id.to_string(),
    }
}

/// Translate an upload failure into the message a user can act on.
///
/// Applied to whichever attempt failed, so a file with an unsupported extension
/// gets the same explanation whether or not the first try also hit an expired token.
/// Errors of a new upload. A 409 here means the path is taken.
fn map_upload_error(error: ApiError) -> ApiError {
    match error {
        ApiError::ConflictError(_) => ApiError::ConflictError(
            "Asset already exists. Please use a different filename or delete the existing asset first."
                .to_string(),
        ),
        other => map_file_error(other),
    }
}

/// Errors any file transfer to the API can produce, worded for the user. A
/// 409 is left as the server phrased it: for a replacement it does not mean
/// "the path is taken".
fn map_file_error(error: ApiError) -> ApiError {
    match error {
        ApiError::HttpStatus {
            status: 422,
            message,
        } => ApiError::HttpStatus {
            status: 422,
            message: format!(
                "Invalid request data. Please check your input and try again. ({})",
                message
            ),
        },
        ApiError::HttpStatus { status: 413, .. } => ApiError::HttpStatus {
            status: 413,
            message: "File is too large. Please check the file size limits and try again."
                .to_string(),
        },
        ApiError::HttpStatus { message, .. } if message.contains("Invalid path extension:") => {
            let file_ext = extract_file_extension_from_error(&message);
            ApiError::InvalidParameterError(if file_ext.is_empty() {
                "Unsupported file type: This file format is not supported by Physna. Please use a supported format like .sldprt, .step, .stl, etc.".to_string()
            } else {
                format!(
                    "Unsupported file type: {} is not supported by Physna. Supported file types may include formats like .sldprt, .step, .stl, etc.",
                    file_ext
                )
            })
        }
        other => other,
    }
}

/// The upload endpoint answers with either `{ "asset": {...} }` or a bare asset.
fn parse_created_asset(text: &str) -> Result<Asset, ApiError> {
    match serde_json::from_str::<crate::model::SingleAssetResponse>(text) {
        Ok(result) => Ok(Asset::from(&result.asset)),
        Err(_) => match serde_json::from_str::<crate::model::AssetResponse>(text) {
            Ok(asset) => Ok(Asset::from(&asset)),
            Err(e) => {
                error!(
                    "Failed to parse response as either SingleAssetResponse or AssetResponse: {}",
                    e
                );
                Err(ApiError::JsonError(e))
            }
        },
    }
}

/// Which of `requested` have no asset in `found`, in request order, once each.
pub fn missing_asset_ids(requested: &[Uuid], found: &[Asset]) -> Vec<Uuid> {
    let present: std::collections::HashSet<Uuid> = found.iter().map(|a| a.uuid()).collect();
    let mut missing = Vec::new();
    for id in requested {
        if !present.contains(id) && !missing.contains(id) {
            missing.push(*id);
        }
    }
    missing
}

/// Helper function to extract file extension from error message
fn extract_file_extension_from_error(error_msg: &str) -> String {
    // Look for the last '.' in the error message to extract the file extension
    // The error message format is: "Invalid path extension: 'path/to/file.ext'"
    if let Some(start) = error_msg.find('\'') {
        if let Some(end) = error_msg[start + 1..].find('\'') {
            let path_str = &error_msg[start + 1..start + 1 + end];
            if let Some(ext_pos) = path_str.rfind('.') {
                return path_str[ext_pos..].to_string();
            }
        }
    }
    // If we can't extract the extension, return an empty string
    String::new()
}

/// Expand an upload file specification into the list of matching files.
///
/// Supports both glob patterns (e.g. "data/*.stl") and comma-separated
/// lists (e.g. "file1.stl,file2.stl"). Comma-separated entries that do
/// not exist on disk are silently skipped, matching upload behavior.
pub fn expand_upload_paths(pattern: &str) -> Result<Vec<std::path::PathBuf>, ApiError> {
    let paths: Vec<std::path::PathBuf> = if pattern.contains(',') {
        // Comma-separated list of explicit file paths. A path that does not exist
        // is an error: it used to be dropped silently, so a typo in one of the
        // names simply uploaded one file fewer.
        let mut paths = Vec::new();
        for entry in pattern.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let path = std::path::PathBuf::from(entry);
            if !path.is_file() {
                return Err(ApiError::PathNotFound(entry.to_string()));
            }
            paths.push(path);
        }
        paths
    } else {
        // Glob pattern: files only. Directories that match are skipped and
        // counted rather than turned into per-file failures.
        let mut skipped_dirs = 0usize;
        let mut paths = Vec::new();
        for path in glob(pattern)?.filter_map(|path_result| path_result.ok()) {
            if path.is_dir() {
                skipped_dirs += 1;
            } else {
                paths.push(path);
            }
        }
        if skipped_dirs > 0 {
            warn!(
                "{} director{} matched '{}' and {} skipped; only files are uploaded",
                skipped_dirs,
                if skipped_dirs == 1 { "y" } else { "ies" },
                pattern,
                if skipped_dirs == 1 { "was" } else { "were" }
            );
        }
        paths
    };
    Ok(paths)
}

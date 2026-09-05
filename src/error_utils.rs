//! Error handling utilities for the PCLI2 application.
//!
//! This module provides consistent error reporting and handling utilities
//! across the application to ensure uniform user experience.

use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

static JSON_ERRORS: AtomicBool = AtomicBool::new(false);

/// Switch stderr diagnostics to one JSON object per line.
pub fn set_json_errors(json: bool) {
    JSON_ERRORS.store(json, Ordering::SeqCst);
}

/// Whether `--error-format json` (or `PCLI2_ERROR_FORMAT=json`) is in effect.
pub fn json_errors() -> bool {
    JSON_ERRORS.load(Ordering::SeqCst)
}

/// Whether JSON errors were asked for, read straight from the command line and
/// the environment.
///
/// Needed before clap has parsed anything: a usage error is reported by clap
/// itself, and a script that asked for JSON must get JSON for that too.
pub fn json_errors_requested() -> bool {
    if std::env::var("PCLI2_ERROR_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        return true;
    }
    let args: Vec<String> = std::env::args().collect();
    args.iter().enumerate().any(|(i, arg)| {
        arg.eq_ignore_ascii_case("--error-format=json")
            || (arg == "--error-format"
                && args
                    .get(i + 1)
                    .map(|v| v.eq_ignore_ascii_case("json"))
                    .unwrap_or(false))
    })
}

fn emit_json(value: &serde_json::Value) {
    eprintln!("{}", value);
}

/// The JSON object for a failed command: exit code, its class, the message,
/// and when known a hint and the HTTP status behind it.
pub fn json_error_object(error: &crate::error::CliError) -> serde_json::Value {
    let code = error.exit_code();
    let message = error.to_string();
    let hint = hint_for(error).or_else(|| oauth_hint(&message));
    let mut object = serde_json::json!({
        "level": "ERROR",
        "code": code.code(),
        "kind": code.kind(),
        "message": message,
    });
    if let Some(hint) = hint {
        object["hint"] = serde_json::Value::String(hint.to_string());
    }
    if let Some(status) = http_status_of(error) {
        object["http_status"] = serde_json::Value::from(status);
    }
    object
}

/// The JSON object for a command line clap rejected.
pub fn json_usage_error(message: &str) -> serde_json::Value {
    let code = crate::exit_codes::PcliExitCode::UsageError;
    serde_json::json!({
        "level": "ERROR",
        "code": code.code(),
        "kind": code.kind(),
        "message": message,
    })
}

fn http_status_of(error: &crate::error::CliError) -> Option<u16> {
    match error {
        crate::error::CliError::PhysnaExtendedApiError(e) => e.http_status(),
        crate::error::CliError::ActionError(crate::actions::CliActionError::ApiError(e)) => {
            e.http_status()
        }
        _ => None,
    }
}

/// Common error types used throughout the application
#[derive(Debug, Error)]
pub enum CommonError {
    /// Error when required arguments are missing
    #[error("Missing required argument: {arg}")]
    MissingArgument { arg: String },

    /// Error when API calls fail
    #[error("API error: {message}")]
    ApiError { message: String },

    /// Error when authentication fails
    #[error("Authentication error: {message}")]
    AuthError { message: String },

    /// Error when resource is not found
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    /// Error when cache operations fail
    #[error("Cache error: {message}")]
    CacheError { message: String },

    /// Error when configuration operations fail
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    /// Error when file operations fail
    #[error("File error: {message}")]
    FileError { message: String },

    /// Error when data formatting fails
    #[error("Formatting error: {message}")]
    FormatError { message: String },

    /// Generic error with custom message
    #[error("Error: {message}")]
    Generic { message: String },
}

/// Report an error consistently with user-facing output.
///
/// This function displays errors in a user-friendly format without internal logging.
pub fn report_error<E: std::fmt::Display>(error: &E) {
    if json_errors() {
        emit_json(&serde_json::json!({"level": "ERROR", "message": error.to_string()}));
        return;
    }
    eprintln!("❌ Error: {}", error);
}

/// Report an error with detailed information including technical details and user guidance.
///
/// This function provides a comprehensive error message that includes:
/// - A clear error title
/// - Technical details about what went wrong
/// - Actionable steps the user can take to resolve the issue
/// - Relevant command examples when applicable
pub fn report_detailed_error<E: std::fmt::Display>(error: &E, context: Option<&str>) {
    let error_str = error.to_string();
    if json_errors() {
        let mut object = serde_json::json!({"level": "ERROR", "message": error_str});
        if let Some(hint) = oauth_hint(&error_str) {
            object["hint"] = serde_json::Value::String(hint.to_string());
        }
        if let Some(ctx) = context.filter(|c| !c.trim().is_empty()) {
            object["context"] = serde_json::Value::String(ctx.to_string());
        }
        emit_json(&object);
        return;
    }
    let user_friendly_msg = create_user_friendly_error(&error_str);

    // Print the main error message
    eprintln!("❌ Error: {}", user_friendly_msg);

    // Add context if provided and meaningful (not generic messages)
    if let Some(ctx) = context {
        // Skip generic context messages that don't add value
        if !ctx.trim().is_empty() && ctx != "Command execution failed" {
            eprintln!("📋 Context: {}", ctx);
        }
    }

    // Log the technical details for debugging (only in debug/trace mode)
    tracing::debug!(
        "Technical error details: {} (context: {:?})",
        error,
        context
    );
}

/// Report an error with suggested remediation steps.
///
/// This function provides error messages with specific steps users can take to resolve the issue.
pub fn report_error_with_remediation<E: std::fmt::Display>(error: &E, remediation_steps: &[&str]) {
    let error_str = error.to_string();
    if json_errors() {
        let mut object = serde_json::json!({"level": "ERROR", "message": error_str});
        if let Some(hint) = oauth_hint(&error_str) {
            object["hint"] = serde_json::Value::String(hint.to_string());
        }
        if !remediation_steps.is_empty() {
            object["steps"] = serde_json::json!(remediation_steps);
        }
        emit_json(&object);
        return;
    }
    let user_friendly_msg = create_user_friendly_error(&error_str);

    eprintln!("❌ Error: {}", user_friendly_msg);

    if !remediation_steps.is_empty() {
        eprintln!("\n🔧 To resolve this issue, try the following:");
        for (i, step) in remediation_steps.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, step);
        }
    }

    tracing::debug!(
        "Error with remediation: {} (steps: {:?})",
        error,
        remediation_steps
    );
}

/// Report a warning through the tracing subsystem.
///
/// Warnings are emitted only via tracing (never printed directly), so the
/// user controls their visibility with --verbose/--quiet, RUST_LOG, or
/// PCLI2_LOG_LEVEL. The default level is "warn", so warnings are visible
/// unless the user opts down to errors only.
pub fn report_warning<E: std::fmt::Display>(warning: &E) {
    tracing::warn!("{}", warning);
}

/// Add a hint to an error message when its cause is one the user can act on.
///
/// The message itself is never replaced. It used to be: anything containing
/// "401", "404", "connection" and the like was swapped for canned text, so a folder
/// named `401-series` reported an authentication problem and an environment that did
/// not exist lost its name. Only the OAuth error codes, which are unambiguous tokens
/// from the auth server, still get an explanation appended.
pub fn create_user_friendly_error<E: std::fmt::Display>(error: E) -> String {
    let error_str = error.to_string();
    match oauth_hint(&error_str) {
        Some(hint) => format!("{}\n💡 {}", error_str, hint),
        None => error_str,
    }
}

/// The explanation for an OAuth 2.0 error code embedded in a message, if any.
fn oauth_hint(error_str: &str) -> Option<&'static str> {
    if error_str.contains("invalid_client") {
        Some("The auth server rejected the client credentials: the client ID or secret is wrong, expired, revoked, or the service account is disabled. Verify them and log in again with 'pcli2 auth login'.")
    } else if error_str.contains("invalid_grant") {
        Some("The authorization grant was rejected. Log in again with 'pcli2 auth login'.")
    } else if error_str.contains("unauthorized_client") {
        Some("This client is not authorized for the requested grant. Verify the client credentials and log in again with 'pcli2 auth login'.")
    } else if error_str.contains("invalid_request") {
        Some("The auth server rejected the request. Verify the credentials and log in again with 'pcli2 auth login'.")
    } else {
        None
    }
}

/// Print a failed command's error the way `main` wants it: the message as-is,
/// then one hint chosen from what the error *is* rather than from its text.
pub fn report_cli_error(error: &crate::error::CliError) {
    if json_errors() {
        emit_json(&json_error_object(error));
        tracing::debug!("Technical error details: {:?}", error);
        return;
    }
    eprintln!("❌ Error: {}", create_user_friendly_error(error));
    if let Some(hint) = hint_for(error) {
        eprintln!("💡 {}", hint);
    }
    tracing::debug!("Technical error details: {:?}", error);
}

/// A remediation hint for the class of failure, when one applies.
fn hint_for(error: &crate::error::CliError) -> Option<&'static str> {
    use crate::exit_codes::PcliExitCode;
    let text = error.to_string();
    let forbidden = match error {
        crate::error::CliError::PhysnaExtendedApiError(e) => e.is_forbidden(),
        crate::error::CliError::ActionError(crate::actions::CliActionError::ApiError(e)) => {
            e.is_forbidden()
        }
        _ => false,
    };
    match error.exit_code() {
        PcliExitCode::AuthError if !text.contains("auth login") => {
            Some("Log in again with 'pcli2 auth login'.")
        }
        PcliExitCode::NetworkError => Some(
            "Check your network connection and the API URL of the active environment ('pcli2 env get').",
        ),
        PcliExitCode::ApiError if forbidden => Some(
            "The request was authenticated but not permitted. Your account may lack the Author role for this tenant; ask a tenant administrator.",
        ),
        PcliExitCode::TempFail => Some("The failure looks transient; retry the command."),
        _ => None,
    }
}

/// Report an error with a user-friendly message based on error content
pub fn report_error_with_user_friendly_message<E: std::fmt::Display>(error: E) {
    if json_errors() {
        report_detailed_error(&error, None);
        return;
    }
    let user_message = create_user_friendly_error(error);
    eprintln!("❌ Error: {}", user_message);
}

/// Check if an error is retryable and user should try again
pub fn is_retryable_error<E: std::fmt::Display>(error: E) -> bool {
    let error_str = error.to_string().to_lowercase();

    error_str.contains("timeout")
        || error_str.contains("connection")
        || error_str.contains("network")
        || error_str.contains("502")
        || error_str.contains("503")
        || error_str.contains("504")
        || error_str.contains("gateway")
        || error_str.contains("proxy")
        || error_str.contains("service unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_kept_verbatim_and_only_oauth_codes_get_a_hint() {
        // Substrings that used to trigger a rewrite now pass through untouched.
        for original in [
            "HTTP Error: 401 Unauthorized",
            "Folder '/Projects/401-series' not found. Did you mean /Projects/401-Series",
            "Environment 'nope' not found",
            "Failed to upload connection-bracket.stl",
            "HTTP 409 - Asset not indexed yet",
        ] {
            assert_eq!(create_user_friendly_error(original), original);
        }

        let with_hint = create_user_friendly_error("Authentication failed: invalid_client");
        assert!(with_hint.starts_with("Authentication failed: invalid_client"));
        assert!(with_hint.contains("client ID or secret"));
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error("Connection timeout error"));
        assert!(is_retryable_error("503 Service Unavailable"));
        assert!(!is_retryable_error("Invalid argument"));
    }

    #[test]
    fn hints_follow_the_error_class_not_its_text() {
        use crate::error::CliError;
        use crate::physna_v3::ApiError;

        let auth = CliError::PhysnaExtendedApiError(ApiError::AuthError("token rejected".into()));
        assert!(hint_for(&auth).unwrap().contains("auth login"));
        // A message that already says what to do gets no second copy of it.
        let already_says_so = CliError::PhysnaExtendedApiError(ApiError::InvalidToken);
        assert!(hint_for(&already_says_so).is_none());

        let forbidden = CliError::PhysnaExtendedApiError(ApiError::HttpStatus {
            status: 403,
            message: "Forbidden".into(),
        });
        assert!(hint_for(&forbidden).unwrap().contains("Author role"));

        let not_found = CliError::FolderNotFound("/x".into(), String::new());
        assert!(hint_for(&not_found).is_none());
    }

    #[test]
    fn json_error_objects_carry_code_kind_hint_and_http_status() {
        use crate::error::CliError;
        use crate::physna_v3::ApiError;

        let not_found = json_error_object(&CliError::FolderNotFound("/x".into(), String::new()));
        assert_eq!(not_found["level"], "ERROR");
        assert_eq!(not_found["code"], 67);
        assert_eq!(not_found["kind"], "not_found");
        assert!(not_found["message"].as_str().unwrap().contains("/x"));
        assert!(not_found.get("hint").is_none());
        assert!(not_found.get("http_status").is_none());

        let forbidden =
            json_error_object(&CliError::PhysnaExtendedApiError(ApiError::HttpStatus {
                status: 403,
                message: "Forbidden".into(),
            }));
        assert_eq!(forbidden["code"], 102);
        assert_eq!(forbidden["kind"], "api");
        assert_eq!(forbidden["http_status"], 403);
        assert!(forbidden["hint"].as_str().unwrap().contains("Author role"));

        let usage = json_usage_error("unexpected argument '--bogus'");
        assert_eq!(usage["code"], 64);
        assert_eq!(usage["kind"], "usage");
    }

    #[test]
    fn test_retryable_errors() {
        assert!(is_retryable_error("Connection timeout"));
        assert!(is_retryable_error("503 Service Unavailable"));
        assert!(!is_retryable_error("Invalid argument"));
    }

    #[test]
    fn test_create_user_friendly_error_metadata_type_mismatch() {
        let error_msg = "Metadata type mismatch: Cannot update metadata field 'test_field' with a value of type 'text'. The field was defined as type 'number'.";
        let friendly_msg = create_user_friendly_error(error_msg);
        assert!(friendly_msg.contains("Metadata type mismatch"));
        assert!(friendly_msg.contains("test_field"));
        assert!(friendly_msg.contains("text"));
        assert!(friendly_msg.contains("number"));
    }
}

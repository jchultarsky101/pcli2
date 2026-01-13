//! Error handling utilities for the PCLI2 application.
//! 
//! This module provides consistent error reporting and handling utilities
//! across the application to ensure uniform user experience.

use thiserror::Error;
use tracing::error;

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
    tracing::debug!("Technical error details: {} (context: {:?})", error, context);
}

/// Report an error with suggested remediation steps.
///
/// This function provides error messages with specific steps users can take to resolve the issue.
pub fn report_error_with_remediation<E: std::fmt::Display>(error: &E, remediation_steps: &[&str]) {
    let error_str = error.to_string();
    let user_friendly_msg = create_user_friendly_error(&error_str);

    eprintln!("❌ Error: {}", user_friendly_msg);

    if !remediation_steps.is_empty() {
        eprintln!("\n🔧 To resolve this issue, try the following:");
        for (i, step) in remediation_steps.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, step);
        }
    }

    tracing::debug!("Error with remediation: {} (steps: {:?})", error, remediation_steps);
}

/// Report an error with a custom user message for better clarity.
///
/// This function allows providing a more user-friendly message while still
/// logging the technical error details for debugging.
pub fn report_error_with_message<E: std::fmt::Display>(error: &E, user_message: &str) {
    error!("{} (original error: {})", user_message, error);
    eprintln!("❌ Error: {}", user_message);
}

/// Report a warning consistently with both logging and user-facing output.
pub fn report_warning<E: std::fmt::Display>(warning: &E) {
    tracing::warn!("{}", warning);
    eprintln!("⚠️  Warning: {}", warning);
}

/// Create a user-friendly error message from a technical error
///
/// This function tries to provide user-friendly error messages for common technical errors
pub fn create_user_friendly_error<E: std::fmt::Display>(error: E) -> String {
    let error_str = error.to_string();

    // Check for common error patterns and provide user-friendly messages
    if error_str.contains("invalid_client") {
        "Authentication failed: Invalid client credentials. This could be due to:\n  - Incorrect client ID or secret\n  - Expired or revoked client credentials\n  - Disabled service account\n  Please verify your credentials and log in again with 'pcli2 auth login'.".to_string()
    } else if error_str.contains("invalid_grant") {
        "Authentication failed: Invalid authorization grant. Please log in again with 'pcli2 auth login'.".to_string()
    } else if error_str.contains("unauthorized_client") {
        "Authentication failed: Unauthorized client. Please verify your client credentials and try logging in again with 'pcli2 auth login'.".to_string()
    } else if error_str.contains("invalid_request") {
        "Authentication failed: Invalid request. Please verify your credentials and try logging in again with 'pcli2 auth login'.".to_string()
    } else if error_str.contains("401") || error_str.to_lowercase().contains("unauthorized") {
        "Authentication failed. Please check your access token and try logging in again with 'pcli2 auth login'.".to_string()
    } else if error_str.contains("403") || error_str.to_lowercase().contains("forbidden") {
        "Access forbidden. You don't have permission to perform this operation.".to_string()
    } else if error_str.contains("404") || error_str.to_lowercase().contains("not found") {
        "Resource not found. Please check the resource ID or path and try again.".to_string()
    } else if error_str.contains("409") || error_str.to_lowercase().contains("conflict") {
        "Operation conflict. The resource may already exist or be in use.".to_string()
    } else if error_str.to_lowercase().contains("timeout") {
        "Request timeout. The server took too long to respond. Please try again.".to_string()
    } else if error_str.to_lowercase().contains("connection") || error_str.to_lowercase().contains("network") {
        "Network error. Please check your internet connection and try again.".to_string()
    } else {
        // Return the original error if no specific user-friendly message applies
        error_str
    }
}

/// Report an error with a user-friendly message based on error content
pub fn report_error_with_user_friendly_message<E: std::fmt::Display>(error: E) {
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
    fn test_create_user_friendly_error_auth() {
        let error_msg = "HTTP Error: 401 Unauthorized";
        let friendly_msg = create_user_friendly_error(error_msg);
        assert!(friendly_msg.contains("Authentication failed"));
    }

    #[test]
    fn test_create_user_friendly_error_not_found() {
        let error_msg = "Resource not found";
        let friendly_msg = create_user_friendly_error(error_msg);
        assert!(friendly_msg.contains("Resource not found"));
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error("Connection timeout error"));
        assert!(is_retryable_error("503 Service Unavailable"));
        assert!(!is_retryable_error("Invalid argument"));
    }

    #[test]
    fn test_user_friendly_error_messages() {
        // Test common error patterns
        assert!(create_user_friendly_error("401 Unauthorized").contains("Authentication failed"));
        assert!(create_user_friendly_error("404 Not Found").contains("Resource not found"));
        assert!(create_user_friendly_error("timeout").contains("Request timeout"));
    }

    #[test]
    fn test_retryable_errors() {
        assert!(is_retryable_error("Connection timeout"));
        assert!(is_retryable_error("503 Service Unavailable"));
        assert!(!is_retryable_error("Invalid argument"));
    }
}



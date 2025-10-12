//! Error handling utilities for the PCLI2 application.
//! 
//! This module provides consistent error reporting and handling utilities
//! across the application to ensure uniform user experience.

use tracing::error;

/// Report an error consistently with both logging and user-facing output.
/// 
/// This function ensures errors are logged for debugging purposes and
/// displayed in a user-friendly format.
pub fn report_error<E: std::fmt::Display>(error: &E) {
    error!("{}", error);
    eprintln!("Error: {}", error);
}

/// Report an error with a custom user message for better clarity.
/// 
/// This function allows providing a more user-friendly message while still
/// logging the technical error details for debugging.
pub fn report_error_with_message<E: std::fmt::Display>(error: &E, user_message: &str) {
    error!("{} (original error: {})", user_message, error);
    eprintln!("Error: {}", user_message);
}

/// Report a warning consistently with both logging and user-facing output.
pub fn report_warning<E: std::fmt::Display>(warning: &E) {
    tracing::warn!("{}", warning);
    eprintln!("Warning: {}", warning);
}
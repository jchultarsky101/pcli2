use thiserror::Error;

use crate::{
    actions::CliActionError, exit_codes::PcliExitCode, folder_hierarchy::FolderHierarchyError,
    physna_v3,
};

/// Error types that can occur during CLI command execution
#[derive(Debug, Error)]
pub enum CliError {
    /// Error when an unsupported or undefined subcommand is encountered
    #[error("Undefined or unsupported subcommand")]
    UnsupportedSubcommand(String),
    /// Error related to configuration loading or management
    #[error("Configuration error: {0}")]
    ConfigurationError(#[from] crate::configuration::ConfigurationError),
    /// Error related to data formatting
    #[error("Formatting error: {0}")]
    FormattingError(#[from] crate::format::FormattingError),
    /// Error related to security operations (authentication, keyring access)
    #[error("Security error")]
    SecurityError(String),
    /// Error when a required command-line argument is missing
    #[error("Missing required argument: {0}")]
    MissingRequiredArgument(String),
    /// Error related to JSON serialization/deserialization
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Error when a tenant cannot be found by name or ID
    #[error("Tenant '{identifier}' not found")]
    TenantNotFound { identifier: String },
    /// Error when a folder cannot be found by path or ID
    #[error("Folder '{0}' not found. Please verify the folder path exists in your tenant.")]
    FolderNotFound(String),

    /// Error when a folder rename operation fails after successful path resolution
    #[error("Failed to rename folder '{0}'. The folder was found but the rename operation failed. This could be due to permissions or API limitations. Error details: {1}")]
    FolderRenameFailed(String, String),

    #[error("API error: {0}")]
    PhysnaExtendedApiError(#[from] physna_v3::ApiError),

    #[error("UUID parsing error: {0}")]
    UuidParsingError(#[from] uuid::Error),

    #[error("{0}")]
    ActionError(#[from] CliActionError),

    #[error("{0}")]
    FolderListError(#[from] FolderHierarchyError),

}

impl CliError {
    /// Get the appropriate exit code for this error
    ///
    /// Returns the corresponding `PcliExitCode` based on the error type:
    /// - `UsageError` for unsupported commands or missing arguments
    /// - `ConfigError` for configuration errors
    /// - `DataError` for formatting or JSON errors
    /// - `AuthError` for security-related errors
    pub fn exit_code(&self) -> PcliExitCode {
        match self {
            CliError::UnsupportedSubcommand(_) => PcliExitCode::UsageError,
            CliError::ConfigurationError(_) => PcliExitCode::ConfigError,
            CliError::FormattingError(_) => PcliExitCode::DataError,
            CliError::SecurityError(_) => PcliExitCode::AuthError,
            CliError::MissingRequiredArgument(_) => PcliExitCode::UsageError,
            CliError::JsonError(_) => PcliExitCode::DataError,
            CliError::TenantNotFound { .. } => PcliExitCode::UsageError,
            CliError::FolderNotFound { .. } => PcliExitCode::UsageError,
            _ => PcliExitCode::SoftwareError,
        }
    }
}

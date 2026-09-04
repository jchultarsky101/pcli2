use thiserror::Error;

use crate::{
    actions::CliActionError, exit_codes::PcliExitCode, folder_hierarchy::FolderHierarchyError,
    physna_v3,
};

/// Error types that can occur during CLI command execution
#[derive(Debug, Error)]
pub enum CliError {
    /// Error when an unsupported or undefined subcommand is encountered
    #[error("Undefined or unsupported subcommand: {0}")]
    UnsupportedSubcommand(String),
    /// Error related to configuration loading or management
    #[error("Configuration error: {0}")]
    ConfigurationError(#[from] crate::configuration::ConfigurationError),
    /// Error related to data formatting
    #[error("Formatting error: {0}")]
    FormattingError(#[from] crate::format::FormattingError),
    /// Error related to security operations (authentication, keyring access)
    #[error("{0}")]
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
    #[error("Folder '{0}' not found. Please verify the folder path exists in your tenant.{1}")]
    FolderNotFound(String, String),

    /// Error when a folder rename operation fails after successful path resolution
    #[error("Failed to rename folder '{0}'. The folder was found but the rename operation failed. This could be due to permissions or API limitations. Error details: {1}")]
    FolderRenameFailed(String, String),

    #[error("API error: {0}")]
    PhysnaExtendedApiError(#[from] physna_v3::ApiError),

    /// Error when one of the two input assets for a comparison cannot be resolved
    #[error("Could not resolve {0} asset: {1}")]
    AssetResolutionError(String, String),

    #[error("UUID parsing error: {0}")]
    UuidParsingError(#[from] uuid::Error),

    #[error("{0}")]
    ActionError(#[from] CliActionError),

    #[error("{0}")]
    FolderListError(#[from] FolderHierarchyError),

    #[error("Excel report error: {0}")]
    XlsxReportError(#[from] crate::xlsx_report::XlsxReportError),

    /// The failure was already reported in full, with remediation steps, by the
    /// code that detected it. Carries only the exit code; nothing is printed again.
    #[error("{}", .0.message())]
    AlreadyReported(PcliExitCode),
}

impl CliError {
    /// The exit code that describes this error to a script.
    ///
    /// Every variant is listed on purpose: a wildcard arm is how every API, network
    /// and not-found failure came to exit 70 "internal software error" while the
    /// documentation promised 100/101/102.
    pub fn exit_code(&self) -> PcliExitCode {
        match self {
            CliError::UnsupportedSubcommand(_) | CliError::MissingRequiredArgument(_) => {
                PcliExitCode::UsageError
            }
            CliError::ConfigurationError(_) => PcliExitCode::ConfigError,
            CliError::FormattingError(_)
            | CliError::JsonError(_)
            | CliError::XlsxReportError(_) => PcliExitCode::DataError,
            CliError::SecurityError(_) => PcliExitCode::AuthError,
            CliError::TenantNotFound { .. }
            | CliError::FolderNotFound(..)
            | CliError::AssetResolutionError(..) => PcliExitCode::NotFound,
            CliError::FolderRenameFailed(..) => PcliExitCode::ApiError,
            CliError::PhysnaExtendedApiError(e) => e.exit_code(),
            CliError::UuidParsingError(_) => PcliExitCode::UsageError,
            CliError::ActionError(e) => e.exit_code(),
            CliError::FolderListError(FolderHierarchyError::ApiError(e)) => e.exit_code(),
            CliError::AlreadyReported(code) => *code,
        }
    }

    /// True when the error has already been printed with its remediation steps and
    /// must not be printed again on the way out.
    pub fn is_already_reported(&self) -> bool {
        matches!(
            self,
            CliError::AlreadyReported(_)
                | CliError::ActionError(CliActionError::AlreadyReported(_))
        )
    }
}

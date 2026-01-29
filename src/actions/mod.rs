use thiserror::Error;

pub mod assets;
pub mod completions;
pub mod folders;
pub mod tenants;
pub mod utils;

#[derive(Debug, Error)]
pub enum CliActionError {
    #[error("{0}")]
    JsonError(#[from] serde_json::Error),

    #[error("{0}")]
    ApiError(#[from] crate::physna_v3::ApiError),

    #[error("{0}")]
    CsvError(#[from] csv::Error),

    #[error("{0}")]
    CsvIntoError(#[from] csv::IntoInnerError<csv::Writer<Vec<u8>>>),

    #[error("{0}")]
    UtfError(#[from] std::string::FromUtf8Error),

    #[error("{0}")]
    UuidPartsinError(#[from] uuid::Error),

    #[error("{0}")]
    ConfigurationError(#[from] crate::configuration::ConfigurationError),

    #[error("ERROR: Unsupported output format: {0}")]
    UnsupportedOutputFormat(String),

    #[error("{0}")]
    FormattingError(#[from] crate::format::FormattingError),

    #[error("Missing required argument: {0}")]
    MissingRequiredArgument(String),

    #[error("Tenant not found: {identifier}")]
    TenantNotFound { identifier: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    ZipError(#[from] zip::result::ZipError),

    #[error("{0}")]
    BusinessLogicError(String),
}

//! Custom exit codes for the PCLI2 application
//!
//! This module defines specific exit codes for different error conditions
//! to make scripting and automation easier.

/// Custom exit codes for PCLI2
///
/// - 0: Success
/// - 64-78: modelled on BSD sysexits.h (68 and 69 are used differently)
/// - 100+: Custom application-specific codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcliExitCode {
    /// Success (0) - Command completed successfully
    Success = 0,

    /// Command line usage error (64) - User input error
    UsageError = 64,

    /// Data format error (65) - Input data was incorrect
    DataError = 65,

    /// Cannot open input file (66) - File not found or permission denied
    NoInput = 66,

    /// Addressee unknown (67) - User or resource not found
    NotFound = 67,

    /// Unavailable (68) - The deployment lacks a feature the command needs (for
    /// example failure log search), or `doctor` could not reach a server
    Unavailable = 68,

    /// Temporary failure (69) - Try again later: rate limited or a server error
    /// that outlasted the retries, a batch with failed items, a report still running
    TempFail = 69,

    /// Internal software error (70) - Unexpected application error
    SoftwareError = 70,

    /// System error (71) - OS-level error
    OSError = 71,

    /// Configuration error (78) - Application configuration issue
    ConfigError = 78,

    /// Authentication error (100) - Login or token issues
    AuthError = 100,

    /// Network error (101) - Connection or communication issues
    NetworkError = 101,

    /// API error (102) - Remote API returned an error
    ApiError = 102,
}

impl PcliExitCode {
    /// The code for a local I/O failure: a file that cannot be opened is the
    /// caller's input problem, anything else is the system's.
    pub fn for_io_error(error: &std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => {
                PcliExitCode::NoInput
            }
            _ => PcliExitCode::OSError,
        }
    }

    /// Convert to numeric exit code
    pub fn code(&self) -> i32 {
        *self as i32
    }

    /// A stable machine-readable name for the failure class, used in JSON error
    /// output alongside the numeric code.
    pub fn kind(&self) -> &'static str {
        match self {
            PcliExitCode::Success => "success",
            PcliExitCode::UsageError => "usage",
            PcliExitCode::DataError => "data",
            PcliExitCode::NoInput => "no_input",
            PcliExitCode::NotFound => "not_found",
            PcliExitCode::Unavailable => "unavailable",
            PcliExitCode::TempFail => "temp_fail",
            PcliExitCode::SoftwareError => "software",
            PcliExitCode::OSError => "os",
            PcliExitCode::ConfigError => "config",
            PcliExitCode::AuthError => "auth",
            PcliExitCode::NetworkError => "network",
            PcliExitCode::ApiError => "api",
        }
    }

    /// Get descriptive message for the exit code
    pub fn message(&self) -> &'static str {
        match self {
            PcliExitCode::Success => "Success",
            PcliExitCode::UsageError => "Command line usage error",
            PcliExitCode::DataError => "Data format error",
            PcliExitCode::NoInput => "Cannot open input file",
            PcliExitCode::NotFound => "Resource not found",
            PcliExitCode::Unavailable => "Service unavailable",
            PcliExitCode::TempFail => "Temporary failure",
            PcliExitCode::SoftwareError => "Internal software error",
            PcliExitCode::OSError => "Operating system error",
            PcliExitCode::ConfigError => "Configuration error",
            PcliExitCode::AuthError => "Authentication error",
            PcliExitCode::NetworkError => "Network communication error",
            PcliExitCode::ApiError => "Remote API error",
        }
    }
}

impl From<PcliExitCode> for i32 {
    fn from(code: PcliExitCode) -> Self {
        code.code()
    }
}

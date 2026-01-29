//! Custom exit codes for the PCLI2 application
//!
//! This module defines specific exit codes for different error conditions
//! to make scripting and automation easier.

/// Custom exit codes for PCLI2
///
/// These codes follow the BSD sysexits.h conventions where possible:
/// - 0: Success
/// - 64-78: Standard exit codes from sysexits.h
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

    /// Host name unknown (68) - Server or service not found
    Unavailable = 68,

    /// Service unavailable (69) - Temporary service error
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
    /// Convert to numeric exit code
    pub fn code(&self) -> i32 {
        *self as i32
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

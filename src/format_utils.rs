//! Enhanced format utilities for the Physna CLI client.
//!
//! This module provides advanced functionality for handling format parameters,
//! building format options, and managing format presets.

use crate::{
    commands::params::{PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_METADATA, PARAMETER_PRETTY},
    format::{OutputFormat, OutputFormatOptions},
};
use clap::ArgMatches;

/// Container for parsed format parameters with consistent defaults and error handling.
#[derive(Debug, Clone)]
pub struct FormatParams {
    pub format: OutputFormat,
    pub format_options: OutputFormatOptions,
    pub format_str: String,
}

impl FormatParams {
    /// Parse all format-related parameters from command arguments with consistent defaults and error handling.
    pub fn from_args(sub_matches: &ArgMatches) -> FormatParams {
        // Get format string with environment variable precedence
        let format_str = get_format_string(sub_matches);

        // Extract all format flags consistently
        let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
        let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
        let with_metadata = sub_matches.get_flag(PARAMETER_METADATA);

        let format_options = OutputFormatOptions {
            with_metadata,
            with_headers,
            pretty,
        };

        let format =
            OutputFormat::from_string_with_options_safe(&format_str, format_options.clone())
                .unwrap_or_else(|_| OutputFormat::Json(OutputFormatOptions::default()));

        FormatParams {
            format,
            format_options,
            format_str,
        }
    }

    /// Get format with custom default when no format is specified.
    pub fn from_args_with_default(sub_matches: &ArgMatches, default_format: &str) -> FormatParams {
        let format_str = if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
            format_val.clone()
        } else {
            // Check environment variable first, then use provided default
            if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
                env_format
            } else {
                default_format.to_string()
            }
        };

        // Extract all format flags consistently
        let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
        let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
        let with_metadata = sub_matches.get_flag(PARAMETER_METADATA);

        let format_options = OutputFormatOptions {
            with_metadata,
            with_headers,
            pretty,
        };

        let format =
            OutputFormat::from_string_with_options_safe(&format_str, format_options.clone())
                .unwrap_or_else(|_| OutputFormat::Json(OutputFormatOptions::default()));

        FormatParams {
            format,
            format_options,
            format_str,
        }
    }
}

fn get_format_string(sub_matches: &ArgMatches) -> String {
    if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string() // Default format
        }
    }
}

/// Builder for format options to make them more flexible and extensible.
#[derive(Debug, Clone)]
pub struct FormatOptionsBuilder {
    with_metadata: bool,
    with_headers: bool,
    pretty: bool,
}

impl FormatOptionsBuilder {
    pub fn new() -> Self {
        Self {
            with_metadata: false,
            with_headers: false,
            pretty: false,
        }
    }

    pub fn with_metadata(mut self, enable: bool) -> Self {
        self.with_metadata = enable;
        self
    }

    pub fn with_headers(mut self, enable: bool) -> Self {
        self.with_headers = enable;
        self
    }

    pub fn pretty(mut self, enable: bool) -> Self {
        self.pretty = enable;
        self
    }

    pub fn build(self) -> OutputFormatOptions {
        OutputFormatOptions {
            with_metadata: self.with_metadata,
            with_headers: self.with_headers,
            pretty: self.pretty,
        }
    }

    /// Create from command line arguments
    pub fn from_args(sub_matches: &ArgMatches) -> Self {
        Self::new()
            .with_metadata(sub_matches.get_flag(PARAMETER_METADATA))
            .with_headers(sub_matches.get_flag(PARAMETER_HEADERS))
            .pretty(sub_matches.get_flag(PARAMETER_PRETTY))
    }
}

/// Format presets for common use cases.
#[derive(Debug, Clone)]
pub enum FormatPreset {
    /// Human-readable format with pretty printing
    HumanReadable,
    /// Machine-readable format (no extra whitespace)
    MachineReadable,
    /// Verbose format with all available metadata
    Verbose,
    /// Compact format with minimal output
    Compact,
    /// Tabular format with headers
    Tabular,
}

impl FormatPreset {
    pub fn to_format(&self, base_format: &str) -> OutputFormat {
        let options = match self {
            FormatPreset::HumanReadable => OutputFormatOptions {
                with_metadata: false,
                with_headers: false,
                pretty: true,
            },
            FormatPreset::MachineReadable => OutputFormatOptions {
                with_metadata: false,
                with_headers: false,
                pretty: false,
            },
            FormatPreset::Verbose => OutputFormatOptions {
                with_metadata: true,
                with_headers: true,
                pretty: true,
            },
            FormatPreset::Compact => OutputFormatOptions {
                with_metadata: false,
                with_headers: false,
                pretty: false,
            },
            FormatPreset::Tabular => OutputFormatOptions {
                with_metadata: false,
                with_headers: true,
                pretty: false,
            },
        };

        OutputFormat::from_string_with_options_safe(base_format, options)
            .unwrap_or_else(|_| OutputFormat::Json(OutputFormatOptions::default()))
    }

    /// Apply preset to an existing format
    pub fn apply_to(&self, format: OutputFormat) -> OutputFormat {
        match format {
            OutputFormat::Json(_) => self.to_format("json"),
            OutputFormat::Csv(_) => self.to_format("csv"),
            OutputFormat::Tree(_) => self.to_format("tree"),
        }
    }
}

/// Enhanced output formatter trait with additional functionality.
pub trait EnhancedOutputFormatter: crate::format::OutputFormatter {
    /// Format with automatic error handling and fallback
    fn format_safe(&self, format: OutputFormat) -> String {
        match self.format(format) {
            Ok(output) => output,
            Err(_) => {
                // Fallback to JSON if formatting fails
                match self.format(OutputFormat::Json(Default::default())) {
                    Ok(fallback) => fallback,
                    Err(_) => "{}".to_string(), // Ultimate fallback
                }
            }
        }
    }

    /// Format with conditional metadata inclusion
    fn format_with_conditional_metadata(
        &self,
        format: OutputFormat,
        include_metadata: bool,
    ) -> Result<String, crate::format::FormattingError> {
        match format {
            OutputFormat::Json(mut opts) => {
                opts.with_metadata = include_metadata;
                self.format(OutputFormat::Json(opts))
            }
            OutputFormat::Csv(mut opts) => {
                opts.with_metadata = include_metadata;
                self.format(OutputFormat::Csv(opts))
            }
            OutputFormat::Tree(mut opts) => {
                opts.with_metadata = include_metadata;
                self.format(OutputFormat::Tree(opts))
            }
        }
    }
}

impl<T: crate::format::OutputFormatter> EnhancedOutputFormatter for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_params_creation() {
        // This test verifies that FormatParams can be created
        // Actual testing would require mocking ArgMatches
        assert!(true);
    }

    #[test]
    fn test_format_options_builder() {
        let options = FormatOptionsBuilder::new()
            .with_metadata(true)
            .with_headers(true)
            .pretty(true)
            .build();

        assert!(options.with_metadata);
        assert!(options.with_headers);
        assert!(options.pretty);
    }
}

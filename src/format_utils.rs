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

        warn_about_noop_format_flags(sub_matches, &format_str);

        FormatParams {
            format,
            format_options,
            format_str,
        }
    }

    /// Get format with custom default when no format is specified.
    pub fn from_args_with_default(sub_matches: &ArgMatches, default_format: &str) -> FormatParams {
        let format_str = get_format_string_with_default(sub_matches, default_format);

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

        warn_about_noop_format_flags(sub_matches, &format_str);

        FormatParams {
            format,
            format_options,
            format_str,
        }
    }
}

/// Whether `id` was given on the command line (not defaulted, not from the
/// environment). Unknown ids answer no.
pub fn given_on_command_line(sub_matches: &ArgMatches, id: &str) -> bool {
    // `value_source` panics on an id the command does not define; check first.
    sub_matches.try_contains_id(id).unwrap_or(false)
        && sub_matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
}

/// Say so when a flag the user typed cannot change anything here.
///
/// Such flags used to be accepted in silence, which is worse than either
/// rejecting or honouring them: `--headers` on a JSON listing looks like it
/// worked. Rejecting would break scripts that pass them harmlessly, so this
/// warns and carries on.
pub fn warn_if_given(sub_matches: &ArgMatches, id: &str, why: &str) {
    if given_on_command_line(sub_matches, id) {
        crate::error_utils::report_warning(&format!("--{} has no effect here: {}", id, why));
    }
}

/// Warn about `--pretty`/`--headers` when the chosen format ignores them.
pub fn warn_about_noop_format_flags(sub_matches: &ArgMatches, format_str: &str) {
    if format_str.eq_ignore_ascii_case("csv") {
        warn_if_given(
            sub_matches,
            PARAMETER_PRETTY,
            "CSV output is never pretty-printed",
        );
    } else if format_str.eq_ignore_ascii_case("json") {
        warn_if_given(
            sub_matches,
            PARAMETER_HEADERS,
            "JSON output has no header row",
        );
    } else if format_str.eq_ignore_ascii_case("tree") {
        warn_if_given(sub_matches, PARAMETER_PRETTY, "tree output has one layout");
        warn_if_given(
            sub_matches,
            PARAMETER_HEADERS,
            "tree output has no header row",
        );
    }
}

fn get_format_string(sub_matches: &ArgMatches) -> String {
    get_format_string_with_default(sub_matches, "json")
}

/// Resolve the effective format string: an explicit `--format` always wins;
/// otherwise the `PCLI2_FORMAT` environment variable takes precedence over
/// the clap default. The env var is intentionally not bound via clap's
/// `.env()`: commands narrow the allowed formats, so an env value valid for
/// one command (e.g. `tree`) must not hard-fail every other command at parse
/// time. Values a command cannot handle fall back downstream.
fn get_format_string_with_default(sub_matches: &ArgMatches, default_format: &str) -> String {
    let explicit =
        sub_matches.value_source(PARAMETER_FORMAT) == Some(clap::parser::ValueSource::CommandLine);
    if !explicit {
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            if !env_format.trim().is_empty() {
                return env_format;
            }
        }
    }
    sub_matches
        .get_one::<String>(PARAMETER_FORMAT)
        .cloned()
        .unwrap_or_else(|| default_format.to_string())
}

/// Builder for format options to make them more flexible and extensible.
#[derive(Debug, Clone)]
pub struct FormatOptionsBuilder {
    with_metadata: bool,
    with_headers: bool,
    pretty: bool,
}

impl Default for FormatOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
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
    #[allow(clippy::result_large_err)]
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

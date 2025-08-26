//! Formatting utilities for the Physna CLI client.
//!
//! This module provides functionality for formatting output in various formats
//! including JSON, CSV, and tree representations.

use csv::Writer;
use serde::{Deserialize, Serialize};
use serde_json;
use std::io::BufWriter;
use std::str::FromStr;
use strum::EnumIter;

/// String constant for JSON format
pub const JSON: &'static str = "json";
/// String constant for CSV format
pub const CSV: &'static str = "csv";
/// String constant for tree format
pub const TREE: &'static str = "tree";

/// Error types that can occur during formatting operations
#[derive(Debug, thiserror::Error)]
pub enum FormattingError {
    /// Error when an unsupported output format is requested
    #[error("invalid output format {format:?}")]
    UnsupportedOutputFormat { format: String },
    /// General error when formatting fails
    #[error("failed to format output due to: {cause:?}")]
    FormatFailure { cause: Box<dyn std::error::Error> },
    /// Error specific to CSV operations
    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),
    /// Error when converting bytes to UTF-8 string
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    /// Error specific to CSV writer operations
    #[error("CSV writer error: {0}")]
    CsvWriterError(String),
}

/// Enum representing the supported output formats
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// CSV (Comma-Separated Values) format
    Csv,
    /// JSON (JavaScript Object Notation) format
    #[default]
    Json,
    /// Tree format for hierarchical data representation
    Tree,
}

impl OutputFormat {
    /// Returns a vector of all supported format names as strings
    pub fn names() -> Vec<&'static str> {
        vec!["json", "csv", "tree"]
    }
}

impl std::fmt::Display for OutputFormat {
    /// Formats the OutputFormat enum as a string for display purposes
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Tree => write!(f, "tree"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = FormattingError;

    /// Parses a string into an OutputFormat enum variant
    fn from_str(format_str: &str) -> Result<OutputFormat, FormattingError> {
        let normalized_format = format_str.to_lowercase();
        let normalized_format = normalized_format.as_str();
        match normalized_format {
            JSON => Ok(OutputFormat::Json),
            CSV => Ok(OutputFormat::Csv),
            TREE => Ok(OutputFormat::Tree),
            _ => Err(FormattingError::UnsupportedOutputFormat {
                format: normalized_format.to_string(),
            }),
        }
    }
}

/// Trait for formatting data in different output formats
pub trait OutputFormatter {
    /// The type of item being formatted
    type Item;
    
    /// Format the data according to the specified output format
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError>;
}

/// Trait for producing CSV records from data
pub trait CsvRecordProducer {
    /// Returns the header row for the CSV output
    fn csv_header() -> Vec<String>;

    /// Converts the data into CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>>;

    /// Produces CSV output with a header row
    fn to_csv_with_header(&self) -> Result<String, FormattingError> {
        self.to_csv(true)
    }

    /// Produces CSV output without a header row
    fn to_csv_without_header(&self) -> Result<String, FormattingError> {
        self.to_csv(false)
    }

    /// Produces CSV output with or without a header row based on the parameter
    fn to_csv(&self, with_header: bool) -> Result<String, FormattingError> {
        let buf = BufWriter::new(Vec::new());
        let mut wtr = Writer::from_writer(buf);
        if with_header {
            wtr.write_record(&Self::csv_header()).unwrap();
        }
        for record in self.as_csv_records() {
            wtr.write_record(&record).unwrap();
        }
        match wtr.flush() {
            Ok(_) => {
                let bytes = wtr.into_inner().unwrap().into_inner().unwrap();
                let csv = String::from_utf8(bytes).unwrap();
                Ok(csv.clone())
            }
            Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
        }
    }
}

/// Trait for producing JSON output from serializable data
pub trait JsonProducer {
    /// Produces pretty-printed JSON output from serializable data
    fn to_json(&self) -> Result<String, FormattingError>
    where
        Self: Serialize,
    {
        let json = serde_json::to_string_pretty(&self);
        match json {
            Ok(json) => Ok(json),
            Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
        }
    }
}

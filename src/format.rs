//! Formatting utilities for the Physna CLI client.
//!
//! This module provides functionality for formatting output in various formats
//! including JSON, CSV, and tree representations.

use csv::Writer;
use serde_json;
use std::io::BufWriter;
use std::str::FromStr;
use strum::EnumIter;

pub const JSON: &str = "json";
pub const CSV: &str = "csv";
pub const TREE: &str = "tree";

/// Error types that can occur during formatting operations
#[derive(Debug, thiserror::Error)]
pub enum FormattingError {
    /// Error when an unsupported output format is requested
    #[error("invalid output format {0}")]
    UnsupportedOutputFormat(String),
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

    #[error("JSON serialization error: {0}")]
    JsonSerializationError(#[from] serde_json::Error),

    #[error("CSV writer into inner error: {0}")]
    CsvIntoInnerError(#[from] csv::IntoInnerError<csv::Writer<Vec<u8>>>),
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct OutputFormatOptions {
    pub with_metadata: bool,
    pub with_headers: bool,
    pub pretty: bool,
}

impl Default for OutputFormatOptions {
    fn default() -> Self {
        OutputFormatOptions {
            with_metadata: false,
            with_headers: false,
            pretty: false,
        }
    }
}

/// Enum representing the supported output formats
#[derive(Debug, Clone, PartialEq, PartialOrd, EnumIter)]
pub enum OutputFormat {
    /// CSV (Comma-Separated Values) format
    Csv(OutputFormatOptions),
    /// JSON (JavaScript Object Notation) format
    Json(OutputFormatOptions),
    /// Tree format for hierarchical data representation
    Tree(OutputFormatOptions),
}

impl OutputFormat {
    /// Returns a vector of all supported format names as strings
    pub fn names() -> Vec<&'static str> {
        vec![JSON, CSV, TREE]
    }

    pub fn from_string_with_options(
        format_str: &str,
        options: OutputFormatOptions,
    ) -> Result<OutputFormat, FormattingError> {
        let normalized_format = format_str.to_lowercase();
        let normalized_format = normalized_format.as_str();
        match normalized_format {
            JSON => Ok(OutputFormat::Json(options)),
            CSV => Ok(OutputFormat::Csv(options)),
            TREE => Ok(OutputFormat::Tree(options)),
            _ => Err(FormattingError::UnsupportedOutputFormat(
                normalized_format.to_string(),
            )),
        }
    }
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Json(OutputFormatOptions::default())
    }
}

impl std::fmt::Display for OutputFormat {
    /// Formats the OutputFormat enum as a string for display purposes
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OutputFormat::Csv(_) => write!(f, "csv"),
            OutputFormat::Json(_) => write!(f, "json"),
            OutputFormat::Tree(_) => write!(f, "tree"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = FormattingError;

    /// Parses a string into an OutputFormat enum variant
    fn from_str(format_str: &str) -> Result<OutputFormat, FormattingError> {
        Self::from_string_with_options(format_str, OutputFormatOptions::default())
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

    /// Returns the header row for the CSV output with metadata columns
    fn csv_header_with_metadata() -> Vec<String> {
        Self::csv_header()
    }

    /// Converts the data into CSV records with metadata columns
    fn as_csv_records_with_metadata(&self) -> Vec<Vec<String>> {
        self.as_csv_records()
    }

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
            wtr.write_record(Self::csv_header()).unwrap();
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

pub trait Formattable {
    fn format(&self, f: &OutputFormat) -> Result<String, FormattingError>;
}

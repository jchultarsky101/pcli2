//! Formatting utilities for the Physna CLI client.
//!
//! This module provides functionality for formatting output in various formats
//! including JSON, CSV, and tree representations.

pub mod impls;

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
#[allow(clippy::large_enum_variant, clippy::result_large_err)]
pub enum FormattingError {
    /// Error when an unsupported output format is requested
    #[error("invalid output format {0}")]
    UnsupportedOutputFormat(String),
    /// General error when formatting fails
    #[error("failed to format output due to: {cause}")]
    FormatFailure {
        cause: Box<dyn std::error::Error + Send + Sync>,
    },
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
    CsvIntoInnerError(#[source] Box<csv::IntoInnerError<csv::Writer<Vec<u8>>>>),
}

// The CSV writer error embeds the whole `csv::Writer` (buffer and state), which
// made every `Result<_, FormattingError>` several hundred bytes wide and tripped
// clippy's `result_large_err`. Boxing it keeps the `?` conversion working.
impl From<csv::IntoInnerError<csv::Writer<Vec<u8>>>> for FormattingError {
    fn from(e: csv::IntoInnerError<csv::Writer<Vec<u8>>>) -> Self {
        FormattingError::CsvIntoInnerError(Box::new(e))
    }
}

/// Print a formatted result to stdout, followed by exactly one line break.
///
/// Nothing at all is printed for an empty result: an empty folder listed as
/// CSV without headers used to print one blank line, which `wc -l` counted
/// and a spreadsheet import turned into an empty row.
pub fn print_output(text: &str) {
    if !text.is_empty() {
        println!("{}", text);
    }
}

static SAFE_CSV: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Switch on formula guarding for every CSV cell this process writes.
pub fn set_safe_csv(safe: bool) {
    SAFE_CSV.store(safe, std::sync::atomic::Ordering::SeqCst);
}

/// Whether `--safe-csv` (or `PCLI2_SAFE_CSV`) is in effect.
pub fn safe_csv() -> bool {
    SAFE_CSV.load(std::sync::atomic::Ordering::SeqCst)
}

/// Whether a cell would be read as a formula by a spreadsheet opening the CSV.
///
/// The trigger characters are the ones Excel, LibreOffice and Google Sheets
/// evaluate at the start of a cell. A value that parses as a number is not a
/// formula, so `-5` and `+3.2` are left alone: prefixing them would turn every
/// negative measurement into text.
pub fn looks_like_formula(cell: &str) -> bool {
    let Some(first) = cell.chars().next() else {
        return false;
    };
    matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r') && cell.trim().parse::<f64>().is_err()
}

/// The cell as it should be written: unchanged unless `--safe-csv` is on and
/// it looks like a formula, in which case it gets a leading single quote,
/// which spreadsheets treat as "this is text".
pub fn guard_csv_cell(cell: &str) -> std::borrow::Cow<'_, str> {
    if safe_csv() && looks_like_formula(cell) {
        std::borrow::Cow::Owned(format!("'{}", cell))
    } else {
        std::borrow::Cow::Borrowed(cell)
    }
}

/// A row with every cell guarded, or the row itself when nothing needs guarding.
pub fn guard_csv_row(row: &[String]) -> std::borrow::Cow<'_, [String]> {
    if safe_csv() && row.iter().any(|cell| looks_like_formula(cell)) {
        std::borrow::Cow::Owned(
            row.iter()
                .map(|cell| guard_csv_cell(cell).into_owned())
                .collect(),
        )
    } else {
        std::borrow::Cow::Borrowed(row)
    }
}

/// Rewrite finished CSV text with every cell guarded. Used by the one place
/// all buffered CSV output passes through, so no formatter has to know.
fn guard_csv_text(text: &str) -> String {
    if !text
        .chars()
        .any(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'))
    {
        return text.to_string();
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut writer = csv::WriterBuilder::new()
        .flexible(true)
        .from_writer(Vec::new());
    for record in reader.records() {
        let Ok(record) = record else {
            // Not something we produced; leave it as it is rather than guess.
            return text.to_string();
        };
        let guarded: Vec<String> = record
            .iter()
            .map(|cell| guard_csv_cell(cell).into_owned())
            .collect();
        if writer.write_record(&guarded).is_err() {
            return text.to_string();
        }
    }
    match writer.into_inner() {
        Ok(bytes) => String::from_utf8(bytes)
            .map(|t| t.trim_end_matches(['\r', '\n']).to_string())
            .unwrap_or_else(|_| text.to_string()),
        Err(_) => text.to_string(),
    }
}

/// The bytes a CSV writer produced, as text, without the trailing line break.
///
/// The writer terminates its last record with a newline and every command prints
/// the result with `println!`, which added a second one: `asset list --format csv
/// | wc -l` reported 117 lines for 116 assets, and a CSV opened in a spreadsheet
/// gained an empty last row.
pub fn csv_text(data: Vec<u8>) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(data).map(|text| {
        let text = text.trim_end_matches(['\r', '\n']);
        if safe_csv() {
            guard_csv_text(text)
        } else {
            text.to_string()
        }
    })
}

#[cfg(test)]
mod safe_csv_tests {
    use super::*;

    #[test]
    fn formula_detection_spares_numbers() {
        for formula in [
            "=SUM(A1)",
            "+cmd|' /C calc'!A0",
            "-2+3",
            "@SUM(1)",
            "\tx",
            "-",
            "+",
        ] {
            assert!(looks_like_formula(formula), "{formula:?}");
        }
        for plain in [
            "",
            "bracket.stl",
            "-5",
            "+3.25",
            "-1e3",
            " -7 ",
            "5-3",
            "a=b",
        ] {
            assert!(!looks_like_formula(plain), "{plain:?}");
        }
    }

    #[test]
    fn guarding_rewrites_only_formula_cells_and_keeps_quoting_valid() {
        let input = "NAME,PATH\n=SUM(A1),\"/a, b\"\n-5,plain\n";
        // Off: untouched apart from the trailing line break.
        set_safe_csv(false);
        assert_eq!(
            csv_text(input.as_bytes().to_vec()).unwrap(),
            input.trim_end()
        );
        // On: the formula cell is prefixed; the number and the quoted cell are not.
        set_safe_csv(true);
        let out = csv_text(input.as_bytes().to_vec()).unwrap();
        let row = guard_csv_row(&["=1+1".to_string(), "-5".to_string()]).into_owned();
        set_safe_csv(false);
        assert_eq!(out, "NAME,PATH\n'=SUM(A1),\"/a, b\"\n-5,plain");
        assert_eq!(row, vec!["'=1+1", "-5"]);
        assert!(!out.ends_with('\n'));
        // Off again: the streaming guard is a no-op.
        assert_eq!(
            guard_csv_row(&["=1+1".to_string()]).as_ref(),
            &["=1+1".to_string()]
        );
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct OutputFormatOptions {
    pub with_metadata: bool,
    pub with_headers: bool,
    pub pretty: bool,
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

    #[allow(clippy::result_large_err)]
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

    /// Enhanced format validation with better error handling and recovery
    #[allow(clippy::result_large_err)]
    pub fn from_string_with_options_safe(
        format_str: &str,
        options: OutputFormatOptions,
    ) -> Result<OutputFormat, FormattingError> {
        let normalized_format = format_str.trim().to_lowercase();

        // Validate format string before processing
        if !Self::is_valid_format(&normalized_format) {
            return Err(FormattingError::UnsupportedOutputFormat(
                format_str.to_string(),
            ));
        }

        match normalized_format.as_str() {
            JSON => Ok(OutputFormat::Json(options)),
            CSV => Ok(OutputFormat::Csv(options)),
            TREE => Ok(OutputFormat::Tree(options)),
            _ => Err(FormattingError::UnsupportedOutputFormat(
                format_str.to_string(),
            )),
        }
    }

    fn is_valid_format(format: &str) -> bool {
        matches!(format, "json" | "csv" | "tree")
    }

    /// Get all supported format names
    pub fn supported_formats() -> &'static [&'static str] {
        &[JSON, CSV, TREE]
    }

    /// Check if a format string is supported (case-insensitive)
    pub fn supports_format(format_str: &str) -> bool {
        Self::supported_formats()
            .iter()
            .any(|&supported| supported.eq_ignore_ascii_case(format_str))
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
#[allow(clippy::result_large_err)]
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
    #[allow(clippy::result_large_err)]
    fn to_csv_with_header(&self) -> Result<String, FormattingError> {
        self.to_csv(true)
    }

    /// Produces CSV output without a header row
    #[allow(clippy::result_large_err)]
    fn to_csv_without_header(&self) -> Result<String, FormattingError> {
        self.to_csv(false)
    }

    /// Produces CSV output with or without a header row based on the parameter
    #[allow(clippy::result_large_err)]
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
                let csv = crate::format::csv_text(bytes).unwrap();
                Ok(csv.clone())
            }
            Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
        }
    }
}

pub trait Formattable {
    #[allow(clippy::result_large_err)]
    fn format(&self, f: &OutputFormat) -> Result<String, FormattingError>;
}

#[cfg(test)]
mod csv_text_tests {
    #[test]
    fn csv_text_drops_only_the_trailing_line_break() {
        let text = super::csv_text(b"A,B\n1,2\n".to_vec()).unwrap();
        assert_eq!(text, "A,B\n1,2");
        let crlf = super::csv_text(b"A,B\r\n1,2\r\n".to_vec()).unwrap();
        assert_eq!(crlf, "A,B\r\n1,2");
        assert_eq!(super::csv_text(b"".to_vec()).unwrap(), "");
    }
}

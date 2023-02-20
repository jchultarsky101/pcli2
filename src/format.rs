use csv::Writer;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{io::BufWriter, str::FromStr};
use strum::EnumIter;

pub const JSON: &'static str = "json";
pub const CSV: &'static str = "csv";
pub const TREE: &'static str = "tree";

#[derive(Debug, thiserror::Error)]
pub enum FormattingError {
    #[error("invalid output format {format:?}")]
    UnsupportedOutputFormat { format: String },
    #[error("failed to format output due to: {cause:?}")]
    FormatFailure { cause: Box<dyn std::error::Error> },
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Csv,
    CsvPretty,
    #[default]
    Json,
    JsonPretty,
    Table,
    TablePretty,
    Tree,
    TreePretty,
}

impl OutputFormat {
    pub fn names() -> Vec<&'static str> {
        vec![
            "json",
            "json_pretty",
            "csv",
            "csv_pretty",
            "table",
            "table_pretty",
            "tree",
            "tree_pretty",
        ]
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::CsvPretty => write!(f, "csv_pretty"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::JsonPretty => write!(f, "json_pretty"),
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::TablePretty => write!(f, "table_pretty"),
            OutputFormat::Tree => write!(f, "tree"),
            OutputFormat::TreePretty => write!(f, "tree_pretty"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = FormattingError;

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

pub trait OutputFormatter {
    type Item;
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError>;
}

pub trait CsvRecordProducer {
    fn csv_header() -> Vec<String>;

    fn as_csv_records(&self) -> Vec<Vec<String>>;

    fn to_csv(&self) -> Result<String, FormattingError> {
        let buf = BufWriter::new(Vec::new());
        let mut wtr = Writer::from_writer(buf);
        wtr.write_record(&Self::csv_header()).unwrap();
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

pub trait JsonProducer {
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

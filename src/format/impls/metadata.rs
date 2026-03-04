//! Formatting implementations for AssetMetadata.
//!
//! This module provides CSV and JSON formatting for asset metadata.

use crate::format::{FormattingError, OutputFormat, OutputFormatter};
use crate::model::AssetMetadata;
use csv::Writer;
use serde_json;

impl OutputFormatter for AssetMetadata {
    type Item = AssetMetadata;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.write_record(vec!["NAME", "VALUE"])?;
                }

                // Sort records by metadata key
                let mut records: Vec<Vec<String>> = self
                    .keys()
                    .map(|k| vec![k.to_owned(), self.get(k).cloned().unwrap_or_default()])
                    .collect();
                records.sort_by(|a, b| a[0].cmp(&b[0]));

                for record in records {
                    wtr.write_record(&record).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV record: {}",
                            e
                        ))
                    })?;
                }
                let data = wtr.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
                })?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

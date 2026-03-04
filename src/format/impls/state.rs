//! Formatting implementations for asset state counts.
//!
//! This module provides CSV, JSON, and Tree formatting for asset state counts.

use crate::format::{Formattable, FormattingError, OutputFormat};
use crate::model::AssetStateCounts;

impl AssetStateCounts {
    /// Helper method to get the value or 0 if None
    fn get_or_default(value: Option<u32>) -> u32 {
        value.unwrap_or(0)
    }
}

impl Formattable for AssetStateCounts {
    fn format(
        &self,
        f: &OutputFormat,
    ) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                match json {
                    Ok(json_str) => Ok(json_str),
                    Err(e) => Err(FormattingError::JsonSerializationError(e)),
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "INDEXING",
                        "FINISHED",
                        "FAILED",
                        "UNSUPPORTED",
                        "NO-3D-DATA",
                    ))?;
                }

                wtr.serialize((
                    AssetStateCounts::get_or_default(self.processing),
                    AssetStateCounts::get_or_default(self.ready),
                    AssetStateCounts::get_or_default(self.failed),
                    AssetStateCounts::get_or_default(self.unsupported),
                    AssetStateCounts::get_or_default(self.no_3d_data),
                ))?;

                let data = wtr.into_inner()?;
                let csv_string = String::from_utf8(data)?;
                Ok(csv_string)
            }
            OutputFormat::Tree(_) => {
                // For tree format, just return a simple representation
                Ok(format!(
                    "Asset State Counts:\n  Processing: {}\n  Ready: {}\n  Failed: {}\n  Unsupported: {}\n  No 3D Data: {}",
                    AssetStateCounts::get_or_default(self.processing),
                    AssetStateCounts::get_or_default(self.ready),
                    AssetStateCounts::get_or_default(self.failed),
                    AssetStateCounts::get_or_default(self.unsupported),
                    AssetStateCounts::get_or_default(self.no_3d_data)
                ))
            }
        }
    }
}

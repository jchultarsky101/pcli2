//! Formatting implementations for Folder and FolderList.
//!
//! This module provides CSV and JSON formatting for folder-related data structures.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{Folder, FolderList};
use csv::Writer;

impl CsvRecordProducer for Folder {
    /// Get the CSV header row for Folder records
    fn csv_header() -> Vec<String> {
        vec![
            "NAME".to_string(),
            "PATH".to_string(),
            "ASSETS_COUNT".to_string(),
            "FOLDERS_COUNT".to_string(),
            "UUID".to_string(),
        ]
    }

    /// Convert the Folder to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.name(),
            self.path(),
            self.assets_count().to_string(),
            self.folders_count().to_string(),
            self.uuid().to_string(),
        ]]
    }

    /// Generate CSV output with a header row
    fn to_csv(&self, with_headers: bool) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);

        if with_headers {
            wtr.write_record(Self::csv_header()).map_err(|e| {
                FormattingError::CsvWriterError(format!("Failed to write CSV header: {}", e))
            })?;
        }

        // Sort records by folder name
        let mut records = self.as_csv_records();
        records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)

        for record in records {
            wtr.write_record(&record).map_err(|e| {
                FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e))
            })?;
        }
        let data = wtr.into_inner().map_err(|e| {
            FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
        })?;
        String::from_utf8(data).map_err(FormattingError::Utf8Error)
    }
}

impl OutputFormatter for Folder {
    type Item = Folder;

    /// Format the Folder according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CsvRecordProducer for FolderList {
    /// Get the CSV header row for FolderList records
    fn csv_header() -> Vec<String> {
        Folder::csv_header()
    }

    /// Convert the FolderList to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for folder in self.iter() {
            records.push(folder.as_csv_records()[0].clone());
        }

        records
    }
}

impl OutputFormatter for FolderList {
    type Item = FolderList;

    /// Format the FolderList according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                // convert to a simple vector for output, sorted by name
                let mut folders: Vec<Folder> = self.folders();
                folders.sort_by_key(|a| a.name());
                let json = if options.pretty {
                    serde_json::to_string_pretty(&folders)
                } else {
                    serde_json::to_string(&folders)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.write_record(Self::csv_header())?;
                }

                // Sort records by folder name
                let mut records = self.as_csv_records();
                records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)

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
            OutputFormat::Tree(_) => {
                // For folder list, tree format is the same as JSON
                // In practice, tree format should be handled at the command level
                // where we have access to the full hierarchy
                let mut folders: Vec<Folder> = self.folders();
                folders.sort_by_key(|a| a.name());
                let json = serde_json::to_string_pretty(&folders);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
        }
    }
}

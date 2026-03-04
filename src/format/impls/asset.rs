//! Formatting implementations for Asset, AssetList, AssetWithThumbnail, and AssetListWithThumbnails.
//!
//! This module provides CSV, JSON, and Tree formatting for asset-related data structures,
//! including support for metadata columns in CSV output.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{Asset, AssetList, AssetListWithThumbnails, AssetWithThumbnail};
use csv::Writer;
use serde_json;
use std::collections::HashSet;
use std::io::BufWriter;

impl CsvRecordProducer for Asset {
    /// Get the CSV header row for Asset records
    fn csv_header() -> Vec<String> {
        vec![
            "NAME".to_string(),
            "PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "UUID".to_string(),
        ]
    }

    /// Convert the Asset to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.name(),
            self.path(),
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default(),
            self.is_assembly().to_string(),
            self.uuid().to_string(),
        ]]
    }

    /// Get the extended CSV header row for Asset records including metadata
    fn csv_header_with_metadata() -> Vec<String> {
        // We'll add metadata columns dynamically when we know what metadata keys exist
        Self::csv_header()
    }

    /// Convert the Asset to CSV records including metadata
    fn as_csv_records_with_metadata(&self) -> Vec<Vec<String>> {
        let record = vec![
            self.name(),
            self.path(),
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default(),
            self.is_assembly().to_string(),
            self.uuid().to_string(),
        ];

        // Add metadata values if they exist
        if let Some(_metadata) = self.metadata() {
            // We'll need to collect all unique metadata keys when building the CSV
            // For now, we just return the basic record without metadata columns
            // The metadata will be added when building the full CSV with dynamic columns
        }

        vec![record]
    }
}

impl Asset {
    /// Generate CSV output with metadata columns
    #[allow(clippy::result_large_err)]
    pub fn to_csv_with_metadata(&self, with_headers: bool) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);

        // Get all unique metadata keys to create headers
        let mut metadata_keys = std::collections::HashSet::new();
        if let Some(metadata) = self.metadata() {
            for key in metadata.keys() {
                metadata_keys.insert(key.clone());
            }
        }

        // Sort metadata keys for consistent column ordering
        let mut sorted_keys: Vec<String> = metadata_keys.into_iter().collect();
        sorted_keys.sort();

        if with_headers {
            let mut header_row = vec![
                "NAME".to_string(),
                "PATH".to_string(),
                "TYPE".to_string(),
                "STATE".to_string(),
                "IS_ASSEMBLY".to_string(),
                "UUID".to_string(),
            ];

            // Add metadata column headers
            for key in &sorted_keys {
                header_row.push(format!("META_{}", key.to_uppercase()));
            }

            wtr.write_record(&header_row).map_err(|e| {
                FormattingError::CsvWriterError(format!("Failed to write CSV header: {}", e))
            })?;
        }

        // Create the data row
        let mut data_row = vec![
            self.name(),
            self.path(),
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default(),
            self.is_assembly().to_string(),
            self.uuid().to_string(),
        ];

        // Add metadata values in the same order as headers
        if let Some(metadata) = self.metadata() {
            for key in &sorted_keys {
                if let Some(value) = metadata.get(key) {
                    data_row.push(value.clone());
                } else {
                    data_row.push("".to_string());
                }
            }
        } else {
            // If no metadata, fill with empty strings
            for _ in 0..sorted_keys.len() {
                data_row.push("".to_string());
            }
        }

        wtr.write_record(&data_row).map_err(|e| {
            FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e))
        })?;

        let data = wtr.into_inner().map_err(|e| {
            FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
        })?;
        String::from_utf8(data).map_err(FormattingError::Utf8Error)
    }

    /// Generate tree output with metadata
    #[allow(clippy::result_large_err)]
    pub fn to_tree_with_metadata(
        &self,
        _options: crate::format::OutputFormatOptions,
    ) -> Result<String, FormattingError> {
        // For tree format, we'll show the asset with its metadata
        let mut result = format!("Asset: {} [{}]", self.name(), self.uuid());

        if let Some(metadata) = self.metadata() {
            if metadata.keys().count() > 0 {
                result.push_str("\n  Metadata:");
                for key in metadata.keys() {
                    if let Some(value) = metadata.get(key) {
                        result.push_str(&format!("\n    {}: {}", key, value));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Generate tree output without metadata
    #[allow(clippy::result_large_err)]
    pub fn to_tree(
        &self,
        _options: crate::format::OutputFormatOptions,
    ) -> Result<String, FormattingError> {
        Ok(format!("Asset: {} [{}]", self.name(), self.uuid()))
    }

    /// Format the Asset with consideration for metadata flag
    #[allow(clippy::result_large_err)]
    pub fn format_with_metadata_flag(
        &self,
        f: OutputFormat,
        include_metadata: bool,
    ) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                // For CSV format with metadata, we need to include metadata fields as columns
                if include_metadata {
                    self.to_csv_with_metadata(options.with_headers)
                } else {
                    Ok(self.to_csv(options.with_headers)?)
                }
            }
            OutputFormat::Tree(options) => {
                // For tree format with metadata, we can include metadata information
                if include_metadata {
                    self.to_tree_with_metadata(options)
                } else {
                    self.to_tree(options)
                }
            }
        }
    }
}

impl OutputFormatter for Asset {
    type Item = Asset;

    /// Format the Asset according to the specified output format
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

impl CsvRecordProducer for AssetList {
    /// Get the CSV header row for AssetList records
    fn csv_header() -> Vec<String> {
        Asset::csv_header()
    }

    /// Convert the AssetList to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for asset in self.iter() {
            records.push(asset.as_csv_records()[0].clone());
        }

        records
    }
}

impl AssetList {
    /// Convert the AssetList to CSV records including metadata
    ///
    /// This method converts the AssetList to CSV records with additional metadata columns.
    ///
    /// # Arguments
    /// * `metadata_keys` - Sorted list of metadata keys to include as columns
    ///
    /// # Returns
    /// Vector of CSV records with metadata columns
    fn as_csv_records_with_metadata(&self, metadata_keys: &[String]) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for asset in self.iter() {
            // Start with standard asset record
            let mut record = vec![
                asset.name(),
                asset.path(),
                asset.file_type().cloned().unwrap_or_default(),
                asset.processing_status().cloned().unwrap_or_default(),
                asset.is_assembly().to_string(),
                asset.uuid().to_string(),
            ];

            // Add metadata values for each key
            if let Some(metadata) = asset.metadata() {
                for key in metadata_keys {
                    let value = metadata.get(key).cloned().unwrap_or_default();
                    record.push(value);
                }
            } else {
                // No metadata, add empty strings for all metadata columns
                for _ in metadata_keys {
                    record.push(String::new());
                }
            }

            records.push(record);
        }

        records
    }
}

impl OutputFormatter for AssetList {
    type Item = AssetList;

    /// Format the AssetList according to the specified output format
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
                // convert to a simple vector for output and sort by path
                let mut assets: Vec<Asset> = self.get_all_assets().into_iter().cloned().collect();
                assets.sort_by_key(|a| a.path());
                let json = if options.pretty {
                    serde_json::to_string_pretty(&assets)
                } else {
                    serde_json::to_string(&assets)
                };

                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                if options.with_metadata {
                    // For CSV with metadata, we need to collect all unique metadata keys first
                    let mut metadata_keys: HashSet<String> = HashSet::new();

                    // Collect all unique metadata keys
                    for asset in self.iter() {
                        if let Some(metadata) = asset.metadata() {
                            for key in metadata.keys() {
                                metadata_keys.insert(key.clone());
                            }
                        }
                    }

                    // Convert to sorted vector for consistent column ordering
                    let mut sorted_metadata_keys: Vec<String> = metadata_keys.into_iter().collect();
                    sorted_metadata_keys.sort();

                    // Build CSV with metadata columns
                    let buf = BufWriter::new(Vec::new());
                    let mut wtr = Writer::from_writer(buf);

                    if options.with_headers {
                        // Build header with metadata columns
                        let mut header = Asset::csv_header();
                        for key in &sorted_metadata_keys {
                            header.push(key.clone());
                        }
                        wtr.write_record(&header).unwrap();
                    }

                    // Sort records by asset path
                    let mut records = self.as_csv_records_with_metadata(&sorted_metadata_keys);
                    records.sort_by(|a, b| a[1].cmp(&b[1])); // Sort by PATH column (index 1)

                    for record in records {
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
                } else {
                    let buf = BufWriter::new(Vec::new());
                    let mut wtr = Writer::from_writer(buf);

                    if options.with_headers {
                        wtr.write_record(Self::csv_header()).unwrap();
                    }

                    // Sort records by asset path
                    let mut records = self.as_csv_records();
                    records.sort_by(|a, b| a[1].cmp(&b[1])); // Sort by PATH column (index 1)

                    for record in records {
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
            OutputFormat::Tree(_) => {
                // For asset list, tree format is the same as JSON
                // In practice, tree format should be handled at the command level
                // where we have access to the full hierarchy
                // convert to a simple vector for output and sort by path
                let mut assets: Vec<Asset> = self.get_all_assets().into_iter().cloned().collect();
                assets.sort_by_key(|a| a.path());
                let json = serde_json::to_string_pretty(&assets);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
        }
    }
}

impl CsvRecordProducer for AssetWithThumbnail {
    /// Get the CSV header row for AssetWithThumbnail records
    fn csv_header() -> Vec<String> {
        vec![
            "NAME".to_string(),
            "PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "UUID".to_string(),
            "THUMBNAIL_URL".to_string(),
        ]
    }

    /// Convert the AssetWithThumbnail to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.asset.name(),
            self.asset.path(),
            self.asset.file_type().cloned().unwrap_or_default(),
            self.asset.processing_status().cloned().unwrap_or_default(),
            self.asset.is_assembly().to_string(),
            self.asset.uuid().to_string(),
            self.thumbnail_url.clone(),
        ]]
    }

    /// Get the extended CSV header row for AssetWithThumbnail records including metadata
    fn csv_header_with_metadata() -> Vec<String> {
        // We'll add metadata columns dynamically when we know what metadata keys exist
        Self::csv_header()
    }
}

impl OutputFormatter for AssetWithThumbnail {
    type Item = AssetWithThumbnail;

    /// Format the AssetWithThumbnail according to the specified output format
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                if options.pretty {
                    serde_json::to_string_pretty(self)
                        .map_err(FormattingError::JsonSerializationError)
                } else {
                    serde_json::to_string(self).map_err(FormattingError::JsonSerializationError)
                }
            }
            OutputFormat::Csv(options) => {
                let buf = BufWriter::new(Vec::new());
                let mut wtr = Writer::from_writer(buf);

                if options.with_headers {
                    wtr.write_record(Self::csv_header()).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV header: {}",
                            e
                        ))
                    })?;
                }

                for record in self.as_csv_records() {
                    wtr.write_record(&record).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV record: {}",
                            e
                        ))
                    })?;
                }

                wtr.flush().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to flush CSV writer: {}", e))
                })?;

                let data = wtr.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!(
                        "Failed to get inner data from CSV writer: {}",
                        e
                    ))
                })?;
                let bytes = data.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to get inner buffer: {}", e))
                })?;
                String::from_utf8(bytes).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(options) => {
                // For single asset with thumbnail, tree format is the same as JSON
                if options.pretty {
                    serde_json::to_string_pretty(self)
                        .map_err(FormattingError::JsonSerializationError)
                } else {
                    serde_json::to_string(self).map_err(FormattingError::JsonSerializationError)
                }
            }
        }
    }
}

impl CsvRecordProducer for AssetListWithThumbnails {
    /// Get the CSV header row for AssetListWithThumbnails records
    fn csv_header() -> Vec<String> {
        AssetWithThumbnail::csv_header()
    }

    /// Convert the AssetListWithThumbnails to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for asset_with_thumbnail in &self.assets {
            records.push(asset_with_thumbnail.as_csv_records()[0].clone());
        }

        records
    }

    /// Get the extended CSV header row for AssetListWithThumbnails records including metadata
    fn csv_header_with_metadata() -> Vec<String> {
        // We'll add metadata columns dynamically when we know what metadata keys exist
        Self::csv_header()
    }
}

impl OutputFormatter for AssetListWithThumbnails {
    type Item = AssetListWithThumbnails;

    /// Format the AssetListWithThumbnails according to the specified output format
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                if options.pretty {
                    serde_json::to_string_pretty(&self.assets)
                        .map_err(FormattingError::JsonSerializationError)
                } else {
                    serde_json::to_string(&self.assets)
                        .map_err(FormattingError::JsonSerializationError)
                }
            }
            OutputFormat::Csv(options) => {
                let buf = BufWriter::new(Vec::new());
                let mut wtr = Writer::from_writer(buf);

                if options.with_headers {
                    wtr.write_record(Self::csv_header()).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV header: {}",
                            e
                        ))
                    })?;
                }

                for asset_with_thumbnail in &self.assets {
                    for record in asset_with_thumbnail.as_csv_records() {
                        wtr.write_record(&record).map_err(|e| {
                            FormattingError::CsvWriterError(format!(
                                "Failed to write CSV record: {}",
                                e
                            ))
                        })?;
                    }
                }

                wtr.flush().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to flush CSV writer: {}", e))
                })?;

                let data = wtr.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!(
                        "Failed to get inner data from CSV writer: {}",
                        e
                    ))
                })?;
                let bytes = data.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to get inner buffer: {}", e))
                })?;
                String::from_utf8(bytes).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(_options) => {
                // For asset list with thumbnails, tree format is the same as JSON
                serde_json::to_string_pretty(&self.assets)
                    .map_err(FormattingError::JsonSerializationError)
            }
        }
    }
}

//! Formatting implementations for search and match operations.
//!
//! This module provides CSV and JSON formatting for part search, geometric search,
//! and visual match response types.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{
    EnhancedGeometricSearchResponse, EnhancedPartSearchResponse, FolderGeometricMatch,
    FolderGeometricMatchResponse, GeometricMatchPair, GeometricSearchResponse, PartMatchPair,
    PartSearchResponse, VisualMatchPair,
};

impl CsvRecordProducer for PartSearchResponse {
    /// Get the CSV header row for PartSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_ID".to_string(),
            "PATH".to_string(),
            "FORWARD_SCORE".to_string(),
            "REVERSE_SCORE".to_string(),
        ]
    }

    /// Convert the PartSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    m.asset_uuid().to_string(),
                    m.path().to_string(),
                    format!("{}", m.forward_score()), // Full precision
                    format!("{}", m.reverse_score()), // Full precision
                ]
            })
            .collect()
    }
}

impl CsvRecordProducer for EnhancedPartSearchResponse {
    /// Get the CSV header row for EnhancedPartSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "FORWARD_MATCH_PERCENTAGE".to_string(),
            "REVERSE_MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the EnhancedPartSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    self.reference_asset.path.clone(),
                    m.path().to_string(),
                    format!("{}", m.forward_score()), // Full precision
                    format!("{}", m.reverse_score()), // Full precision
                    self.reference_asset.uuid.to_string(),
                    m.asset_uuid().to_string(),
                    m.comparison_url.clone().unwrap_or_default(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for EnhancedPartSearchResponse {
    type Item = EnhancedPartSearchResponse;

    /// Format the EnhancedPartSearchResponse according to the specified output format
    ///
    /// This method formats the EnhancedPartSearchResponse based on the requested format:
    /// - JSON: Outputs as JSON with optional pretty printing
    /// - CSV: Outputs as CSV with optional headers
    /// - Tree: Not supported for this type
    #[allow(clippy::result_large_err)]
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        // Extract the metadata flag from the format options
        let with_metadata = match &f {
            OutputFormat::Json(options) => options.with_metadata,
            OutputFormat::Csv(options) => options.with_metadata,
            OutputFormat::Tree(options) => options.with_metadata,
        };

        self.format_with_metadata_flag(f, with_metadata)
    }
}

impl EnhancedPartSearchResponse {
    /// Format the EnhancedPartSearchResponse with consideration for metadata flag
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
                let mut wtr = csv::Writer::from_writer(vec![]);

                // Pre-calculate the metadata keys that will be used for both header and all records to ensure consistency
                let mut header_metadata_keys = Vec::new();
                if include_metadata {
                    // Collect all unique metadata keys from ALL matches for consistent headers
                    let mut all_metadata_keys = std::collections::HashSet::new();

                    // Collect metadata keys from reference asset
                    for key in self.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Collect metadata keys from all matching assets
                    for match_result in &self.matches {
                        for key in match_result.asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();
                    header_metadata_keys = sorted_keys;
                }

                if options.with_headers {
                    if include_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = Self::csv_header();

                        // Extend headers with metadata columns using the pre-calculated keys
                        for key in &header_metadata_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(EnhancedPartSearchResponse::csv_header())?;
                    }
                }

                for match_result in &self.matches {
                    if include_metadata {
                        // Include metadata values in the output
                        let mut base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.forward_score()),
                            format!("{}", match_result.reverse_score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];

                        // Add metadata values for each key that was included in the header
                        for key in &header_metadata_keys {
                            // Add reference asset metadata value (same for all records)
                            let ref_value = self
                                .reference_asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(ref_value);

                            // Add candidate asset metadata value (specific to this match)
                            let cand_value = match_result
                                .asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(cand_value);
                        }

                        wtr.serialize(base_values.as_slice())?;
                    } else {
                        let base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.forward_score()),
                            format!("{}", match_result.reverse_score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];
                        wtr.serialize(base_values.as_slice())?;
                    }
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CsvRecordProducer for PartMatchPair {
    /// Get the CSV header row for PartMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "SCORE_1".to_string(), // Generic score field that can be forward match % for geometric/part or empty for visual
            "SCORE_2".to_string(), // Generic score field that can be reverse match % for geometric/part or empty for visual
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the PartMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset.path.clone(),
            self.candidate_asset.path.clone(),
            format!("{}", self.forward_match_percentage.unwrap_or(0.0)),
            format!("{}", self.reverse_match_percentage.unwrap_or(0.0)),
            self.reference_asset.uuid.to_string(),
            self.candidate_asset.uuid.to_string(),
            self.comparison_url.clone().unwrap_or_default(),
        ]]
    }
}

impl CsvRecordProducer for VisualMatchPair {
    /// Get the CSV header row for VisualMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the VisualMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset.path.clone(),
            self.candidate_asset.path.clone(),
            self.reference_asset.uuid.to_string(),
            self.candidate_asset.uuid.to_string(),
            self.comparison_url.clone().unwrap_or_default(),
        ]]
    }
}

impl CsvRecordProducer for FolderGeometricMatch {
    /// Get the CSV header row for FolderGeometricMatch records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_NAME".to_string(),
            "CANDIDATE_ASSET_NAME".to_string(),
            "MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the FolderGeometricMatch to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset_name.clone(),
            self.candidate_asset_name.clone(),
            format!("{:.2}", self.match_percentage),
            self.reference_asset_path.clone(),
            self.candidate_asset_path.clone(),
            self.reference_asset_uuid.to_string(),
            self.candidate_asset_uuid.to_string(),
            self.comparison_url.clone(),
        ]]
    }
}

impl CsvRecordProducer for FolderGeometricMatchResponse {
    /// Get the CSV header row for FolderGeometricMatchResponse records
    fn csv_header() -> Vec<String> {
        FolderGeometricMatch::csv_header()
    }

    /// Convert the FolderGeometricMatchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.iter().flat_map(|m| m.as_csv_records()).collect()
    }
}

impl OutputFormatter for FolderGeometricMatchResponse {
    type Item = FolderGeometricMatchResponse;

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

impl CsvRecordProducer for GeometricSearchResponse {
    /// Get the CSV header row for GeometricSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_ID".to_string(),
            "PATH".to_string(),
            "SCORE".to_string(),
        ]
    }

    /// Convert the GeometricSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    m.asset_uuid().to_string(),
                    m.path().to_string(),
                    format!("{}", m.score()), // Full precision
                ]
            })
            .collect()
    }
}

impl CsvRecordProducer for GeometricMatchPair {
    /// Get the CSV header row for GeometricMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the GeometricMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset.path.clone(),
            self.candidate_asset.path.clone(),
            format!("{}", self.match_percentage), // Full precision
            self.reference_asset.uuid.to_string(),
            self.candidate_asset.uuid.to_string(),
            self.comparison_url.clone().unwrap_or_default(),
        ]]
    }
}

impl OutputFormatter for GeometricMatchPair {
    type Item = GeometricMatchPair;

    /// Format the GeometricMatchPair according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
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
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = GeometricMatchPair::csv_header();

                        // Get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from candidate asset
                        for key in self.candidate_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(GeometricMatchPair::csv_header())?;
                    }
                }

                if options.with_metadata {
                    // Include metadata values in the output
                    let mut base_values = vec![
                        self.reference_asset.path.clone(),
                        self.candidate_asset.path.clone(),
                        format!("{}", self.match_percentage),
                        self.reference_asset.uuid.to_string(),
                        self.candidate_asset.uuid.to_string(),
                        self.comparison_url.clone().unwrap_or_default(),
                    ];

                    // Get unique metadata keys from both reference and candidate assets
                    let mut all_metadata_keys = std::collections::HashSet::new();

                    // Collect metadata keys from reference asset
                    for key in self.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Collect metadata keys from candidate asset
                    for key in self.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Add metadata values for each key
                    for key in &sorted_keys {
                        // Add reference asset metadata value
                        let ref_value = self
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = self
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }

                    wtr.serialize(base_values.as_slice())?;
                } else {
                    wtr.serialize((
                        &self.reference_asset.path,
                        &self.candidate_asset.path,
                        &self.match_percentage,
                        &self.reference_asset.uuid.to_string(),
                        &self.candidate_asset.uuid.to_string(),
                        &self.comparison_url.clone().unwrap_or_default(),
                    ))?;
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CsvRecordProducer for EnhancedGeometricSearchResponse {
    /// Get the CSV header row for EnhancedGeometricSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the EnhancedGeometricSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    self.reference_asset.path.clone(),     // Reference asset path
                    m.path().to_string(),                  // Candidate asset path
                    format!("{}", m.score()),              // Full precision match percentage
                    self.reference_asset.uuid.to_string(), // Reference asset UUID
                    m.asset_uuid().to_string(),            // Candidate asset UUID
                    m.comparison_url.clone().unwrap_or_default(), // Comparison URL
                ]
            })
            .collect()
    }
}

impl OutputFormatter for EnhancedGeometricSearchResponse {
    type Item = EnhancedGeometricSearchResponse;

    /// Format the EnhancedGeometricSearchResponse according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
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
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = Self::csv_header();
                        // Add metadata columns - we need to get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets
                        for match_result in &self.matches {
                            for key in match_result.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(EnhancedGeometricSearchResponse::csv_header())?;
                    }
                }

                for match_result in &self.matches {
                    if options.with_metadata {
                        // Include metadata values in the output
                        let mut base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];

                        // Get ALL unique metadata keys that were used in the header
                        // (collected from reference asset and ALL match assets)
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets (same as header generation)
                        for match_result_iter in &self.matches {
                            for key in match_result_iter.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Add metadata values for each key that was included in the header
                        for key in &sorted_keys {
                            // Add reference asset metadata value (same for all records)
                            let ref_value = self
                                .reference_asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(ref_value);

                            // Add candidate asset metadata value (specific to this match)
                            let cand_value = match_result
                                .asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(cand_value);
                        }

                        wtr.serialize(base_values.as_slice())?;
                    } else {
                        wtr.serialize((
                            &self.reference_asset.path,
                            &match_result.path(),
                            &match_result.score(),
                            &self.reference_asset.uuid.to_string(),
                            &match_result.asset_uuid().to_string(),
                            &match_result.comparison_url.clone().unwrap_or_default(),
                        ))?;
                    }
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl EnhancedGeometricSearchResponse {
    /// Format the EnhancedGeometricSearchResponse with consideration for metadata flag
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    #[allow(clippy::result_large_err)]
    pub fn format_with_metadata_option(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = Self::csv_header();
                        // Add metadata columns - we need to get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets
                        for match_result in &self.matches {
                            for key in match_result.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(EnhancedGeometricSearchResponse::csv_header())?;
                    }
                }

                for match_result in &self.matches {
                    if options.with_metadata {
                        // Include metadata values in the output
                        let mut base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];

                        // Get ALL unique metadata keys that were used in the header
                        // (collected from reference asset and ALL match assets)
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets (same as header generation)
                        for match_result_iter in &self.matches {
                            for key in match_result_iter.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Add metadata values for each key that was included in the header
                        for key in &sorted_keys {
                            // Add reference asset metadata value (same for all records)
                            let ref_value = self
                                .reference_asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(ref_value);

                            // Add candidate asset metadata value (specific to this match)
                            let cand_value = match_result
                                .asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(cand_value);
                        }

                        wtr.serialize(base_values.as_slice())?;
                    } else {
                        wtr.serialize((
                            &self.reference_asset.path,
                            &match_result.path(),
                            &match_result.score(),
                            &self.reference_asset.uuid.to_string(),
                            &match_result.asset_uuid().to_string(),
                            &match_result.comparison_url.clone().unwrap_or_default(),
                        ))?;
                    }
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

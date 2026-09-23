//! Text search results.

use super::*;

/// Represents a match result from the text search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMatch {
    /// The matching asset details
    pub asset: AssetResponse,
    /// The relevance score of the match (may not be present in all API responses)
    #[serde(
        rename = "relevanceScore",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub relevance_score: Option<f64>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl TextMatch {
    /// Get the asset path
    pub fn path(&self) -> String {
        self.asset.path.clone()
    }

    /// Get the asset UUID
    pub fn asset_uuid(&self) -> Uuid {
        self.asset.uuid
    }

    /// Get the relevance score
    pub fn score(&self) -> f64 {
        self.relevance_score.unwrap_or(0.0)
    }
}

impl CsvRecordProducer for TextMatch {
    /// Get the CSV header row for TextMatch records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_NAME".to_string(),
            "ASSET_PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "RELEVANCE_SCORE".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_URL".to_string(),
        ]
    }

    /// Convert the TextMatch to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        // Extract the asset name from the path (last segment after the final '/')
        let asset_name = self
            .asset
            .path
            .split('/')
            .next_back()
            .unwrap_or(&self.asset.path);

        // Build the asset URL using the template: {baseUrl}/tenants/{tenantId}/asset/{assetUuid}
        let asset_url = if let Some(asset_url) = self
            .comparison_url
            .as_ref()
            .filter(|url| !url.contains("/compare?"))
        {
            // Already an asset URL (text match stores one): use it as-is. Splitting
            // on "/compare?" below found nothing and appended "/asset/<uuid>" to a
            // URL that already ended that way.
            asset_url.clone()
        } else if let Some(ref comparison_url) = self.comparison_url {
            // Extract base URL from the comparison URL and build the asset URL
            let url_parts: Vec<&str> = comparison_url.split("/compare?").collect();
            if let Some(base_url) = url_parts.first() {
                // Check if the base URL already contains the tenant path to avoid duplication
                if base_url.contains("/tenants/") {
                    // If the base URL already has tenant info, just replace compare with asset
                    format!("{}/asset/{}", base_url, self.asset.uuid)
                } else {
                    // If the base URL doesn't have tenant info, add it
                    format!(
                        "{}/tenants/{}/asset/{}",
                        base_url, self.asset.tenant_id, self.asset.uuid
                    )
                }
            } else {
                comparison_url
                    .replace("compare?", "asset/")
                    .replace("&", "")
            }
        } else {
            // If no comparison URL is available, construct a basic URL
            format!(
                "https://app.physna.com/tenants/{}/asset/{}",
                self.asset.tenant_id, self.asset.uuid
            )
        };

        vec![vec![
            asset_name.to_string(),                             // ASSET_NAME
            self.asset.path.clone(),                            // ASSET_PATH
            self.asset.asset_type.clone(),                      // TYPE
            self.asset.state.clone(),                           // STATE
            self.asset.is_assembly.to_string(),                 // IS_ASSEMBLY
            format!("{}", self.relevance_score.unwrap_or(0.0)), // RELEVANCE_SCORE
            self.asset.uuid.to_string(),                        // ASSET_UUID
            asset_url,                                          // ASSET_URL
        ]]
    }
}

/// Response structure for text search operations
///
/// This structure holds the results of a text search operation, including
/// the list of matching assets and pagination/filter information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSearchResponse {
    /// The list of matching assets
    pub matches: Vec<TextMatch>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
    /// Filter information
    #[serde(rename = "filterData")]
    pub filter_data: Option<FilterData>,
}

/// Enhanced response structure for text search that includes search query information
///
/// This structure extends the basic TextSearchResponse by including information about
/// the search query that was performed, making it easier to understand
/// the context of the matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedTextSearchResponse {
    /// The search query that was performed
    #[serde(rename = "searchQuery")]
    pub search_query: String,
    /// The list of matching assets
    pub matches: Vec<TextMatch>,
}

impl CsvRecordProducer for EnhancedTextSearchResponse {
    /// Get the CSV header row for EnhancedTextSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_NAME".to_string(),
            "ASSET_PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "RELEVANCE_SCORE".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_URL".to_string(),
        ]
    }

    /// Convert the EnhancedTextSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .flat_map(|m| m.as_csv_records().into_iter().collect::<Vec<Vec<String>>>())
            .collect()
    }
}

/// Represents a pair of assets that matched in a text search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMatchPair {
    /// The reference asset (the one being searched against)
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The candidate asset (the one that matched)
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The relevance score
    #[serde(rename = "relevanceScore")]
    pub relevance_score: f64,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl CsvRecordProducer for TextMatchPair {
    /// Get the CSV header row for TextMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_NAME".to_string(),
            "ASSET_PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "RELEVANCE_SCORE".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_URL".to_string(),
        ]
    }

    /// Convert the TextMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        // Extract the asset name from the path (last segment after the final '/')
        let asset_name = self
            .reference_asset
            .path
            .split('/')
            .next_back()
            .unwrap_or(&self.reference_asset.path);

        // Build the asset URL using the template: {baseUrl}/tenants/{tenantId}/asset/{assetUuid}
        let asset_url = if let Some(ref comparison_url) = self.comparison_url {
            // Extract base URL from the comparison URL and build the asset URL
            let url_parts: Vec<&str> = comparison_url.split("/compare?").collect();
            if let Some(base_url) = url_parts.first() {
                // Check if the base URL already contains the tenant path to avoid duplication
                if base_url.contains("/tenants/") {
                    // If the base URL already has tenant info, just replace compare with asset
                    format!("{}/asset/{}", base_url, self.reference_asset.uuid)
                } else {
                    // If the base URL doesn't have tenant info, add it
                    format!(
                        "{}/tenants/{}/asset/{}",
                        base_url, self.reference_asset.tenant_id, self.reference_asset.uuid
                    )
                }
            } else {
                comparison_url
                    .replace("compare?", "asset/")
                    .replace("&", "")
            }
        } else {
            // If no comparison URL is available, construct a basic URL
            format!(
                "https://app.physna.com/tenants/{}/asset/{}",
                self.reference_asset.tenant_id, self.reference_asset.uuid
            )
        };

        vec![vec![
            asset_name.to_string(),                       // ASSET_NAME
            self.reference_asset.path.clone(),            // ASSET_PATH
            self.reference_asset.asset_type.clone(),      // TYPE
            self.reference_asset.state.clone(),           // STATE
            self.reference_asset.is_assembly.to_string(), // IS_ASSEMBLY
            format!("{}", self.relevance_score),          // RELEVANCE_SCORE
            self.reference_asset.uuid.to_string(),        // ASSET_UUID
            asset_url,                                    // ASSET_URL
        ]]
    }
}

impl From<&TextMatch> for TextMatchPair {
    fn from(text_match: &TextMatch) -> Self {
        TextMatchPair {
            reference_asset: text_match.asset.clone(), // For text search, we'll treat the matched asset as both ref and candidate
            candidate_asset: text_match.asset.clone(),
            relevance_score: text_match.relevance_score.unwrap_or(0.0),
            comparison_url: text_match.comparison_url.clone(),
        }
    }
}

impl OutputFormatter for TextMatchPair {
    type Item = TextMatchPair;

    /// Format the TextMatchPair according to the specified output format
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = TextMatchPair::csv_header();

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
                            base_headers.push(format!("CAN_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(TextMatchPair::csv_header())?;
                    }
                }

                // Rows come from the same producer as the header. They used to be
                // written by hand with six fields under an eight-column header,
                // which the CSV writer rejects outright.
                let base_row = self.as_csv_records().into_iter().next().unwrap_or_default();
                if options.with_metadata {
                    // Include metadata values in the output
                    let mut base_values = base_row;

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
                    wtr.serialize(base_row.as_slice())?;
                }

                let data = wtr.into_inner()?;
                crate::format::csv_text(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

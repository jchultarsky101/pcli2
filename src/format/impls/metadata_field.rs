//! Formatting implementations for the tenant metadata-field registry.
//!
//! `tenant metadata list` reports every metadata field registered in the tenant
//! together with its data type. The CSV layout deliberately mirrors the classic
//! `asset metadata create-batch` input (`ASSET_PATH,NAME,VALUE,TYPE`) so the
//! output can be turned into a batch-upload file: the NAME and TYPE columns are
//! filled from the registry and the ASSET_PATH and VALUE columns are left empty
//! for the user to populate.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{MetadataField, MetadataFieldListResponse};
use csv::Writer;

impl CsvRecordProducer for MetadataFieldListResponse {
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_PATH".to_string(),
            "NAME".to_string(),
            "VALUE".to_string(),
            "TYPE".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.metadata_fields
            .iter()
            .map(|field| {
                vec![
                    // ASSET_PATH and VALUE are left blank so the output is a
                    // ready-to-fill create-batch template.
                    String::new(),
                    field.name.clone(),
                    String::new(),
                    field.field_type.clone(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for MetadataFieldListResponse {
    type Item = MetadataFieldListResponse;

    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                // Emit a flat array of {name, type}, sorted by name, rather than
                // the paginated envelope the API returns.
                let mut fields: Vec<MetadataField> = self.metadata_fields.clone();
                fields.sort_by(|a, b| a.name.cmp(&b.name));
                let json = if options.pretty {
                    serde_json::to_string_pretty(&fields)
                } else {
                    serde_json::to_string(&fields)
                };
                json.map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })
            }
            OutputFormat::Csv(options) => {
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.write_record(Self::csv_header())?;
                }

                // Sort by the NAME column (index 1) for stable, readable output.
                let mut records = self.as_csv_records();
                records.sort_by(|a, b| a[1].cmp(&b[1]));

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
                let mut fields: Vec<&MetadataField> = self.metadata_fields.iter().collect();
                fields.sort_by(|a, b| a.name.cmp(&b.name));
                let mut output = String::new();
                for field in fields {
                    output.push_str(&format!("{} ({})\n", field.name, field.field_type));
                }
                Ok(output)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::OutputFormatOptions;

    fn sample() -> MetadataFieldListResponse {
        MetadataFieldListResponse {
            metadata_fields: vec![
                MetadataField {
                    name: "Unit Price ($)".to_string(),
                    field_type: "number".to_string(),
                },
                MetadataField {
                    name: "Description".to_string(),
                    field_type: "text".to_string(),
                },
                MetadataField {
                    name: "Supplier Link".to_string(),
                    field_type: "url".to_string(),
                },
            ],
            page_data: None,
        }
    }

    #[test]
    fn csv_matches_create_batch_header_and_is_sorted() {
        let options = OutputFormatOptions {
            with_metadata: false,
            with_headers: true,
            pretty: false,
        };
        let out = sample().format(OutputFormat::Csv(options)).unwrap();
        let expected = "ASSET_PATH,NAME,VALUE,TYPE\n\
                        ,Description,,text\n\
                        ,Supplier Link,,url\n\
                        ,Unit Price ($),,number\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn csv_can_omit_headers() {
        let options = OutputFormatOptions {
            with_metadata: false,
            with_headers: false,
            pretty: false,
        };
        let out = sample().format(OutputFormat::Csv(options)).unwrap();
        assert!(!out.contains("ASSET_PATH"));
        assert!(out.starts_with(",Description,,text"));
    }

    #[test]
    fn json_is_flat_sorted_array_of_name_and_type() {
        let options = OutputFormatOptions::default();
        let out = sample().format(OutputFormat::Json(options)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["name"], "Description");
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[2]["name"], "Unit Price ($)");
        assert_eq!(arr[2]["type"], "number");
    }
}

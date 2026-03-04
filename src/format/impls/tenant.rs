//! Formatting implementations for Tenant and TenantList.
//!
//! This module provides CSV, JSON, and Tree formatting for tenant-related data structures.

use crate::format::{
    CsvRecordProducer, Formattable, FormattingError, OutputFormat, OutputFormatter,
};
use crate::model::{Tenant, TenantList};
use csv::Writer;

impl Formattable for Tenant {
    fn format(&self, f: &OutputFormat) -> Result<String, FormattingError> {
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
                // For CSV format, output header with tenant name, UUID, and description columns only if with_headers is true
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.serialize(("TENANT_NAME", "TENANT_UUID", "TENANT_DESCRIPTION"))?;
                }

                wtr.serialize((&self.name, &self.uuid.to_string(), &self.description))?;

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(_) => {
                // For tree format, include name, UUID, and description
                Ok(format!(
                    "{} ({}) - {}",
                    self.name, self.uuid, self.description
                ))
            }
        }
    }
}

impl CsvRecordProducer for TenantList {
    /// Get the CSV header row for TenantList records
    fn csv_header() -> Vec<String> {
        vec![
            "TENANT_NAME".to_string(),
            "TENANT_UUID".to_string(),
            "TENANT_DESCRIPTION".to_string(),
        ]
    }

    /// Convert the TenantList to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.iter()
            .map(|tenant| {
                vec![
                    tenant.name.clone(),
                    tenant.uuid.to_string(),
                    tenant.description.clone(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for TenantList {
    type Item = TenantList;

    /// Format the TenantList according to the specified output format
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
                let mut tenants: Vec<Tenant> = self.tenants();
                tenants.sort_by_key(|a| a.name.clone());
                let json = if options.pretty {
                    serde_json::to_string_pretty(&tenants)
                } else {
                    serde_json::to_string(&tenants)
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

                // Sort records by tenant name
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
                // For tree format, include name, UUID, and description
                let mut output = String::new();
                for tenant in self.iter() {
                    output.push_str(&format!(
                        "{} ({}) - {}\n",
                        tenant.name, tenant.uuid, tenant.description
                    ));
                }
                Ok(output)
            }
        }
    }
}

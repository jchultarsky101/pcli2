//! Formatting for `tenant metadata coverage`: one record, JSON or CSV.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::MetadataCoverage;

impl CsvRecordProducer for MetadataCoverage {
    fn csv_header() -> Vec<String> {
        vec![
            "COVERED_ASSETS".to_string(),
            "TOTAL_ASSETS".to_string(),
            "COVERAGE_PERCENT".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.covered_assets.to_string(),
            self.total_assets.to_string(),
            format!("{:.1}", self.coverage_percent),
        ]]
    }
}

impl OutputFormatter for MetadataCoverage {
    type Item = MetadataCoverage;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::OutputFormatOptions;
    use crate::model::MetadataCoverageResponse;

    #[test]
    fn the_percentage_is_computed_and_an_empty_tenant_is_zero() {
        let coverage: MetadataCoverage = MetadataCoverageResponse {
            covered_assets: 16143.0,
            total_assets: 23580.0,
        }
        .into();
        assert_eq!(coverage.covered_assets, 16143);
        assert_eq!(coverage.total_assets, 23580);
        assert!((coverage.coverage_percent - 68.4605).abs() < 0.001);

        let empty: MetadataCoverage = MetadataCoverageResponse {
            covered_assets: 0.0,
            total_assets: 0.0,
        }
        .into();
        assert_eq!(empty.coverage_percent, 0.0);
    }

    #[test]
    fn csv_has_one_row_with_one_decimal() {
        let coverage: MetadataCoverage = MetadataCoverageResponse {
            covered_assets: 2.0,
            total_assets: 3.0,
        }
        .into();
        let csv = coverage
            .format(OutputFormat::Csv(OutputFormatOptions {
                with_metadata: false,
                with_headers: true,
                pretty: false,
            }))
            .unwrap();
        assert_eq!(
            csv,
            "COVERED_ASSETS,TOTAL_ASSETS,COVERAGE_PERCENT\n2,3,66.7"
        );
    }

    #[test]
    fn json_uses_the_api_names() {
        let coverage: MetadataCoverage = MetadataCoverageResponse {
            covered_assets: 1.0,
            total_assets: 4.0,
        }
        .into();
        let json = coverage
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert_eq!(
            json,
            r#"{"coveredAssets":1,"totalAssets":4,"coveragePercent":25.0}"#
        );
    }
}

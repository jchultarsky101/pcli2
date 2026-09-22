//! Formatting for failure diagnostics (`asset diagnose`) and the recent
//! failures listing (`tenant failures`).
//!
//! Both are JSON or CSV. Optional fields the server did not send are empty
//! CSV cells and absent JSON keys.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{AssetFailureDiagnostics, RecentFailuresList};

impl CsvRecordProducer for AssetFailureDiagnostics {
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_PATH".to_string(),
            "ASSET_STATE".to_string(),
            "STATUS".to_string(),
            "KIND".to_string(),
            "SUMMARY".to_string(),
            "TRACE_ID".to_string(),
            "OCCURRED_AT".to_string(),
            "ASSET_UUID".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.asset_path.clone(),
            self.asset_state.clone().unwrap_or_default(),
            self.status.as_str().to_string(),
            self.kind
                .map(|k| k.as_str().to_string())
                .unwrap_or_default(),
            self.summary.clone().unwrap_or_default(),
            self.trace_id.clone().unwrap_or_default(),
            self.occurred_at.clone().unwrap_or_default(),
            self.asset_uuid.to_string(),
        ]]
    }
}

impl OutputFormatter for AssetFailureDiagnostics {
    type Item = AssetFailureDiagnostics;

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

impl CsvRecordProducer for RecentFailuresList {
    fn csv_header() -> Vec<String> {
        vec![
            "KIND".to_string(),
            "NAME".to_string(),
            "ID".to_string(),
            "FAILED_AT".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.failures
            .iter()
            .map(|f| {
                vec![
                    f.kind.as_str().to_string(),
                    f.name.clone(),
                    f.id.to_string(),
                    f.failed_at.clone(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for RecentFailuresList {
    type Item = RecentFailuresList;

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
    use crate::model::{
        FailureCountsByKind, FailureDiagnosticsStatus, FailureKind, FailureSource, RecentFailure,
    };
    use uuid::Uuid;

    fn csv(with_headers: bool) -> OutputFormat {
        OutputFormat::Csv(OutputFormatOptions {
            with_metadata: false,
            with_headers,
            pretty: false,
        })
    }

    fn diagnostics() -> AssetFailureDiagnostics {
        AssetFailureDiagnostics {
            asset_path: "/Home/parts/bracket.stp".to_string(),
            asset_uuid: Uuid::nil(),
            asset_state: Some("failed".to_string()),
            status: FailureDiagnosticsStatus::Found,
            kind: Some(FailureKind::User),
            summary: Some("The file format or version is not supported.".to_string()),
            trace_id: Some("6f2b1c40-9f3a-4a1e-9a1b-2c7d4e5f6a7b".to_string()),
            occurred_at: Some("2026-08-26T19:03:16.000Z".to_string()),
        }
    }

    #[test]
    fn diagnostics_csv_has_one_row_with_the_api_spellings() {
        let out = diagnostics().format(csv(true)).unwrap();
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "ASSET_PATH,ASSET_STATE,STATUS,KIND,SUMMARY,TRACE_ID,OCCURRED_AT,ASSET_UUID"
        );
        assert_eq!(
            lines.next().unwrap(),
            "/Home/parts/bracket.stp,failed,found,user,The file format or version is not supported.,6f2b1c40-9f3a-4a1e-9a1b-2c7d4e5f6a7b,2026-08-26T19:03:16.000Z,00000000-0000-0000-0000-000000000000"
        );
        assert!(lines.next().is_none());
    }

    #[test]
    fn diagnostics_csv_leaves_unsent_fields_empty() {
        let mut d = diagnostics();
        d.status = FailureDiagnosticsStatus::NotFound;
        d.kind = None;
        d.summary = None;
        d.trace_id = None;
        d.occurred_at = None;
        let out = d.format(csv(false)).unwrap();
        assert_eq!(
            out,
            "/Home/parts/bracket.stp,failed,not-found,,,,,00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn diagnostics_json_uses_the_api_names_and_omits_unsent_fields() {
        let full = diagnostics()
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert!(full.contains("\"assetPath\":\"/Home/parts/bracket.stp\""));
        assert!(full.contains("\"assetState\":\"failed\""));
        assert!(full.contains("\"status\":\"found\""));
        assert!(full.contains("\"kind\":\"user\""));
        assert!(full.contains("\"traceId\":\"6f2b1c40"));
        assert!(full.contains("\"occurredAt\":\"2026-08-26T19:03:16.000Z\""));

        let mut d = diagnostics();
        d.kind = None;
        d.trace_id = None;
        let sparse = d
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert!(!sparse.contains("\"kind\""));
        assert!(!sparse.contains("\"traceId\""));
    }

    #[test]
    fn diagnostics_rejects_tree() {
        let err = diagnostics()
            .format(OutputFormat::Tree(OutputFormatOptions::default()))
            .unwrap_err();
        assert!(matches!(err, FormattingError::UnsupportedOutputFormat(_)));
    }

    fn failures() -> RecentFailuresList {
        RecentFailuresList {
            failures: vec![
                RecentFailure {
                    kind: FailureSource::Asset,
                    id: Uuid::nil(),
                    name: "bracket.stp".to_string(),
                    failed_at: "2026-09-20T10:00:00.000Z".to_string(),
                },
                RecentFailure {
                    kind: FailureSource::PartFinderReport,
                    id: Uuid::nil(),
                    name: "weekly".to_string(),
                    failed_at: "2026-09-19T10:00:00.000Z".to_string(),
                },
            ],
            counts_by_kind: FailureCountsByKind {
                asset: 7.0,
                report: 0.0,
                part_finder_report: 1.0,
            },
        }
    }

    #[test]
    fn failures_csv_has_one_row_per_failure() {
        let out = failures().format(csv(true)).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "KIND,NAME,ID,FAILED_AT");
        assert_eq!(
            lines[1],
            "asset,bracket.stp,00000000-0000-0000-0000-000000000000,2026-09-20T10:00:00.000Z"
        );
        assert_eq!(
            lines[2],
            "part-finder-report,weekly,00000000-0000-0000-0000-000000000000,2026-09-19T10:00:00.000Z"
        );
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn failures_json_keeps_the_tenant_wide_totals() {
        let out = failures()
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert!(out.contains(
            "\"countsByKind\":{\"asset\":7.0,\"report\":0.0,\"part-finder-report\":1.0}"
        ));
        assert!(out.contains("\"kind\":\"part-finder-report\""));
        assert!(out.contains("\"failedAt\":\"2026-09-20T10:00:00.000Z\""));
    }
}

//! Formatting for the `report` command group: report records, the report
//! listing, and report failure diagnostics. JSON or CSV.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{Report, ReportFailureDiagnostics, ReportList};

/// A number the API types as double but which is usually whole (progress,
/// group counts, thresholds): printed without a trailing `.0`.
fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

fn report_header() -> Vec<String> {
    [
        "ID",
        "NAME",
        "TYPE",
        "STATUS",
        "PROGRESS",
        "GROUPS",
        "MIN_THRESHOLD",
        "MAX_THRESHOLD",
        "CREATED_AT",
        "UPDATED_AT",
        "CREATOR",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn report_record(report: &Report) -> Vec<String> {
    vec![
        report.id.clone(),
        report.name.clone().unwrap_or_default(),
        report.report_type.as_str().to_string(),
        report.status.as_str().to_string(),
        number(report.progress),
        number(report.group_count),
        number(report.min_threshold),
        number(report.max_threshold),
        report.created_at.clone(),
        report.updated_at.clone(),
        report
            .creator
            .as_ref()
            .map(|c| c.email.clone())
            .unwrap_or_default(),
    ]
}

fn json<T: serde::Serialize>(value: &T, pretty: bool) -> Result<String, FormattingError> {
    if pretty {
        Ok(serde_json::to_string_pretty(value)?)
    } else {
        Ok(serde_json::to_string(value)?)
    }
}

impl CsvRecordProducer for Report {
    fn csv_header() -> Vec<String> {
        report_header()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![report_record(self)]
    }
}

impl OutputFormatter for Report {
    type Item = Report;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => json(self, options.pretty),
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CsvRecordProducer for ReportList {
    fn csv_header() -> Vec<String> {
        report_header()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.reports.iter().map(report_record).collect()
    }
}

impl OutputFormatter for ReportList {
    type Item = ReportList;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            // A flat array of reports, like the other listings.
            OutputFormat::Json(options) => json(&self.reports, options.pretty),
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CsvRecordProducer for ReportFailureDiagnostics {
    fn csv_header() -> Vec<String> {
        [
            "REPORT_ID",
            "REPORT_NAME",
            "REPORT_STATUS",
            "STATUS",
            "KIND",
            "SUMMARY",
            "TRACE_ID",
            "OCCURRED_AT",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.report_id.clone(),
            self.report_name.clone().unwrap_or_default(),
            self.report_status.as_str().to_string(),
            self.status.as_str().to_string(),
            self.kind
                .map(|k| k.as_str().to_string())
                .unwrap_or_default(),
            self.summary.clone().unwrap_or_default(),
            self.trace_id.clone().unwrap_or_default(),
            self.occurred_at.clone().unwrap_or_default(),
        ]]
    }
}

impl OutputFormatter for ReportFailureDiagnostics {
    type Item = ReportFailureDiagnostics;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => json(self, options.pretty),
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::OutputFormatOptions;
    use crate::model::{Creator, JobStatus, ReportType};
    use uuid::Uuid;

    fn sample() -> Report {
        Report {
            id: "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1".to_string(),
            tenant_id: "68555ebf-f09c-4861-96b1-692d2ec10de7".to_string(),
            status: JobStatus::Completed,
            progress: 100.0,
            report_type: ReportType::Custom,
            min_threshold: 80.0,
            max_threshold: 99.5,
            created_at: "2026-09-21T18:33:10.382Z".to_string(),
            updated_at: "2026-09-21T18:33:11.254Z".to_string(),
            group_count: 1.0,
            name: Some("Custom 9/21/2026".to_string()),
            creator: Some(Creator {
                id: Uuid::nil(),
                email: "someone@example.com".to_string(),
            }),
            extensions: None,
            folder_ids: Some(vec!["3c91b897-0c8b-40f5-946d-6c3a867e7869".to_string()]),
            folder_paths: None,
            excluded_folder_ids: None,
            exclude_assemblies: Some(false),
            exclude_exact_duplicates: Some(false),
            include_home_folder_assets: Some(false),
            metadata_filters: None,
            search_query: None,
            model_index_config_id: None,
        }
    }

    fn csv(with_headers: bool) -> OutputFormat {
        OutputFormat::Csv(OutputFormatOptions {
            with_metadata: false,
            with_headers,
            pretty: false,
        })
    }

    #[test]
    fn a_report_row_prints_whole_numbers_without_decimals() {
        let out = sample().format(csv(true)).unwrap();
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "ID,NAME,TYPE,STATUS,PROGRESS,GROUPS,MIN_THRESHOLD,MAX_THRESHOLD,CREATED_AT,UPDATED_AT,CREATOR"
        );
        assert_eq!(
            lines.next().unwrap(),
            "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1,Custom 9/21/2026,CUSTOM,COMPLETED,100,1,80,99.5,2026-09-21T18:33:10.382Z,2026-09-21T18:33:11.254Z,someone@example.com"
        );
    }

    #[test]
    fn the_listing_is_a_json_array_and_omits_unsent_fields() {
        let list = ReportList {
            reports: vec![sample()],
        };
        let out = list
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert!(out.starts_with("[{\"id\":\"f046e5b8"));
        assert!(out.contains("\"reportType\":\"CUSTOM\""));
        assert!(out.contains("\"status\":\"COMPLETED\""));
        assert!(!out.contains("searchQuery"));
        assert!(!out.contains("metadataFilters"));
    }

    #[test]
    fn an_empty_listing_is_an_empty_array_and_only_a_header() {
        let list = ReportList { reports: vec![] };
        assert_eq!(
            list.format(OutputFormat::Json(OutputFormatOptions::default()))
                .unwrap(),
            "[]"
        );
        assert_eq!(list.format(csv(false)).unwrap(), "");
        assert_eq!(
            list.format(csv(true)).unwrap(),
            "ID,NAME,TYPE,STATUS,PROGRESS,GROUPS,MIN_THRESHOLD,MAX_THRESHOLD,CREATED_AT,UPDATED_AT,CREATOR"
        );
    }

    #[test]
    fn diagnostics_carry_the_report_identity() {
        let d = ReportFailureDiagnostics::new(
            &sample(),
            crate::model::FailureDiagnostics {
                status: crate::model::FailureDiagnosticsStatus::NotFound,
                kind: None,
                summary: None,
                trace_id: None,
                occurred_at: None,
            },
        );
        assert_eq!(
            d.format(csv(false)).unwrap(),
            "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1,Custom 9/21/2026,COMPLETED,not-found,,,,"
        );
        let json = d
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert!(json.contains("\"reportId\":\"f046e5b8"));
        assert!(json.contains("\"reportStatus\":\"COMPLETED\""));
        assert!(!json.contains("\"kind\""));
    }
}

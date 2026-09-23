//! Formatting for `tenant usage`.
//!
//! The summary is one `CATEGORY,NAME,COUNT` row per number rather than one
//! wide row: the searches, reports, features and asset types the server
//! reports can grow, and a new one then adds a row instead of shifting the
//! columns a script reads.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{DailyUsage, TenantUsage};

fn json<T: serde::Serialize>(value: &T, pretty: bool) -> Result<String, FormattingError> {
    if pretty {
        Ok(serde_json::to_string_pretty(value)?)
    } else {
        Ok(serde_json::to_string(value)?)
    }
}

impl CsvRecordProducer for TenantUsage {
    fn csv_header() -> Vec<String> {
        vec![
            "CATEGORY".to_string(),
            "NAME".to_string(),
            "COUNT".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let row = |category: &str, name: &str, count: u64| {
            vec![category.to_string(), name.to_string(), count.to_string()]
        };
        let a = &self.activity;
        let mut rows = vec![
            row("activity", "searches", a.searches),
            row("activity", "compares", a.compares),
            row("activity", "downloads", a.downloads),
            row("activity", "uploads", a.uploads),
            row("activity", "reports", a.reports),
            row("activity", "activeUsers", a.active_users),
            row("activity", "internalActiveUsers", a.internal_active_users),
        ];
        for (category, counts) in [
            ("searches", &a.searches_by_type),
            ("reports", &a.reports_by_type),
            ("features", &a.feature_usage),
            ("assets", &self.asset_types),
        ] {
            rows.extend(
                counts
                    .0
                    .iter()
                    .map(|(name, count)| row(category, name, *count)),
            );
        }
        rows
    }
}

impl OutputFormatter for TenantUsage {
    type Item = TenantUsage;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => json(self, options.pretty),
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CsvRecordProducer for DailyUsage {
    fn csv_header() -> Vec<String> {
        [
            "DATE",
            "SEARCHES",
            "COMPARES",
            "DOWNLOADS",
            "UPLOADS",
            "REPORTS",
            "ACTIVE_USERS",
        ]
        .map(String::from)
        .to_vec()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.0
            .iter()
            .map(|day| {
                vec![
                    day.date.clone(),
                    day.searches.to_string(),
                    day.compares.to_string(),
                    day.downloads.to_string(),
                    day.uploads.to_string(),
                    day.reports.to_string(),
                    day.active_users.to_string(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for DailyUsage {
    type Item = DailyUsage;

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
    use crate::model::{ActivityMetrics, Counts};

    /// Trimmed from a live demo-1 response (2026-09-23).
    const METRICS: &str = r#"{"searches":289,"compares":53,"downloads":4,"uploads":0,"reports":2,
        "activeUsers":11,"internalActiveUsers":11,
        "searchesByType":{"text":49,"visual":98},
        "reportsByType":{"DUPLICATION":0,"CUSTOM":2},
        "featureUsage":{"folder_browse":357},
        "daily":[{"date":"2026-09-22","searches":81,"compares":13,"downloads":3,"uploads":0,"reports":0,"activeUsers":7},
                 {"date":"2026-09-23","searches":0,"compares":0,"downloads":0,"uploads":0,"reports":0,"activeUsers":2}]}"#;

    fn usage() -> TenantUsage {
        TenantUsage {
            from: "2026-09-22".to_string(),
            to: "2026-09-23".to_string(),
            activity: serde_json::from_str::<ActivityMetrics>(METRICS).unwrap(),
            asset_types: serde_json::from_str::<Counts>(r#"{"model":21091,"scan":1}"#).unwrap(),
        }
    }

    fn csv(headers: bool) -> OutputFormat {
        OutputFormat::Csv(OutputFormatOptions {
            with_metadata: false,
            with_headers: headers,
            pretty: false,
        })
    }

    #[test]
    fn summary_csv_is_one_row_per_count() {
        assert_eq!(
            usage().format(csv(true)).unwrap(),
            "CATEGORY,NAME,COUNT\n\
             activity,searches,289\n\
             activity,compares,53\n\
             activity,downloads,4\n\
             activity,uploads,0\n\
             activity,reports,2\n\
             activity,activeUsers,11\n\
             activity,internalActiveUsers,11\n\
             searches,text,49\n\
             searches,visual,98\n\
             reports,DUPLICATION,0\n\
             reports,CUSTOM,2\n\
             features,folder_browse,357\n\
             assets,model,21091\n\
             assets,scan,1"
        );
    }

    #[test]
    fn summary_json_is_the_api_response_plus_period_and_asset_types() {
        let json: serde_json::Value = serde_json::from_str(
            &usage()
                .format(OutputFormat::Json(OutputFormatOptions::default()))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["from"], "2026-09-22");
        assert_eq!(json["to"], "2026-09-23");
        assert_eq!(json["activeUsers"], 11);
        assert_eq!(json["searchesByType"]["visual"], 98);
        assert_eq!(json["daily"][0]["activeUsers"], 7);
        assert_eq!(json["assetTypes"]["model"], 21091);
    }

    #[test]
    fn daily_csv_is_one_row_per_day() {
        let daily = DailyUsage(usage().activity.daily);
        assert_eq!(
            daily.format(csv(true)).unwrap(),
            "DATE,SEARCHES,COMPARES,DOWNLOADS,UPLOADS,REPORTS,ACTIVE_USERS\n\
             2026-09-22,81,13,3,0,0,7\n\
             2026-09-23,0,0,0,0,0,2"
        );
        assert_eq!(
            daily
                .format(OutputFormat::Json(OutputFormatOptions::default()))
                .unwrap(),
            r#"[{"date":"2026-09-22","searches":81,"compares":13,"downloads":3,"uploads":0,"reports":0,"activeUsers":7},{"date":"2026-09-23","searches":0,"compares":0,"downloads":0,"uploads":0,"reports":0,"activeUsers":2}]"#
        );
    }
}

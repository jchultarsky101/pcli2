//! Formatting for the `catalog` command group: the catalog listing and the
//! match reports. JSON or CSV (tables are drawn from the CSV).
//!
//! The match CSV follows `asset geometric-match` and friends column for
//! column, with a CATALOG column after the candidate's path, so a script
//! written for one reads the other with one extra field.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{CatalogList, CatalogMatch, CatalogMatchReport, CatalogSearchKind};

fn json<T: serde::Serialize>(value: &T, pretty: bool) -> Result<String, FormattingError> {
    if pretty {
        Ok(serde_json::to_string_pretty(value)?)
    } else {
        Ok(serde_json::to_string(value)?)
    }
}

fn to_csv(rows: Vec<Vec<String>>) -> Result<String, FormattingError> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    for row in rows {
        writer.write_record(&row)?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?;
    Ok(crate::format::csv_text(bytes)?)
}

fn score(value: Option<f64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

impl CsvRecordProducer for CatalogList {
    fn csv_header() -> Vec<String> {
        ["NAME", "DESCRIPTION", "ID", "ROLE"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.catalogs
            .iter()
            .map(|c| {
                vec![
                    c.name.clone(),
                    c.description.clone(),
                    c.id.to_string(),
                    c.role.clone(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for CatalogList {
    type Item = CatalogList;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            // A flat array, like the other listings.
            OutputFormat::Json(options) => json(&self.catalogs, options.pretty),
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl CatalogMatchReport {
    fn csv_header(&self) -> Vec<String> {
        let mut header = vec!["REFERENCE_ASSET_PATH", "CATALOG", "CANDIDATE_ASSET_PATH"];
        match self.search_type {
            CatalogSearchKind::Geometric => header.push("MATCH_PERCENTAGE"),
            CatalogSearchKind::Part => {
                header.extend(["FORWARD_MATCH_PERCENTAGE", "REVERSE_MATCH_PERCENTAGE"])
            }
            CatalogSearchKind::Visual => {}
        }
        header.extend([
            "REFERENCE_ASSET_UUID",
            "CANDIDATE_ASSET_UUID",
            "COMPARISON_URL",
        ]);
        header.into_iter().map(String::from).collect()
    }

    fn csv_record(&self, m: &CatalogMatch) -> Vec<String> {
        let mut row = vec![
            self.reference_asset.path.clone(),
            self.catalog.name.clone(),
            m.asset.path.clone(),
        ];
        match self.search_type {
            CatalogSearchKind::Geometric => row.push(score(m.match_percentage)),
            CatalogSearchKind::Part => {
                row.push(score(m.forward_match_percentage));
                row.push(score(m.reverse_match_percentage));
            }
            CatalogSearchKind::Visual => {}
        }
        row.extend([
            self.reference_asset.uuid.to_string(),
            m.asset.uuid.to_string(),
            m.comparison_url.clone().unwrap_or_default(),
        ]);
        row
    }
}

impl OutputFormatter for CatalogMatchReport {
    type Item = CatalogMatchReport;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => json(self, options.pretty),
            OutputFormat::Csv(options) => {
                let mut rows = Vec::with_capacity(self.matches.len() + 1);
                if options.with_headers {
                    rows.push(self.csv_header());
                }
                rows.extend(self.matches.iter().map(|m| self.csv_record(m)));
                to_csv(rows)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::OutputFormatOptions;
    use crate::model::{AssetResponse, Catalog};

    fn asset(path: &str) -> AssetResponse {
        serde_json::from_value(serde_json::json!({
            "id": "2f9d0a2e-6a55-4d26-9a0a-1c9fd0e2c4b1",
            "tenantId": "a0d94427-d7eb-4b28-b624-49c68ce83686",
            "path": path,
            "type": "model",
            "createdAt": "2026-01-01T00:00:00.000Z",
            "updatedAt": "2026-01-01T00:00:00.000Z",
            "state": "finished",
            "isAssembly": false,
            "metadata": {}
        }))
        .unwrap()
    }

    fn report(kind: CatalogSearchKind) -> CatalogMatchReport {
        CatalogMatchReport {
            search_type: kind,
            reference_asset: asset("Parts/bracket.stl"),
            catalog: Catalog {
                id: uuid::Uuid::nil(),
                name: "catalog".into(),
                description: "Supplier Catalog".into(),
                role: "search".into(),
            },
            matches: vec![CatalogMatch {
                asset: asset("cad/1432A11.SLDPRT"),
                match_percentage: Some(97.5),
                forward_match_percentage: Some(100.0),
                reverse_match_percentage: Some(80.0),
                comparison_url: Some("https://x/compare".into()),
            }],
        }
    }

    fn csv(kind: CatalogSearchKind) -> Vec<String> {
        let options = OutputFormatOptions {
            with_headers: true,
            ..Default::default()
        };
        report(kind)
            .format(OutputFormat::Csv(options))
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    }

    #[test]
    fn each_search_has_the_score_columns_of_its_asset_command_plus_catalog() {
        assert_eq!(
            csv(CatalogSearchKind::Geometric)[0],
            "REFERENCE_ASSET_PATH,CATALOG,CANDIDATE_ASSET_PATH,MATCH_PERCENTAGE,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID,COMPARISON_URL"
        );
        assert_eq!(
            csv(CatalogSearchKind::Part)[0],
            "REFERENCE_ASSET_PATH,CATALOG,CANDIDATE_ASSET_PATH,FORWARD_MATCH_PERCENTAGE,REVERSE_MATCH_PERCENTAGE,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID,COMPARISON_URL"
        );
        assert_eq!(
            csv(CatalogSearchKind::Visual)[0],
            "REFERENCE_ASSET_PATH,CATALOG,CANDIDATE_ASSET_PATH,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID,COMPARISON_URL"
        );
        let row = &csv(CatalogSearchKind::Geometric)[1];
        assert!(
            row.starts_with("Parts/bracket.stl,catalog,cad/1432A11.SLDPRT,97.5,"),
            "{row}"
        );
        assert!(csv(CatalogSearchKind::Part)[1].contains(",100,80,"));
    }

    #[test]
    fn json_names_the_search_the_catalog_and_the_reference() {
        let text = report(CatalogSearchKind::Visual)
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["searchType"], "visual");
        assert_eq!(value["catalog"]["name"], "catalog");
        assert_eq!(value["referenceAsset"]["path"], "Parts/bracket.stl");
        assert_eq!(value["matches"][0]["comparisonUrl"], "https://x/compare");
    }
}

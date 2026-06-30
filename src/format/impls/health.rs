use std::fmt::Write;

use crate::format::{Formattable, FormattingError, OutputFormat};
use crate::model::AssetHealthReport;

impl Formattable for AssetHealthReport {
    fn format(&self, f: &OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                json.map_err(FormattingError::JsonSerializationError)
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "TOTAL",
                        "FINISHED",
                        "INDEXING",
                        "FAILED",
                        "UNSUPPORTED",
                        "NO-3D-DATA",
                        "MISSING-DEPS",
                        "ERRORS-TOTAL",
                        "ASSEMBLIES",
                        "PARTS",
                        "FILE-TYPES",
                    ))?;
                }

                let file_types_str = {
                    let mut entries: Vec<_> = self.file_types.iter().collect();
                    entries.sort_by(|a, b| b.1.cmp(a.1));
                    entries
                        .iter()
                        .map(|(k, v)| format!("{}:{}", k, v))
                        .collect::<Vec<_>>()
                        .join(";")
                };

                wtr.serialize((
                    self.total,
                    self.finished,
                    self.indexing,
                    self.failed,
                    self.unsupported,
                    self.no_3d_data,
                    self.missing_dependencies,
                    self.error_total(),
                    self.assemblies,
                    self.parts,
                    &file_types_str,
                ))?;

                let data = wtr.into_inner()?;
                let csv_string = String::from_utf8(data)?;
                Ok(csv_string)
            }
            OutputFormat::Tree(_) => {
                let mut out = String::new();
                writeln!(out, "Asset Health Report").unwrap();
                writeln!(out, "  Total assets:          {}", self.total).unwrap();
                writeln!(out, "  Finished:              {}", self.finished).unwrap();
                writeln!(out, "  Indexing:              {}", self.indexing).unwrap();
                writeln!(out, "  Failed:                {}", self.failed).unwrap();
                writeln!(out, "  Unsupported:           {}", self.unsupported).unwrap();
                writeln!(out, "  No 3D data:            {}", self.no_3d_data).unwrap();
                writeln!(
                    out,
                    "  Missing dependencies:  {}",
                    self.missing_dependencies
                )
                .unwrap();
                writeln!(out, "  Errors (total):        {}", self.error_total()).unwrap();
                writeln!(out, "  Assemblies:            {}", self.assemblies).unwrap();
                writeln!(out, "  Parts:                 {}", self.parts).unwrap();

                if !self.file_types.is_empty() {
                    writeln!(out, "  File types:").unwrap();
                    let mut entries: Vec<_> = self.file_types.iter().collect();
                    entries.sort_by(|a, b| b.1.cmp(a.1));
                    for (ft, count) in entries {
                        writeln!(out, "    {:<20} {}", ft, count).unwrap();
                    }
                }

                Ok(out)
            }
        }
    }
}

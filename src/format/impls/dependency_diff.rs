//! Formatting implementation for the dependency diff.
//!
//! Provides JSON, CSV, and Tree output for [`DependencyDiff`]. The tree output
//! is the headline format: it renders the merged diff tree using the `ptree`
//! crate, marking each node with `(=)` (present in both), `(-)` (only in the
//! reference), or `(+)` (only in the candidate), followed by a summary line.

use crate::dependency_diff::{DependencyDiff, DiffNode};
use crate::format::{FormattingError, OutputFormat, OutputFormatter};
use csv::Writer;
use ptree::TreeBuilder;

impl OutputFormatter for DependencyDiff {
    type Item = DependencyDiff;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                let result = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                result.map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })
            }
            OutputFormat::Csv(options) => self.to_csv_string(options.with_headers),
            OutputFormat::Tree(_) => Ok(self.to_tree_string()),
        }
    }
}

impl DependencyDiff {
    /// Render the diff as a `ptree` tree followed by a summary line.
    fn to_tree_string(&self) -> String {
        let root_label = format!(
            "dependency diff: reference `{}` vs candidate `{}`",
            self.reference, self.candidate
        );
        let mut builder = TreeBuilder::new(root_label);
        for node in &self.nodes {
            build_tree_node(&mut builder, node);
        }
        let tree = builder.build();

        let mut output = Vec::new();
        // Writing to an in-memory buffer is infallible in practice.
        ptree::write_tree(&tree, &mut output).unwrap_or_default();
        let mut rendered = String::from_utf8(output).unwrap_or_default();

        rendered.push_str(&format!(
            "\nLegend: (=) in both  (-) only in reference  (+) only in candidate\n\
             Summary: {} common, {} only in reference, {} only in candidate\n",
            self.summary.common, self.summary.only_in_reference, self.summary.only_in_candidate
        ));
        rendered
    }

    /// Render the diff as flat CSV rows (depth-first), one row per node.
    #[allow(clippy::result_large_err)]
    fn to_csv_string(&self, with_headers: bool) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);

        if with_headers {
            wtr.write_record([
                "STATUS",
                "ASSEMBLY_PATH",
                "FILENAME",
                "ASSET_UUID",
                "ASSET_STATE",
            ])?;
        }

        let mut records: Vec<Vec<String>> = Vec::new();
        for node in &self.nodes {
            collect_csv_records(node, "", &mut records);
        }
        for record in records {
            wtr.write_record(&record)?;
        }

        let data = wtr.into_inner()?;
        String::from_utf8(data).map_err(FormattingError::Utf8Error)
    }
}

/// Recursively add a diff node and its children to the tree builder.
fn build_tree_node(builder: &mut TreeBuilder, node: &DiffNode) {
    let label = tree_label(node);
    if node.children.is_empty() {
        builder.add_empty_child(label);
    } else {
        builder.begin_child(label);
        for child in &node.children {
            build_tree_node(builder, child);
        }
        builder.end_child();
    }
}

/// Build the display label for a single diff node.
fn tree_label(node: &DiffNode) -> String {
    let uuid = node.uuid.clone().unwrap_or_else(|| "None".to_string());
    let state = node.state.clone().unwrap_or_else(|| "missing".to_string());
    format!(
        "({}) {} [{}] ({})",
        node.status.marker(),
        node.filename,
        state,
        uuid
    )
}

/// Recursively flatten a diff node into CSV records, tracking the assembly path.
fn collect_csv_records(node: &DiffNode, parent_path: &str, out: &mut Vec<Vec<String>>) {
    let assembly_path = if parent_path.is_empty() {
        node.filename.clone()
    } else {
        format!("{}/{}", parent_path, node.filename)
    };

    out.push(vec![
        node.status.label().to_string(),
        assembly_path.clone(),
        node.filename.clone(),
        node.uuid.clone().unwrap_or_else(|| "None".to_string()),
        node.state.clone().unwrap_or_else(|| "missing".to_string()),
    ]);

    for child in &node.children {
        collect_csv_records(child, &assembly_path, out);
    }
}

#[cfg(test)]
mod tests {
    use crate::dependency_diff::{DependencyDiff, DiffNode, DiffStatus, DiffSummary};
    use crate::format::{OutputFormat, OutputFormatOptions, OutputFormatter};

    /// A small diff with a common parent, a removed child, and an added top-level node.
    fn sample_diff() -> DependencyDiff {
        DependencyDiff {
            reference: "/A.asm".to_string(),
            candidate: "/B.asm".to_string(),
            summary: DiffSummary {
                common: 1,
                only_in_reference: 1,
                only_in_candidate: 1,
            },
            nodes: vec![
                DiffNode {
                    filename: "gearbox.asm".to_string(),
                    status: DiffStatus::Common,
                    uuid: None,
                    state: Some("finished".to_string()),
                    path: Some("/parts/gearbox.asm".to_string()),
                    children: vec![DiffNode {
                        filename: "old.stl".to_string(),
                        status: DiffStatus::OnlyInReference,
                        uuid: None,
                        state: Some("finished".to_string()),
                        path: Some("/parts/old.stl".to_string()),
                        children: vec![],
                    }],
                },
                DiffNode {
                    filename: "new.stl".to_string(),
                    status: DiffStatus::OnlyInCandidate,
                    uuid: None,
                    state: Some("finished".to_string()),
                    path: Some("/parts/new.stl".to_string()),
                    children: vec![],
                },
            ],
        }
    }

    #[test]
    fn tree_format_shows_markers_and_summary() {
        let out = sample_diff()
            .format(OutputFormat::Tree(OutputFormatOptions::default()))
            .unwrap();
        assert!(out.contains("(=) gearbox.asm"));
        assert!(out.contains("(-) old.stl"));
        assert!(out.contains("(+) new.stl"));
        assert!(out.contains("Summary: 1 common, 1 only in reference, 1 only in candidate"));
    }

    #[test]
    fn csv_format_has_header_and_assembly_paths() {
        let options = OutputFormatOptions {
            with_headers: true,
            ..Default::default()
        };
        let out = sample_diff().format(OutputFormat::Csv(options)).unwrap();
        assert!(out.contains("STATUS,ASSEMBLY_PATH,FILENAME,ASSET_UUID,ASSET_STATE"));
        assert!(out.contains("only_in_reference,gearbox.asm/old.stl,old.stl"));
        assert!(out.contains("only_in_candidate,new.stl,new.stl"));
    }

    #[test]
    fn json_format_includes_status_labels() {
        let out = sample_diff()
            .format(OutputFormat::Json(OutputFormatOptions::default()))
            .unwrap();
        assert!(out.contains("\"status\":\"common\""));
        assert!(out.contains("\"status\":\"only_in_reference\""));
        assert!(out.contains("\"status\":\"only_in_candidate\""));
        assert!(out.contains("\"reference\":\"/A.asm\""));
    }
}

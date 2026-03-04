//! Formatting implementations for asset dependencies and assembly trees.
//!
//! This module provides CSV, JSON, and Tree formatting for asset dependency lists
//! and assembly tree structures using the ptree crate for tree visualization.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use crate::model::{AssemblyNode, AssemblyTree, AssetDependencyList};
use csv::Writer;
use ptree::TreeBuilder;
use serde::Serialize;

impl CsvRecordProducer for AssetDependencyList {
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_PATH".to_string(),
            "ASSEMBLY_PATH".to_string(),
            "DEPENDENCY_PATH".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_NAME".to_string(),
            "ASSET_STATE".to_string(),
            "OCCURRENCES".to_string(),
            "HAS_DEPENDENCIES".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.dependencies
            .iter()
            .map(|dep| {
                let (asset_uuid, asset_filename, asset_state) = match &dep.asset {
                    Some(asset) => {
                        let filename = asset
                            .path
                            .split('/')
                            .next_back()
                            .unwrap_or(&asset.path)
                            .to_string();
                        // Normalize state values - convert "missing-dependencies" to "missing" for consistency
                        let normalized_state = if asset.state == "missing-dependencies" {
                            "missing".to_string()
                        } else {
                            asset.state.clone()
                        };
                        (
                            if asset.uuid.is_nil() {
                                "None".to_string()
                            } else {
                                asset.uuid.to_string()
                            },
                            filename,
                            normalized_state,
                        )
                    }
                    None => {
                        // For missing dependencies, use the path as the name and mark as missing
                        let filename = dep
                            .path
                            .split('/')
                            .next_back()
                            .unwrap_or(&dep.path)
                            .to_string();
                        ("N/A".to_string(), filename, "missing".to_string()) // For missing dependencies
                    }
                };

                vec![
                    if self.path == "MULTIPLE_ASSETS" {
                        // For folder dependencies, use the original asset path from the dependency if available
                        dep.original_asset_path.clone().unwrap_or_else(|| {
                            // Fallback to extracting from the dependency's path if original asset path is not set
                            dep.path
                                .split('/')
                                .take(dep.path.matches('/').count())
                                .collect::<Vec<&str>>()
                                .join("/")
                        })
                    } else {
                        // For single asset dependencies, use the list's path as the original asset path
                        self.path.clone()
                    }, // ASSET_PATH (the original asset)
                    if self.path == "MULTIPLE_ASSETS" {
                        // For folder dependencies, ASSEMBLY_PATH should be the relative path within assembly hierarchy
                        // This should be just the assembly path part, not the full path
                        dep.assembly_path.clone()
                    } else {
                        // For single asset dependencies, use the assembly_path as is
                        dep.assembly_path.clone()
                    }, // ASSEMBLY_PATH (the relative path within assembly hierarchy)
                    dep.path.clone(), // DEPENDENCY_PATH (the dependency path)
                    asset_uuid,       // ASSET_UUID
                    asset_filename,   // ASSET_NAME
                    asset_state,      // ASSET_STATE (added as requested)
                    dep.occurrences.to_string(), // OCCURRENCES
                    dep.has_dependencies.to_string(), // HAS_DEPENDENCIES
                ]
            })
            .collect()
    }
}

impl OutputFormatter for AssetDependencyList {
    type Item = AssetDependencyList;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                // Create a simplified representation for JSON output that includes state information
                #[derive(Serialize)]
                struct SimplifiedAssetDependency {
                    path: String,
                    assembly_path: String,
                    asset_id: Option<String>,
                    asset_name: Option<String>,
                    asset_state: Option<String>,
                    occurrences: u32,
                    has_dependencies: bool,
                }

                #[derive(Serialize)]
                struct SimplifiedAssetDependencyList {
                    asset_path: String,
                    dependencies: Vec<SimplifiedAssetDependency>,
                }

                let simplified_deps: Vec<SimplifiedAssetDependency> = self
                    .dependencies
                    .iter()
                    .map(|dep| {
                        let (asset_id, asset_name, asset_state) = match &dep.asset {
                            Some(asset) => {
                                let name = asset
                                    .path
                                    .split('/')
                                    .next_back()
                                    .unwrap_or(&asset.path)
                                    .to_string();
                                // Normalize state values - convert "missing-dependencies" to "missing" for consistency
                                let normalized_state = if asset.state == "missing-dependencies" {
                                    "missing".to_string()
                                } else {
                                    asset.state.clone()
                                };
                                (
                                    Some(if asset.uuid.is_nil() {
                                        "None".to_string()
                                    } else {
                                        asset.uuid.to_string()
                                    }),
                                    Some(name),
                                    Some(normalized_state),
                                )
                            }
                            None => {
                                // For missing dependencies, use the path as the name and mark as missing
                                let name = dep
                                    .path
                                    .split('/')
                                    .next_back()
                                    .unwrap_or(&dep.path)
                                    .to_string();
                                (None, Some(name), Some("missing".to_string())) // Mark missing dependencies with "missing" state
                            }
                        };

                        SimplifiedAssetDependency {
                            path: dep.path.clone(),
                            assembly_path: dep.assembly_path.clone(),
                            asset_id,
                            asset_name,
                            asset_state,
                            occurrences: dep.occurrences,
                            has_dependencies: dep.has_dependencies,
                        }
                    })
                    .collect();

                let simplified_list = SimplifiedAssetDependencyList {
                    asset_path: self.path.clone(),
                    dependencies: simplified_deps,
                };

                let result = if options.pretty {
                    serde_json::to_string_pretty(&simplified_list)
                } else {
                    serde_json::to_string(&simplified_list)
                };

                match result {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                use csv::Writer;
                use std::io::BufWriter;

                let buf = BufWriter::new(Vec::new());
                let mut wtr = Writer::from_writer(buf);

                if options.with_headers {
                    wtr.write_record(Self::csv_header())
                        .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?;
                }

                // Sort records by ASSET_PATH (index 0), then by ASSEMBLY_PATH (index 1)
                let mut records = self.as_csv_records();
                records.sort_by(|a, b| {
                    // First sort by ASSET_PATH (index 0)
                    match a[0].cmp(&b[0]) {
                        std::cmp::Ordering::Equal => {
                            // If ASSET_PATH is equal, sort by ASSEMBLY_PATH (index 1)
                            a[1].cmp(&b[1])
                        }
                        other => other,
                    }
                });

                for record in records {
                    wtr.write_record(&record)
                        .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?;
                }

                wtr.flush()
                    .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?;

                let bytes = wtr
                    .into_inner()
                    .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?
                    .into_inner()
                    .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?;

                String::from_utf8(bytes).map_err(|e| FormattingError::FormatFailure {
                    cause: Box::new(std::io::Error::other(e)),
                })
            }
            OutputFormat::Tree(_) => {
                // Create a tree representation using the ptree crate
                use ptree::TreeBuilder;

                let mut tree = TreeBuilder::new(
                    self.path
                        .split('/')
                        .next_back()
                        .unwrap_or(&self.path)
                        .to_string(),
                );

                for dep in &self.dependencies {
                    let (asset_name, asset_state) = match &dep.asset {
                        Some(asset) => {
                            let name = asset
                                .path
                                .split('/')
                                .next_back()
                                .unwrap_or(&asset.path)
                                .to_string();
                            // Normalize state values - convert "missing-dependencies" to "missing" for consistency
                            let normalized_state = if asset.state == "missing-dependencies" {
                                "missing".to_string()
                            } else {
                                asset.state.clone()
                            };
                            (name, normalized_state)
                        }
                        None => {
                            // If asset details are not available, use the path directly and mark as missing
                            let name = dep
                                .path
                                .split('/')
                                .next_back()
                                .unwrap_or(&dep.path)
                                .to_string();
                            (name, "missing".to_string())
                        }
                    };

                    let node_label = format!(
                        "{} [{}] ({} occurrences)",
                        asset_name, asset_state, dep.occurrences
                    );
                    tree.add_empty_child(node_label);
                }

                let tree = tree.build();

                let mut output = Vec::new();
                ptree::write_tree(&tree, &mut output)
                    .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })?;

                String::from_utf8(output)
                    .map_err(|e| FormattingError::FormatFailure { cause: Box::new(e) })
            }
        }
    }
}

impl OutputFormatter for AssemblyNode {
    type Item = AssemblyNode;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                // For JSON, serialize the node and its children recursively
                let json = if options.pretty {
                    serde_json::to_string_pretty(&self)
                } else {
                    serde_json::to_string(&self)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                // For CSV, we'll create a flat representation of the node and its children
                let mut records = Vec::new();

                // Add the current node
                records.push(vec![
                    self.asset().path(),
                    self.asset().name(),
                    self.asset().uuid().to_string(),
                    self.asset()
                        .processing_status()
                        .cloned()
                        .unwrap_or_default(),
                    self.children_len().to_string(),
                ]);

                // Add children recursively
                for child in self.children() {
                    let child_records = child.as_csv_records_recursive()?;
                    records.extend(child_records);
                }

                let mut wtr = Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "ASSET_PATH",
                        "ASSET_NAME",
                        "ASSET_UUID",
                        "ASSET_STATE",
                        "CHILDREN_COUNT",
                    ))?;
                }

                for record in records {
                    wtr.serialize(&record)?;
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(_) => {
                // For tree format, use ptree for better formatting
                Ok(self.build_ptree_string())
            }
        }
    }
}

impl AssemblyNode {
    /// Helper method to create CSV records recursively for all nodes in the tree
    #[allow(clippy::result_large_err)]
    fn as_csv_records_recursive(&self) -> Result<Vec<Vec<String>>, FormattingError> {
        let mut records = Vec::new();

        // Add current node
        records.push(vec![
            self.asset().path(),
            self.asset().name(),
            self.asset().uuid().to_string(),
            self.asset()
                .processing_status()
                .cloned()
                .unwrap_or_default(),
            self.children_len().to_string(),
        ]);

        // Add children recursively
        for child in self.children() {
            let child_records = child.as_csv_records_recursive()?;
            records.extend(child_records);
        }

        Ok(records)
    }

    /// Format the tree structure using ptree crate
    fn build_ptree_string(&self) -> String {
        fn build_ptree_recursive(builder: &mut TreeBuilder, node: &AssemblyNode) {
            for child in node.children() {
                let state = child
                    .asset()
                    .processing_status()
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("missing");
                let uuid_str = if child.asset().uuid().is_nil() {
                    "None".to_string()
                } else {
                    child.asset().uuid().to_string()
                };
                let child_label = format!("{} [{}] ({})", child.asset().name(), state, uuid_str);
                builder.begin_child(child_label);

                // Recursively add grandchildren
                build_ptree_recursive(builder, child);

                builder.end_child();
            }
        }

        let state = self
            .asset()
            .processing_status()
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("missing");
        let uuid_str = if self.asset().uuid().is_nil() {
            "None".to_string()
        } else {
            self.asset().uuid().to_string()
        };
        let mut builder = TreeBuilder::new(format!(
            "{} [{}] ({})",
            self.asset().name(),
            state,
            uuid_str
        ));

        // Add all direct children of the root node
        build_ptree_recursive(&mut builder, self);

        let tree = builder.build();

        let mut output = Vec::new();
        ptree::write_tree(&tree, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }
}

impl OutputFormatter for AssemblyTree {
    type Item = AssemblyTree;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                // For JSON, serialize the entire tree
                let json = if options.pretty {
                    serde_json::to_string_pretty(&self)
                } else {
                    serde_json::to_string(&self)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                // For CSV, use the root node's recursive CSV functionality
                let records = self.root().as_csv_records_recursive()?;

                let mut wtr = Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "ASSET_PATH",
                        "ASSET_NAME",
                        "ASSET_UUID",
                        "ASSET_STATE",
                        "CHILDREN_COUNT",
                    ))?;
                }

                for record in records {
                    wtr.serialize(&record)?;
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(_) => {
                // For tree format, use ptree for better formatting
                Ok(self.root().build_ptree_string())
            }
        }
    }
}

//! Structural diff of two assets' dependency trees.
//!
//! This module compares the recursive dependency trees of two assemblies (a
//! *reference* and a *candidate*) and produces a structural diff. The two trees
//! are walked in parallel and their child nodes are matched by **filename**
//! (the basename of the asset path). The comparison is **presence-only**: a part
//! is considered present or absent, and occurrence counts are ignored.
//!
//! The pure [`compute_dependency_diff`] function performs the comparison without
//! any I/O, which keeps the diff logic directly unit-testable. Output formatting
//! for [`DependencyDiff`] lives in `crate::format::impls::dependency_diff`.

use crate::model::{AssemblyNode, AssemblyTree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Presence status of a node in the dependency diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    /// Present in both the reference and candidate assemblies.
    Common,
    /// Present only in the reference assembly (i.e. removed in the candidate).
    OnlyInReference,
    /// Present only in the candidate assembly (i.e. added in the candidate).
    OnlyInCandidate,
}

impl DiffStatus {
    /// Short single-character marker used in tree output (`=`, `-`, `+`).
    pub fn marker(&self) -> &'static str {
        match self {
            DiffStatus::Common => "=",
            DiffStatus::OnlyInReference => "-",
            DiffStatus::OnlyInCandidate => "+",
        }
    }

    /// Stable snake_case label used in JSON and CSV output.
    pub fn label(&self) -> &'static str {
        match self {
            DiffStatus::Common => "common",
            DiffStatus::OnlyInReference => "only_in_reference",
            DiffStatus::OnlyInCandidate => "only_in_candidate",
        }
    }
}

/// Aggregate counts of nodes by status across the whole diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Number of nodes present in both assemblies.
    pub common: usize,
    /// Number of nodes present only in the reference assembly.
    pub only_in_reference: usize,
    /// Number of nodes present only in the candidate assembly.
    pub only_in_candidate: usize,
}

impl DiffSummary {
    fn bump(&mut self, status: DiffStatus) {
        match status {
            DiffStatus::Common => self.common += 1,
            DiffStatus::OnlyInReference => self.only_in_reference += 1,
            DiffStatus::OnlyInCandidate => self.only_in_candidate += 1,
        }
    }
}

/// A single node in the dependency diff tree.
///
/// A node carries the filename used as its match key, its presence status, and
/// representative asset details (taken from the reference side for `Common` and
/// `OnlyInReference` nodes, or the candidate side for `OnlyInCandidate` nodes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffNode {
    /// The filename (basename) used as the match key at this position.
    pub filename: String,
    /// Whether this node is common, only-in-reference, or only-in-candidate.
    pub status: DiffStatus,
    /// UUID of the representative asset, if known (`None` for nil/unresolved).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Processing state of the representative asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Full Physna path of the representative asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Child diff nodes.
    pub children: Vec<DiffNode>,
}

/// The result of diffing two assets' dependency trees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyDiff {
    /// Path of the reference assembly.
    pub reference: String,
    /// Path of the candidate assembly.
    pub candidate: String,
    /// Aggregate counts of nodes by status.
    pub summary: DiffSummary,
    /// Top-level diffed dependency nodes.
    pub nodes: Vec<DiffNode>,
}

/// Compute the structural diff of two dependency trees.
///
/// The roots of the two trees are the two input assemblies themselves; they are
/// aligned at the top regardless of their own names, and their child
/// dependencies are diffed recursively, matching by filename.
pub fn compute_dependency_diff(
    reference: &AssemblyTree,
    candidate: &AssemblyTree,
) -> DependencyDiff {
    let mut summary = DiffSummary::default();

    let ref_children: Vec<&AssemblyNode> = reference.root().children().collect();
    let cand_children: Vec<&AssemblyNode> = candidate.root().children().collect();
    let nodes = diff_level(&ref_children, &cand_children, &mut summary);

    DependencyDiff {
        reference: reference.root().asset().path(),
        candidate: candidate.root().asset().path(),
        summary,
        nodes,
    }
}

/// Extract the filename (basename) that identifies a node for matching.
///
/// Falls back to the asset name when the path has no trailing component.
fn node_filename(node: &AssemblyNode) -> String {
    let asset = node.asset();
    let path = asset.path();
    match path.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => asset.name(),
    }
}

/// Extract the representative `(uuid, state, path)` display details for a node.
fn node_details(node: &AssemblyNode) -> (Option<String>, Option<String>, Option<String>) {
    let asset = node.asset();
    let uuid = if asset.uuid().is_nil() {
        None
    } else {
        Some(asset.uuid().to_string())
    };
    let state = Some(asset.normalized_processing_status());
    let path = Some(asset.path());
    (uuid, state, path)
}

/// Group sibling nodes by filename, preserving first-seen order.
///
/// Duplicate filenames among siblings are collapsed into a single group; this
/// keeps the presence-only semantics consistent regardless of how many times a
/// part is repeated at a given level.
fn group_by_filename<'a>(nodes: &[&'a AssemblyNode]) -> Vec<(String, Vec<&'a AssemblyNode>)> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&'a AssemblyNode>> = HashMap::new();
    for &node in nodes {
        let filename = node_filename(node);
        if !groups.contains_key(&filename) {
            order.push(filename.clone());
        }
        groups.entry(filename).or_default().push(node);
    }
    order
        .into_iter()
        .map(|filename| {
            let group = groups.remove(&filename).expect("filename inserted above");
            (filename, group)
        })
        .collect()
}

/// Diff two sets of sibling nodes, matching by filename.
///
/// Common and reference-only nodes are emitted in reference order; candidate-only
/// nodes follow in candidate order. The result is deterministic.
fn diff_level(
    ref_nodes: &[&AssemblyNode],
    cand_nodes: &[&AssemblyNode],
    summary: &mut DiffSummary,
) -> Vec<DiffNode> {
    let ref_groups = group_by_filename(ref_nodes);
    let cand_groups = group_by_filename(cand_nodes);
    let cand_lookup: HashMap<&str, &Vec<&AssemblyNode>> = cand_groups
        .iter()
        .map(|(filename, group)| (filename.as_str(), group))
        .collect();

    let mut out = Vec::new();

    // Reference-driven pass: common nodes and nodes only in the reference.
    for (filename, ref_group) in &ref_groups {
        match cand_lookup.get(filename.as_str()) {
            Some(cand_group) => {
                summary.bump(DiffStatus::Common);
                let (uuid, state, path) = node_details(ref_group[0]);
                let ref_children: Vec<&AssemblyNode> =
                    ref_group.iter().flat_map(|node| node.children()).collect();
                let cand_children: Vec<&AssemblyNode> =
                    cand_group.iter().flat_map(|node| node.children()).collect();
                let children = diff_level(&ref_children, &cand_children, summary);
                out.push(DiffNode {
                    filename: filename.clone(),
                    status: DiffStatus::Common,
                    uuid,
                    state,
                    path,
                    children,
                });
            }
            None => {
                out.push(one_sided_group(
                    filename,
                    ref_group,
                    DiffStatus::OnlyInReference,
                    summary,
                ));
            }
        }
    }

    // Candidate-driven pass: nodes only in the candidate.
    let ref_filenames: HashMap<&str, ()> = ref_groups
        .iter()
        .map(|(filename, _)| (filename.as_str(), ()))
        .collect();
    for (filename, cand_group) in &cand_groups {
        if ref_filenames.contains_key(filename.as_str()) {
            continue;
        }
        out.push(one_sided_group(
            filename,
            cand_group,
            DiffStatus::OnlyInCandidate,
            summary,
        ));
    }

    out
}

/// Build a diff node (and its whole subtree) present on only one side.
fn one_sided_group(
    filename: &str,
    group: &[&AssemblyNode],
    status: DiffStatus,
    summary: &mut DiffSummary,
) -> DiffNode {
    summary.bump(status);
    let (uuid, state, path) = node_details(group[0]);
    let children: Vec<&AssemblyNode> =
        group.iter().flat_map(|node| node.children()).collect();
    let child_nodes = one_sided_level(&children, status, summary);
    DiffNode {
        filename: filename.to_string(),
        status,
        uuid,
        state,
        path,
        children: child_nodes,
    }
}

/// Build all diff nodes for a set of siblings present on only one side.
fn one_sided_level(
    nodes: &[&AssemblyNode],
    status: DiffStatus,
    summary: &mut DiffSummary,
) -> Vec<DiffNode> {
    group_by_filename(nodes)
        .iter()
        .map(|(filename, group)| one_sided_group(filename, group, status, summary))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Asset, AssemblyTree};
    use uuid::Uuid;

    /// Create a simple finished, non-assembly asset from a path.
    fn asset(path: &str) -> Asset {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        Asset::new(
            Uuid::nil(),
            name,
            path.to_string(),
            None,
            None,
            Some("finished".to_string()),
            None,
            None,
            None,
            false,
        )
    }

    /// Collect the top-level filenames of a diff by status, for concise asserts.
    fn top_level(diff: &DependencyDiff, status: DiffStatus) -> Vec<String> {
        diff.nodes
            .iter()
            .filter(|node| node.status == status)
            .map(|node| node.filename.clone())
            .collect()
    }

    #[test]
    fn identical_trees_are_all_common() {
        let mut reference = AssemblyTree::new(asset("/A.asm"));
        reference.root_mut().add_child_mut(asset("/parts/shaft.stl"));
        reference.root_mut().add_child_mut(asset("/parts/gear.stl"));

        let mut candidate = AssemblyTree::new(asset("/B.asm"));
        candidate.root_mut().add_child_mut(asset("/parts/shaft.stl"));
        candidate.root_mut().add_child_mut(asset("/parts/gear.stl"));

        let diff = compute_dependency_diff(&reference, &candidate);

        assert_eq!(diff.summary.common, 2);
        assert_eq!(diff.summary.only_in_reference, 0);
        assert_eq!(diff.summary.only_in_candidate, 0);
        assert_eq!(diff.reference, "/A.asm");
        assert_eq!(diff.candidate, "/B.asm");
    }

    #[test]
    fn detects_added_and_removed_parts() {
        let mut reference = AssemblyTree::new(asset("/A.asm"));
        reference.root_mut().add_child_mut(asset("/parts/shaft.stl"));
        reference.root_mut().add_child_mut(asset("/parts/old.stl"));

        let mut candidate = AssemblyTree::new(asset("/B.asm"));
        candidate.root_mut().add_child_mut(asset("/parts/shaft.stl"));
        candidate.root_mut().add_child_mut(asset("/parts/new.stl"));

        let diff = compute_dependency_diff(&reference, &candidate);

        assert_eq!(diff.summary.common, 1);
        assert_eq!(diff.summary.only_in_reference, 1);
        assert_eq!(diff.summary.only_in_candidate, 1);
        assert_eq!(top_level(&diff, DiffStatus::Common), vec!["shaft.stl"]);
        assert_eq!(top_level(&diff, DiffStatus::OnlyInReference), vec!["old.stl"]);
        assert_eq!(top_level(&diff, DiffStatus::OnlyInCandidate), vec!["new.stl"]);
    }

    #[test]
    fn recurses_into_common_subassemblies() {
        // Both have gearbox.asm, but its children differ.
        let mut reference = AssemblyTree::new(asset("/A.asm"));
        {
            let gearbox = reference.root_mut().add_child_mut(asset("/parts/gearbox.asm"));
            gearbox.add_child_mut(asset("/parts/bearing-v1.stl"));
            gearbox.add_child_mut(asset("/parts/shaft.stl"));
        }

        let mut candidate = AssemblyTree::new(asset("/B.asm"));
        {
            let gearbox = candidate.root_mut().add_child_mut(asset("/parts/gearbox.asm"));
            gearbox.add_child_mut(asset("/parts/bearing-v2.stl"));
            gearbox.add_child_mut(asset("/parts/shaft.stl"));
        }

        let diff = compute_dependency_diff(&reference, &candidate);

        // gearbox.asm + shaft.stl are common; bearing versions differ.
        assert_eq!(diff.summary.common, 2);
        assert_eq!(diff.summary.only_in_reference, 1);
        assert_eq!(diff.summary.only_in_candidate, 1);

        let gearbox = &diff.nodes[0];
        assert_eq!(gearbox.filename, "gearbox.asm");
        assert_eq!(gearbox.status, DiffStatus::Common);
        let removed: Vec<&str> = gearbox
            .children
            .iter()
            .filter(|n| n.status == DiffStatus::OnlyInReference)
            .map(|n| n.filename.as_str())
            .collect();
        let added: Vec<&str> = gearbox
            .children
            .iter()
            .filter(|n| n.status == DiffStatus::OnlyInCandidate)
            .map(|n| n.filename.as_str())
            .collect();
        assert_eq!(removed, vec!["bearing-v1.stl"]);
        assert_eq!(added, vec!["bearing-v2.stl"]);
    }

    #[test]
    fn one_sided_subtree_is_fully_marked() {
        // Reference has a whole subassembly the candidate lacks.
        let mut reference = AssemblyTree::new(asset("/A.asm"));
        {
            let sub = reference.root_mut().add_child_mut(asset("/parts/sub.asm"));
            sub.add_child_mut(asset("/parts/deep.stl"));
        }
        let candidate = AssemblyTree::new(asset("/B.asm"));

        let diff = compute_dependency_diff(&reference, &candidate);

        assert_eq!(diff.summary.only_in_reference, 2); // sub.asm + deep.stl
        assert_eq!(diff.summary.common, 0);
        assert_eq!(diff.summary.only_in_candidate, 0);
        let sub = &diff.nodes[0];
        assert_eq!(sub.status, DiffStatus::OnlyInReference);
        assert_eq!(sub.children[0].filename, "deep.stl");
        assert_eq!(sub.children[0].status, DiffStatus::OnlyInReference);
    }

    #[test]
    fn non_assemblies_produce_empty_diff() {
        let reference = AssemblyTree::new(asset("/A.stl"));
        let candidate = AssemblyTree::new(asset("/B.stl"));

        let diff = compute_dependency_diff(&reference, &candidate);

        assert!(diff.nodes.is_empty());
        assert_eq!(diff.summary, DiffSummary::default());
    }
}

//! Metadata fields and assembly dependencies.

use super::*;

/// Represents a metadata field definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataField {
    /// The field's id, which rename and delete address it by. Optional so a
    /// listing without ids (older responses, cached files) still reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    /// The name of the metadata field
    pub name: String,
    /// The type of the metadata field (e.g., "text", "number", etc.)
    #[serde(rename = "type")]
    pub field_type: String,
}

/// Represents a response containing a list of metadata fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataFieldListResponse {
    /// List of metadata fields
    #[serde(rename = "metadataFields")]
    pub metadata_fields: Vec<MetadataField>,
    /// Pagination information (absent if the endpoint returns an unpaginated list)
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
}

/// How a dependency of an assembly stands, as the API reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyStatus {
    /// The referenced file was found by its path.
    Matched,
    /// The reference was linked to an existing asset by hand (`asset resolve-dependency`).
    Resolved,
    /// No asset stands in for the reference; the assembly cannot be fully indexed.
    Missing,
    /// A value this build does not know yet. Physna adds values without bumping
    /// the API version; one new value must not turn a whole listing into a
    /// deserialization error. The weekly spec-drift check reports the new value.
    #[serde(other)]
    Unknown,
}

impl DependencyStatus {
    /// Every status, as the API spells them.
    pub const ALL: [&'static str; 3] = ["matched", "resolved", "missing"];

    /// The value as the API spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyStatus::Matched => "matched",
            DependencyStatus::Resolved => "resolved",
            DependencyStatus::Missing => "missing",
            DependencyStatus::Unknown => "unknown",
        }
    }
}

/// Represents a dependency relationship for an asset from the API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDependencyApiResponse {
    /// The Physna path of the dependent asset
    pub path: String,
    /// The asset details (optional because some dependencies may not have full asset details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<AssetResponse>,
    /// Number of occurrences
    pub occurrences: u32,
    /// Whether the dependency has its own dependencies
    #[serde(rename = "hasDependencies")]
    pub has_dependencies: bool,
    /// Matched, resolved or missing. Older responses may omit it, in which case
    /// a dependency without an asset is taken to be missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DependencyStatus>,
}

impl AssetDependencyApiResponse {
    /// True when no asset stands in for this dependency: the API says so, or
    /// it sent no asset details (the only signal older responses carry).
    pub fn is_missing(&self) -> bool {
        matches!(self.status, Some(DependencyStatus::Missing)) || self.asset.is_none()
    }
}

/// Represents a dependency relationship for an asset with assembly path information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDependency {
    /// The Physna path of the dependent asset
    pub path: String,
    /// The asset details (optional because some dependencies may not have full asset details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<AssetResponse>,
    /// Number of occurrences
    pub occurrences: u32,
    /// Whether the dependency has its own dependencies
    #[serde(rename = "hasDependencies")]
    pub has_dependencies: bool,
    /// The assembly path showing the location of this dependency within the assembly hierarchy
    #[serde(rename = "assemblyPath")]
    pub assembly_path: String,
    /// The path of the original asset that has this dependency (for folder dependencies)
    #[serde(rename = "originalAssetPath", skip_serializing_if = "Option::is_none")]
    pub original_asset_path: Option<String>,
}

impl From<AssetDependencyApiResponse> for AssetDependency {
    fn from(api_dep: AssetDependencyApiResponse) -> Self {
        // `status` is deliberately not carried over: this type is the CSV/JSON
        // shape of `asset dependencies`, which must not change under scripts.
        AssetDependency {
            path: api_dep.path,
            asset: api_dep.asset,
            occurrences: api_dep.occurrences,
            has_dependencies: api_dep.has_dependencies,
            assembly_path: String::new(), // Will be filled in by the tree building logic
            original_asset_path: None,    // Default to None for API responses
        }
    }
}

/// Represents the response from the asset dependencies API endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDependenciesResponse {
    /// List of assets that depend on this asset
    pub dependencies: Vec<AssetDependencyApiResponse>,
    /// Pagination data for the response
    #[serde(rename = "pageData")]
    pub page_data: PageData,
    /// The path of the original asset that was queried (for tree formatting)
    #[serde(skip_serializing, skip_deserializing)]
    pub original_asset_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDependencyList {
    pub path: String,
    pub dependencies: Vec<AssetDependency>,
}

impl From<AssetDependenciesResponse> for AssetDependencyList {
    fn from(response: AssetDependenciesResponse) -> Self {
        // Convert from API response to our internal representation
        let dependencies = response
            .dependencies
            .into_iter()
            .map(AssetDependency::from)
            .collect();

        Self {
            path: response.original_asset_path,
            dependencies,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyNode {
    asset: Asset,
    children: Option<Vec<AssemblyNode>>,
    /// How many times the parent assembly uses this part (a bolt used four times
    /// is one dependency with four occurrences).
    #[serde(default = "one_occurrence")]
    occurrences: u32,
}

fn one_occurrence() -> u32 {
    1
}

impl AssemblyNode {
    pub fn new(asset: Asset) -> Self {
        Self {
            asset,
            children: None,
            occurrences: 1,
        }
    }

    /// How many times the parent assembly uses this node's asset.
    pub fn occurrences(&self) -> u32 {
        self.occurrences
    }

    /// Add a dependency used `occurrences` times, returning the stored node.
    pub fn add_dependency_mut(&mut self, asset: Asset, occurrences: u32) -> &mut AssemblyNode {
        let child = self.add_child_mut(asset);
        child.occurrences = occurrences;
        child
    }

    pub fn asset(&self) -> &Asset {
        &self.asset
    }

    pub fn add_child_mut(&mut self, asset: Asset) -> &mut AssemblyNode {
        let children = self.children.get_or_insert_with(Vec::new);
        children.push(AssemblyNode::new(asset));
        children.last_mut().expect("just pushed")
    }

    pub fn has_children(&self) -> bool {
        self.children.is_some()
    }

    pub fn children(&self) -> std::slice::Iter<'_, AssemblyNode> {
        self.children
            .as_ref() // Option<&Vec<AssemblyNode>>
            .map(|v| v.iter()) // Option<Iter<AssemblyNode>>
            .unwrap_or_else(|| [].iter())
    }

    pub fn children_len(&self) -> usize {
        self.children.as_ref().map_or(0, |v| v.len())
    }
}

impl From<Asset> for AssemblyNode {
    fn from(asset: Asset) -> Self {
        Self::new(asset)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyTree {
    root: AssemblyNode,
}

impl AssemblyTree {
    pub fn new(asset: Asset) -> Self {
        let root = AssemblyNode::new(asset);
        Self { root }
    }

    pub fn root(&self) -> &AssemblyNode {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut AssemblyNode {
        &mut self.root
    }
}

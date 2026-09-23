//! Assets, asset listings and the API's page data.

use super::*;

/// Serialize a map with its keys in sorted order.
///
/// `HashMap` iterates in a different order on every run, so metadata in JSON output
/// came out shuffled and two runs over the same data were never byte-identical,
/// although the folder match help promises exactly that. Stored as a `HashMap`,
/// written as if it were a `BTreeMap`.
pub(crate) fn serialize_sorted<S, V>(
    map: &HashMap<String, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    V: Serialize,
{
    let sorted: std::collections::BTreeMap<&String, &V> = map.iter().collect();
    sorted.serialize(serializer)
}

// Asset models for Physna V3 API

/// Represents an asset response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetResponse {
    /// The UUID of the asset (mapped from JSON "id" field)
    #[serde(rename = "id")]
    pub uuid: Uuid,
    /// The UUID of the tenant that owns the asset
    #[serde(rename = "tenantId")]
    pub tenant_id: Uuid,
    /// The path of the asset
    pub path: String,
    /// The ID of the folder containing the asset
    #[serde(rename = "folderId", skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<Uuid>,
    /// The type of the asset
    #[serde(rename = "type", default)]
    pub asset_type: String,
    /// The creation timestamp of the asset
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    /// The last update timestamp of the asset
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
    /// The state of the asset
    #[serde(default)]
    pub state: String,
    /// Whether the asset is an assembly
    #[serde(rename = "isAssembly", default)]
    pub is_assembly: bool,
    /// Metadata associated with the asset. Defaulted so one item without the field
    /// does not fail a whole page of two hundred.
    #[serde(rename = "metadata", default, serialize_with = "serialize_sorted")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    /// The ID of the parent folder, if any
    #[serde(rename = "parentFolderId", skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<Uuid>,
    /// The ID of the owner, if any
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

impl AssetResponse {}

/// Represents a single asset response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleAssetResponse {
    /// The asset data
    #[serde(rename = "asset")]
    pub asset: AssetResponse,
}

/// Represents an asset list response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetListResponse {
    /// The list of assets (can be named either "assets" or "contents" in API responses)
    #[serde(alias = "contents")]
    pub assets: Vec<AssetResponse>,
    /// Pagination data
    /// Optional in the specification for the asset listing (cursor mode has
    /// none); a missing one reads as a single page.
    #[serde(rename = "pageData", default)]
    pub page_data: PageData,
}

impl FolderResponse {
    /// Convert the FolderResponse to a Folder with the specified path
    ///
    /// # Arguments
    /// * `path` - The path to use for the folder
    ///
    /// # Returns
    /// A new Folder instance
    pub fn to_folder(&self, path: String) -> Folder {
        Folder::from_folder_response(self.clone(), path)
    }
}

impl AssetListResponse {
    /// Convert the AssetListResponse to an AssetList
    ///
    /// # Returns
    /// A new AssetList instance containing the assets from the response
    pub fn to_asset_list(&self) -> AssetList {
        let mut asset_list = AssetList::empty();
        for asset_response in &self.assets {
            // For assets, use the path from the API response
            let asset: Asset = asset_response.into();
            asset_list.insert(asset);
        }
        asset_list
    }
}

/// Represents a single folder response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleFolderResponse {
    /// The folder data
    #[serde(rename = "folder")]
    pub folder: FolderResponse,
}

/// Represents a folder list response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderListResponse {
    /// The list of folders (can be named either "folders" or "contents" in API responses)
    #[serde(alias = "contents")]
    pub folders: Vec<FolderResponse>,
    /// Pagination data
    #[serde(rename = "pageData")]
    pub page_data: PageData,
}

impl FolderListResponse {
    /// Convert the FolderListResponse to a FolderList
    ///
    /// # Returns
    /// A new FolderList instance containing the folders from the response
    pub fn to_folder_list(&self) -> FolderList {
        let mut folder_list = FolderList::empty();
        for folder_response in &self.folders {
            // For now, we'll use the folder name as the path since we don't have the full hierarchy yet
            // In a real implementation, we would need to build the full hierarchy to get proper paths
            let folder = folder_response.to_folder(folder_response.name.clone());
            folder_list.add(folder);
        }
        folder_list
    }
}

/// Represents pagination data in API responses
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageData {
    /// Total number of items
    pub total: usize,
    /// Number of items per page
    #[serde(rename = "perPage")]
    pub per_page: usize,
    /// Current page number (1-based)
    #[serde(rename = "currentPage")]
    pub current_page: usize,
    /// Last page number
    #[serde(rename = "lastPage")]
    pub last_page: usize,
    /// Start index of items on this page
    #[serde(rename = "startIndex", default)]
    pub start_index: usize,
    /// End index of items on this page
    #[serde(rename = "endIndex", default)]
    pub end_index: usize,
}

impl Default for PageData {
    /// "Everything is on this one page": what a response without page data
    /// means to a pagination loop.
    fn default() -> Self {
        PageData {
            total: 0,
            per_page: 0,
            current_page: 1,
            last_page: 1,
            start_index: 0,
            end_index: 0,
        }
    }
}

// Asset models for Physna V3 API

/// Serialized as the bare map: the `meta` field is an implementation detail
/// that used to leak into `metadata get --format json` output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetMetadata {
    #[serde(serialize_with = "serialize_sorted")]
    meta: HashMap<String, String>,
}

impl Default for AssetMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetMetadata {
    pub fn new() -> Self {
        Self {
            meta: HashMap::new(),
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.meta.keys()
    }

    pub fn get(&self, key: &String) -> Option<&String> {
        self.meta.get(key)
    }
}

impl From<HashMap<String, String>> for AssetMetadata {
    fn from(meta: HashMap<String, String>) -> Self {
        Self { meta }
    }
}

/// A metadata value as a report cell.
///
/// Strings are shown as-is; numbers, booleans, and structures as their JSON text;
/// `null` as empty. Used for both sides of a match row so a numeric field compares
/// equal to itself - the reference side used to be stringified while the candidate
/// side was blanked for anything but a string, which marked every typed field as a
/// difference.
pub fn metadata_cell(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

impl From<HashMap<String, serde_json::Value>> for AssetMetadata {
    fn from(ht: HashMap<String, serde_json::Value>) -> Self {
        let meta: HashMap<String, String> = ht
            .iter()
            .map(|(k, v)| (k.to_owned(), metadata_cell(v)))
            .collect();

        Self::from(meta)
    }
}

/// Represents an asset in the Physna system
///
/// This struct represents an asset entity in the Physna system with both
/// internal tracking properties and API-related identifiers.
///
/// Assets are 3D models or other geometric files that can be stored in
/// Physna folders and subjected to geometric analysis and matching.
///
/// # Fields
/// * `uuid` - Unique identifier from the Physna API (required for API operations)
/// * `name` - Display name of the asset (derived from the file name part of the path)
/// * `path` - Full path of the asset in the folder hierarchy (e.g., "/Home/Folder/file.stl")
/// * `file_size` - Size of the uploaded file in bytes (optional)
/// * `file_type` - File type/extension (e.g., "stl", "step", "iges") (optional)
/// * `processing_status` - Current processing status (e.g., "processed", "processing", "failed") (optional)
/// * `created_at` - Creation timestamp (optional)
/// * `updated_at` - Last update timestamp (optional)
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    uuid: Uuid,
    name: String,
    path: String,
    file_size: Option<u64>,
    file_type: Option<String>,
    processing_status: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    metadata: Option<AssetMetadata>,
    is_assembly: bool,
}

// Equality is determined solely by name
impl PartialEq for Asset {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Asset {}

// Ordering is determined solely by name
impl PartialOrd for Asset {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Asset {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Asset {
    /// Create a new Asset instance
    ///
    /// # Arguments
    /// * `uuid` - Optional UUID from the API
    /// * `name` - Name of the asset
    /// * `path` - Full path of the asset
    /// * `file_size` - Optional file size of the asset
    /// * `file_type` - Optional file type of the asset
    /// * `processing_status` - Optional processing status of the asset
    /// * `created_at` - Optional creation timestamp of the asset
    /// * `updated_at` - Optional last update timestamp of the asset
    /// * `metadata` - Optional metadata key-value pairs for the asset
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uuid: Uuid,
        name: String,
        path: String,
        file_size: Option<u64>,
        file_type: Option<String>,
        processing_status: Option<String>,
        created_at: Option<String>,
        updated_at: Option<String>,
        metadata: Option<AssetMetadata>,
        is_assembly: bool,
    ) -> Asset {
        Asset {
            uuid,
            name,
            path,
            file_size,
            file_type,
            processing_status,
            created_at,
            updated_at,
            metadata,
            is_assembly,
        }
    }

    /// Get the UUID of the asset
    pub fn uuid(&self) -> Uuid {
        self.uuid.to_owned()
    }

    /// Set the name of the asset
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Get the name of the asset
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get the path of the asset
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// Set the path of the asset
    pub fn set_path(&mut self, path: String) {
        self.path = path;
    }

    /// Get the file size of the asset
    pub fn file_size(&self) -> Option<u64> {
        self.file_size
    }

    /// Get the file type of the asset
    pub fn file_type(&self) -> Option<&String> {
        self.file_type.as_ref()
    }

    /// Get the processing status of the asset
    pub fn processing_status(&self) -> Option<&String> {
        self.processing_status.as_ref()
    }

    /// Get the normalized processing status of the asset
    /// Converts "missing-dependencies" to "missing" for consistency
    pub fn normalized_processing_status(&self) -> String {
        match &self.processing_status {
            Some(status) => {
                if status == "missing-dependencies" {
                    "missing".to_string()
                } else {
                    status.clone()
                }
            }
            None => "missing".to_string(),
        }
    }

    /// Get the creation timestamp of the asset
    pub fn created_at(&self) -> Option<&String> {
        self.created_at.as_ref()
    }

    /// Get the last update timestamp of the asset
    pub fn updated_at(&self) -> Option<&String> {
        self.updated_at.as_ref()
    }

    /// Get the metadata of the asset
    pub fn metadata(&self) -> Option<&AssetMetadata> {
        self.metadata.as_ref()
    }

    /// Check if the asset is an assembly (has dependencies)
    pub fn is_assembly(&self) -> bool {
        self.is_assembly
    }

    /// Generate the thumbnail URL for this asset
    pub fn thumbnail_url(&self, base_url: &str, tenant_id: &str) -> String {
        format!(
            "{}/tenants/{}/assets/{}/thumbnail.png",
            base_url,
            tenant_id,
            self.uuid()
        )
    }
}

/// Asset with thumbnail URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetWithThumbnail {
    #[serde(flatten)]
    pub asset: Asset,
    pub thumbnail_url: String,
}

impl AssetWithThumbnail {
    /// Create a new AssetWithThumbnail instance
    pub fn new(asset: Asset, thumbnail_url: String) -> Self {
        AssetWithThumbnail {
            asset,
            thumbnail_url,
        }
    }
}

/// Asset list with thumbnail URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetListWithThumbnails {
    pub assets: Vec<AssetWithThumbnail>,
}

impl AssetListWithThumbnails {
    /// Create a new empty AssetListWithThumbnails
    pub fn empty() -> AssetListWithThumbnails {
        AssetListWithThumbnails { assets: Vec::new() }
    }

    /// Check if the AssetListWithThumbnails is empty
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Get the number of assets in the AssetListWithThumbnails
    pub fn len(&self) -> usize {
        self.assets.len()
    }
}

impl FromIterator<AssetWithThumbnail> for AssetListWithThumbnails {
    /// Create an AssetListWithThumbnails from an iterator of AssetWithThumbnail instances
    fn from_iter<I: IntoIterator<Item = AssetWithThumbnail>>(iter: I) -> AssetListWithThumbnails {
        let mut assets = AssetListWithThumbnails::empty();
        for asset in iter {
            assets.assets.push(asset);
        }
        assets
    }
}

impl From<Vec<AssetWithThumbnail>> for AssetListWithThumbnails {
    fn from(assets: Vec<AssetWithThumbnail>) -> Self {
        AssetListWithThumbnails { assets }
    }
}

// Note: Converting AssetList to AssetListWithThumbnails requires base_url and tenant_id
// which are not available in this context. This conversion is handled in the action layer.

impl From<AssetListWithThumbnails> for Vec<AssetWithThumbnail> {
    fn from(asset_list: AssetListWithThumbnails) -> Self {
        asset_list.assets
    }
}

impl From<AssetListWithThumbnails> for AssetList {
    fn from(_asset_list: AssetListWithThumbnails) -> Self {
        // This conversion is not directly possible since we'd lose the thumbnail info
        AssetList::empty()
    }
}

impl From<AssetListResponse> for AssetList {
    fn from(response: AssetListResponse) -> Self {
        response.to_asset_list()
    }
}

impl From<&AssetResponse> for Asset {
    fn from(asset_response: &AssetResponse) -> Self {
        // Extract the name from the path (last part after the last slash)
        let name = asset_response
            .path
            .rsplit_once('/')
            .map(|(_, name)| name.to_string())
            .unwrap_or_else(|| asset_response.path.clone());

        Asset::new(
            asset_response.uuid.to_owned(),
            name.to_string(),
            asset_response.path.clone(),
            None, // file_size not in current API response
            Some(asset_response.asset_type.clone()),
            Some(asset_response.state.clone()),
            Some(asset_response.created_at.clone()),
            Some(asset_response.updated_at.clone()),
            Some(asset_response.metadata.clone().into()),
            asset_response.is_assembly, // Pass the is_assembly field
        )
    }
}

impl From<AssetResponse> for Asset {
    fn from(asset_response: AssetResponse) -> Self {
        <Asset as From<&AssetResponse>>::from(&asset_response)
    }
}

/// A collection of Asset instances
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetList {
    /// Map of asset IDs to Asset instances
    assets: HashMap<Uuid, Asset>, // ID -> Asset
}

impl AssetList {
    /// Create a new empty AssetList
    pub fn empty() -> AssetList {
        AssetList {
            assets: HashMap::new(),
        }
    }

    /// Check if the AssetList is empty
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Get the number of assets in the AssetList
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Insert an asset into the AssetList
    ///
    /// # Arguments
    /// * `asset` - The asset to insert
    pub fn insert(&mut self, asset: Asset) {
        self.assets.insert(asset.uuid(), asset);
    }

    /// Remove an asset from the AssetList by ID
    ///
    /// # Arguments
    /// * `uuid` - The ID of the asset to remove
    pub fn remove(&mut self, uuid: &Uuid) {
        self.assets.remove(uuid);
    }

    /// Get an asset from the AssetList by UUID
    ///
    /// # Arguments
    /// * `uuid` - The UUID of the asset to retrieve
    ///
    /// # Returns
    /// * `Some(&Asset)` - If an asset with the specified ID exists
    /// * `None` - If no asset with the specified ID exists
    pub fn get(&self, uuid: &Uuid) -> Option<&Asset> {
        self.assets.get(uuid)
    }

    /// Get an iterator over all assets in the AssetList
    ///
    /// # Returns
    /// An iterator over the asset values
    pub fn iter(&self) -> impl Iterator<Item = &Asset> {
        self.assets.values()
    }

    /// Find an asset in the AssetList by name
    ///
    /// # Arguments
    /// * `name` - The name of the asset to find
    ///
    /// # Returns
    /// * `Some(&Asset)` - If an asset with the specified name exists
    /// * `None` - If no asset with the specified name exists
    pub fn find_by_name(&self, name: &String) -> Option<&Asset> {
        let result = self.assets.iter().find(|(_, f)| f.name.eq(name));

        match result {
            Some((_key, folder)) => Some(folder),
            None => None,
        }
    }

    /// Get all assets as a vector
    ///
    /// # Returns
    /// A vector containing all assets in the AssetList
    /// Every asset, ordered by path (then UUID), so whatever is built from the list
    /// comes out the same way on every run.
    pub fn get_all_assets(&self) -> Vec<&Asset> {
        let mut assets: Vec<&Asset> = self.assets.values().collect();
        assets.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.uuid.cmp(&b.uuid)));
        assets
    }
}

impl Default for AssetList {
    fn default() -> Self {
        AssetList::empty()
    }
}

impl FromIterator<Asset> for AssetList {
    /// Create an AssetList from an iterator of Asset instances
    fn from_iter<I: IntoIterator<Item = Asset>>(iter: I) -> AssetList {
        let mut assets = AssetList::empty();
        for a in iter {
            assets.insert(a);
        }

        assets
    }
}

impl From<Vec<Asset>> for AssetList {
    fn from(assets: Vec<Asset>) -> Self {
        AssetList::from_iter(assets)
    }
}

// Geometric search models

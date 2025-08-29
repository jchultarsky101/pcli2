//! Data models for the Physna CLI client.
//!
//! This module contains all the data structures used in the application,
//! including models for folders, assets, tenants, and API responses.
//! It also includes implementations for formatting these models in
//! various output formats like JSON, CSV, and tree representations.

use crate::format::{
    CsvRecordProducer, FormattingError, JsonProducer, OutputFormat, OutputFormatter,
};
use csv::Writer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufWriter;
use thiserror::Error;

/// Error types that can occur when working with models
#[derive(Debug, Error)]
pub enum ModelError {
    /// Error when a required property value is missing
    #[error("missing property value {name:?}")]
    MissingPropertyValue { name: String },
}

/// Represents a folder in the Physna system
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Folder {
    /// Internal ID of the folder (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    /// UUID of the folder from the API
    #[serde(skip_serializing_if = "Option::is_none")]
    uuid: Option<String>,
    /// Name of the folder
    name: String,
    /// Full path of the folder
    path: String,
}

impl Folder {
    /// Create a new Folder instance
    /// 
    /// # Arguments
    /// * `id` - Optional internal ID for the folder
    /// * `uuid` - Optional UUID from the API
    /// * `name` - Name of the folder
    /// * `path` - Full path of the folder
    pub fn new(id: Option<u32>, uuid: Option<String>, name: String, path: String) -> Folder {
        Folder { id, uuid, name, path }
    }
    
    /// Create a Folder from a FolderResponse with a specified path
    /// 
    /// # Arguments
    /// * `folder_response` - The API response containing folder data
    /// * `path` - The full path for this folder
    pub fn from_folder_response(folder_response: FolderResponse, path: String) -> Folder {
        Folder { 
            id: None, 
            uuid: Some(folder_response.id), 
            name: folder_response.name, 
            path 
        }
    }

    /// Set the internal ID of the folder
    #[allow(dead_code)]
    pub fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }

    /// Get the internal ID of the folder
    #[allow(dead_code)]
    pub fn id(&self) -> Option<u32> {
        self.id
    }
    
    /// Get the UUID of the folder
    #[allow(dead_code)]
    pub fn uuid(&self) -> Option<&String> {
        self.uuid.as_ref()
    }

    /// Set the name of the folder
    #[allow(dead_code)]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Get the name of the folder
    pub fn name(&self) -> String {
        self.name.clone()
    }
    
    /// Get the path of the folder
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// Create a new FolderBuilder for constructing Folder instances
    pub fn builder() -> FolderBuilder {
        FolderBuilder::new()
    }
}

impl CsvRecordProducer for Folder {
    /// Get the CSV header row for Folder records
    fn csv_header() -> Vec<String> {
        vec!["NAME".to_string(), "PATH".to_string()]
    }

    /// Convert the Folder to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![self.name(), self.path()]]
    }
    
    /// Generate CSV output with a header row
    fn to_csv_with_header(&self) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);
        wtr.write_record(&Self::csv_header())
            .map_err(|e| FormattingError::CsvWriterError(format!("Failed to write CSV header: {}", e)))?;
        
        // Sort records by folder name
        let mut records = self.as_csv_records();
        records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)
        
        for record in records {
            wtr.write_record(&record)
                .map_err(|e| FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e)))?;
        }
        let data = wtr.into_inner()
            .map_err(|e| FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e)))?;
        Ok(String::from_utf8(data)
            .map_err(|e| FormattingError::Utf8Error(e))?)
    }

    /// Generate CSV output without a header row
    fn to_csv_without_header(&self) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);
        
        // Sort records by folder name
        let mut records = self.as_csv_records();
        records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)
        
        for record in records {
            wtr.write_record(&record)
                .map_err(|e| FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e)))?;
        }
        let data = wtr.into_inner()
            .map_err(|e| FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e)))?;
        Ok(String::from_utf8(data)
            .map_err(|e| FormattingError::Utf8Error(e))?)
    }
}

impl JsonProducer for Folder {}

impl OutputFormatter for Folder {
    type Item = Folder;

    /// Format the Folder according to the specified output format
    /// 
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    /// 
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => Ok(self.to_json()?),
            OutputFormat::Csv => Ok(self.to_csv_with_header()?),
            OutputFormat::Tree => Ok(self.to_json()?), // For single folder, tree format is the same as JSON
        }
    }
}

/// Builder for constructing Folder instances with a fluent API
pub struct FolderBuilder {
    /// Internal ID of the folder
    id: Option<u32>,
    /// UUID of the folder from the API
    uuid: Option<String>,
    /// Name of the folder
    name: Option<String>,
    /// Full path of the folder
    path: Option<String>,
}

impl FolderBuilder {
    /// Create a new FolderBuilder
    fn new() -> FolderBuilder {
        FolderBuilder {
            id: None,
            uuid: None,
            name: None,
            path: None,
        }
    }

    /// Set the internal ID of the folder
    pub fn id(&mut self, id: u32) -> &mut FolderBuilder {
        self.id = Some(id);
        self
    }
    
    /// Set the UUID of the folder
    pub fn uuid(&mut self, uuid: String) -> &mut FolderBuilder {
        self.uuid = Some(uuid);
        self
    }
    
    /// Set the name of the folder
    pub fn name(&mut self, name: &String) -> &mut FolderBuilder {
        self.name = Some(name.clone());
        self
    }
    
    /// Set the path of the folder
    pub fn path(&mut self, path: String) -> &mut FolderBuilder {
        self.path = Some(path);
        self
    }

    /// Build the Folder instance
    /// 
    /// # Returns
    /// * `Ok(Folder)` - The constructed Folder instance
    /// * `Err(ModelError)` - If required properties are missing
    pub fn build(&self) -> Result<Folder, ModelError> {
        let name = match &self.name {
            Some(name) => name.clone(),
            None => {
                return Err(ModelError::MissingPropertyValue {
                    name: "name".to_string(),
                })
            }
        };
        
        let path = match &self.path {
            Some(path) => path.clone(),
            None => name.clone(),
        };
        
        Ok(Folder::new(self.id, self.uuid.clone(), name, path))
    }
}

/// A collection of Folder instances
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderList {
    /// Map of folder hash keys to Folder instances
    folders: HashMap<u32, Folder>,
}

impl FolderList {
    /// Create a new empty FolderList
    pub fn empty() -> FolderList {
        FolderList {
            folders: HashMap::new(),
        }
    }

    /// Check if the FolderList is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// Get the number of folders in the FolderList
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    /// Insert a folder into the FolderList
    /// 
    /// # Arguments
    /// * `folder` - The folder to insert
    pub fn insert(&mut self, folder: Folder) {
        // Use the full key to avoid hash collisions
        let key = format!("{}:{}", folder.name(), folder.path());
        let hash_key = key.chars().fold(0u32, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u32));
        self.folders.insert(hash_key, folder.clone());
    }

    /// Remove a folder from the FolderList by ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the folder to remove
    #[allow(dead_code)]
    pub fn remove(&mut self, id: &u32) {
        self.folders.remove(id);
    }

    /// Get a folder from the FolderList by ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the folder to retrieve
    /// 
    /// # Returns
    /// * `Some(&Folder)` - If a folder with the specified ID exists
    /// * `None` - If no folder with the specified ID exists
    #[allow(dead_code)]
    pub fn get(&self, id: &u32) -> Option<&Folder> {
        self.folders.get(id)
    }

    /// Find a folder in the FolderList by name
    /// 
    /// # Arguments
    /// * `name` - The name of the folder to find
    /// 
    /// # Returns
    /// * `Some(&Folder)` - If a folder with the specified name exists
    /// * `None` - If no folder with the specified name exists
    #[allow(dead_code)]
    pub fn find_by_name(&self, name: &String) -> Option<&Folder> {
        let result = self.folders.iter().find(|(_, f)| f.name.eq(name));

        match result {
            Some((_key, folder)) => Some(folder),
            None => None,
        }
    }
}

impl Default for FolderList {
    fn default() -> Self {
        FolderList::empty()
    }
}

impl FromIterator<Folder> for FolderList {
    /// Create a FolderList from an iterator of Folder instances
    fn from_iter<I: IntoIterator<Item = Folder>>(iter: I) -> FolderList {
        let mut folders = FolderList::empty();
        for f in iter {
            folders.insert(f);
        }

        folders
    }
}

impl CsvRecordProducer for FolderList {
    /// Get the CSV header row for FolderList records
    fn csv_header() -> Vec<String> {
        Folder::csv_header()
    }

    /// Convert the FolderList to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for (_, folder) in &self.folders {
            records.push(folder.as_csv_records()[0].clone());
        }

        records
    }
    
    /// Generate CSV output with a header row
    fn to_csv_with_header(&self) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);
        wtr.write_record(&Self::csv_header())
            .map_err(|e| FormattingError::CsvWriterError(format!("Failed to write CSV header: {}", e)))?;
        
        // Sort records by folder name
        let mut records = self.as_csv_records();
        records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)
        
        for record in records {
            wtr.write_record(&record)
                .map_err(|e| FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e)))?;
        }
        let data = wtr.into_inner()
            .map_err(|e| FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e)))?;
        Ok(String::from_utf8(data)
            .map_err(|e| FormattingError::Utf8Error(e))?)
    }
    
    /// Generate CSV output without a header row
    fn to_csv_without_header(&self) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);
        
        // Sort records by folder name
        let mut records = self.as_csv_records();
        records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)
        
        for record in records {
            wtr.write_record(&record)
                .map_err(|e| FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e)))?;
        }
        let data = wtr.into_inner()
            .map_err(|e| FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e)))?;
        Ok(String::from_utf8(data)
            .map_err(|e| FormattingError::Utf8Error(e))?)
    }
}

impl OutputFormatter for FolderList {
    type Item = FolderList;

    /// Format the FolderList according to the specified output format
    /// 
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    /// 
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => {
                // convert to a simple vector for output, sorted by name
                let mut folders: Vec<Folder> = self.folders.iter().map(|(_, f)| f.clone()).collect();
                folders.sort_by(|a, b| a.name().cmp(&b.name()));
                let json = serde_json::to_string_pretty(&folders);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv => {
                // Use the csv crate for proper escaping
                self.to_csv_with_header()
            }
            OutputFormat::Tree => {
                // For folder list, tree format is the same as JSON
                // In practice, tree format should be handled at the command level
                // where we have access to the full hierarchy
                let mut folders: Vec<Folder> = self.folders.iter().map(|(_, f)| f.clone()).collect();
                folders.sort_by(|a, b| a.name().cmp(&b.name()));
                let json = serde_json::to_string_pretty(&folders);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
        }
    }
}

// New models for Physna V3 API

/// Represents a tenant setting for a user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantSetting {
    /// The ID of the tenant
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    /// The role of the user in the tenant
    #[serde(rename = "tenantRole")]
    pub tenant_role: String,
    /// Whether the user is enabled in this tenant
    #[serde(rename = "userEnabled")]
    pub user_enabled: bool,
    /// The display name of the tenant
    #[serde(rename = "tenantDisplayName")]
    pub tenant_display_name: String,
    /// The short name of the tenant
    #[serde(rename = "tenantShortName")]
    pub tenant_short_name: String,
}

/// Represents a user in the Physna system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// The display name of the user
    #[serde(rename = "displayName")]
    pub display_name: String,
    /// The tenant settings for the user
    pub settings: Vec<TenantSetting>,
}

/// Represents the response for getting the current user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentUserResponse {
    /// The user data
    pub user: User,
}

impl CurrentUserResponse {
    /// Get a tenant setting by name (display name or short name)
    /// 
    /// # Arguments
    /// * `name` - The name of the tenant to find
    /// 
    /// # Returns
    /// * `Some(&TenantSetting)` - If a tenant with the specified name exists
    /// * `None` - If no tenant with the specified name exists
    pub fn get_tenant_by_name(&self, name: &str) -> Option<&TenantSetting> {
        self.user.settings.iter().find(|setting| {
            setting.tenant_display_name == name || setting.tenant_short_name == name
        })
    }
    
    /// Get a tenant setting by ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the tenant to find
    /// 
    /// # Returns
    /// * `Some(&TenantSetting)` - If a tenant with the specified ID exists
    /// * `None` - If no tenant with the specified ID exists
    pub fn get_tenant_by_id(&self, id: &str) -> Option<&TenantSetting> {
        self.user.settings.iter().find(|setting| setting.tenant_id == id)
    }
}

// Folder models for Physna V3 API

/// Represents a folder response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderResponse {
    /// The ID of the folder
    pub id: String,
    /// The ID of the tenant that owns the folder
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    /// The name of the folder
    pub name: String,
    /// The creation timestamp of the folder
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// The last update timestamp of the folder
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    /// The number of assets in the folder
    #[serde(rename = "assetsCount")]
    pub assets_count: u32,
    /// The number of subfolders in the folder
    #[serde(rename = "foldersCount")]
    pub folders_count: u32,
    /// The ID of the parent folder, if any
    #[serde(rename = "parentFolderId", skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
    /// The ID of the owner, if any
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

// Asset models for Physna V3 API

/// Represents an asset response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetResponse {
    /// The ID of the asset
    pub id: String,
    /// The ID of the tenant that owns the asset
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    /// The path of the asset
    pub path: String,
    /// The ID of the folder containing the asset
    #[serde(rename = "folderId", skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    /// The type of the asset
    #[serde(rename = "type")]
    pub asset_type: String,
    /// The creation timestamp of the asset
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// The last update timestamp of the asset
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    /// The state of the asset
    pub state: String,
    /// Whether the asset is an assembly
    #[serde(rename = "isAssembly")]
    pub is_assembly: bool,
    /// Metadata associated with the asset
    #[serde(rename = "metadata")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    /// The ID of the parent folder, if any
    #[serde(rename = "parentFolderId", skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
    /// The ID of the owner, if any
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

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
    /// The list of assets
    pub assets: Vec<AssetResponse>,
    /// Pagination data
    #[serde(rename = "pageData")]
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
            let path = asset_response.path.clone();
            let asset = Asset::from_asset_response(asset_response.clone(), path);
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
    /// The list of folders
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
            folder_list.insert(folder);
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
    #[serde(rename = "startIndex")]
    pub start_index: usize,
    /// End index of items on this page
    #[serde(rename = "endIndex")]
    pub end_index: usize,
}

// Asset models for Physna V3 API

/// Represents an asset in the Physna system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    /// Internal ID of the asset (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    /// UUID of the asset from the API
    #[serde(skip_serializing_if = "Option::is_none")]
    uuid: Option<String>,
    /// Name of the asset
    name: String,
    /// Full path of the asset
    path: String,
    /// File size of the asset (optional)
    #[serde(rename = "fileSize", skip_serializing_if = "Option::is_none")]
    file_size: Option<u64>,
    /// File type of the asset (optional)
    #[serde(rename = "fileType", skip_serializing_if = "Option::is_none")]
    file_type: Option<String>,
    /// Processing status of the asset (optional)
    #[serde(rename = "processingStatus", skip_serializing_if = "Option::is_none")]
    processing_status: Option<String>,
    /// Creation timestamp of the asset (optional)
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    /// Last update timestamp of the asset (optional)
    #[serde(rename = "updatedAt", skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

impl Asset {
    /// Create a new Asset instance
    /// 
    /// # Arguments
    /// * `id` - Optional internal ID for the asset
    /// * `uuid` - Optional UUID from the API
    /// * `name` - Name of the asset
    /// * `path` - Full path of the asset
    /// * `file_size` - Optional file size of the asset
    /// * `file_type` - Optional file type of the asset
    /// * `processing_status` - Optional processing status of the asset
    /// * `created_at` - Optional creation timestamp of the asset
    /// * `updated_at` - Optional last update timestamp of the asset
    pub fn new(id: Option<u32>, uuid: Option<String>, name: String, path: String, file_size: Option<u64>, file_type: Option<String>, processing_status: Option<String>, created_at: Option<String>, updated_at: Option<String>) -> Asset {
        Asset {
            id,
            uuid,
            name,
            path,
            file_size,
            file_type,
            processing_status,
            created_at,
            updated_at,
        }
    }
    
    /// Create an Asset from an AssetResponse with a specified path
    /// 
    /// # Arguments
    /// * `asset_response` - The API response containing asset data
    /// * `path` - The full path for this asset
    pub fn from_asset_response(asset_response: AssetResponse, path: String) -> Asset {
        // Extract the name from the path (last part after the last slash)
        let name = asset_response.path.split('/').last().unwrap_or(&asset_response.path).to_string();
        
        Asset::new(
            Some(asset_response.id.chars().take(8).fold(0u32, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u32))),
            Some(asset_response.id.clone()),
            name,
            path,
            None, // file_size not in current API response
            Some(asset_response.asset_type),
            Some(asset_response.state),
            Some(asset_response.created_at),
            Some(asset_response.updated_at),
        )
    }

    /// Set the internal ID of the asset
    #[allow(dead_code)]
    pub fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }

    /// Get the internal ID of the asset
    #[allow(dead_code)]
    pub fn id(&self) -> Option<u32> {
        self.id
    }
    
    /// Get the UUID of the asset
    #[allow(dead_code)]
    pub fn uuid(&self) -> Option<&String> {
        self.uuid.as_ref()
    }

    /// Set the name of the asset
    #[allow(dead_code)]
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
    
    /// Get the creation timestamp of the asset
    pub fn created_at(&self) -> Option<&String> {
        self.created_at.as_ref()
    }
    
    /// Get the last update timestamp of the asset
    pub fn updated_at(&self) -> Option<&String> {
        self.updated_at.as_ref()
    }
}

impl CsvRecordProducer for Asset {
    /// Get the CSV header row for Asset records
    fn csv_header() -> Vec<String> {
        vec!["NAME".to_string(), "PATH".to_string(), "TYPE".to_string(), "STATE".to_string(), "UUID".to_string()]
    }

    /// Convert the Asset to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.name(), 
            self.path(), 
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default(),
            self.uuid().cloned().unwrap_or_default()
        ]]
    }
}

impl JsonProducer for Asset {}

impl OutputFormatter for Asset {
    type Item = Asset;

    /// Format the Asset according to the specified output format
    /// 
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    /// 
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => Ok(self.to_json()?),
            OutputFormat::Csv => Ok(self.to_csv_with_header()?),
            // No tree format for assets - they're not hierarchical
            OutputFormat::Tree => {
                // For single asset, tree format is the same as JSON
                Ok(self.to_json()?)
            }
        }
    }
}

/// A collection of Asset instances
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetList {
    /// Map of asset IDs to Asset instances
    assets: HashMap<u32, Asset>, // ID -> Asset
}

impl AssetList {
    /// Create a new empty AssetList
    pub fn empty() -> AssetList {
        AssetList {
            assets: HashMap::new(),
        }
    }

    /// Check if the AssetList is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Get the number of assets in the AssetList
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Insert an asset into the AssetList
    /// 
    /// # Arguments
    /// * `asset` - The asset to insert
    pub fn insert(&mut self, asset: Asset) {
        if let Some(id) = asset.id() {
            self.assets.insert(id, asset);
        }
    }

    /// Remove an asset from the AssetList by ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the asset to remove
    #[allow(dead_code)]
    pub fn remove(&mut self, id: &u32) {
        self.assets.remove(id);
    }

    /// Get an asset from the AssetList by ID
    /// 
    /// # Arguments
    /// * `id` - The ID of the asset to retrieve
    /// 
    /// # Returns
    /// * `Some(&Asset)` - If an asset with the specified ID exists
    /// * `None` - If no asset with the specified ID exists
    #[allow(dead_code)]
    pub fn get(&self, id: &u32) -> Option<&Asset> {
        self.assets.get(id)
    }

    /// Find an asset in the AssetList by name
    /// 
    /// # Arguments
    /// * `name` - The name of the asset to find
    /// 
    /// # Returns
    /// * `Some(&Asset)` - If an asset with the specified name exists
    /// * `None` - If no asset with the specified name exists
    #[allow(dead_code)]
    pub fn find_by_name(&self, name: &String) -> Option<&Asset> {
        let result = self.assets.iter().find(|(_, f)| f.name.eq(name));

        match result {
            Some((_key, folder)) => Some(folder),
            None => None,
        }
    }
    
    /// Find an asset in the AssetList by path
    /// 
    /// # Arguments
    /// * `path` - The path of the asset to find
    /// 
    /// # Returns
    /// * `Some(&Asset)` - If an asset with the specified path exists
    /// * `None` - If no asset with the specified path exists
    pub fn find_by_path(&self, path: &str) -> Option<&Asset> {
        let result = self.assets.iter().find(|(_, a)| a.path().eq(path));

        match result {
            Some((_key, asset)) => Some(asset),
            None => None,
        }
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

impl CsvRecordProducer for AssetList {
    /// Get the CSV header row for AssetList records
    fn csv_header() -> Vec<String> {
        Asset::csv_header()
    }

    /// Convert the AssetList to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for (_, asset) in &self.assets {
            records.push(asset.as_csv_records()[0].clone());
        }

        records
    }
}

impl OutputFormatter for AssetList {
    type Item = AssetList;

    /// Format the AssetList according to the specified output format
    /// 
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    /// 
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => {
                // convert to a simple vector for output and sort by path
                let mut assets: Vec<Asset> = self.assets.iter().map(|(_, f)| f.clone()).collect();
                assets.sort_by(|a, b| a.path().cmp(&b.path()));
                let json = serde_json::to_string_pretty(&assets);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv => {
                let buf = BufWriter::new(Vec::new());
                let mut wtr = Writer::from_writer(buf);
                wtr.write_record(&Self::csv_header()).unwrap();
                
                // Sort records by asset path
                let mut records = self.as_csv_records();
                records.sort_by(|a, b| a[1].cmp(&b[1])); // Sort by PATH column (index 1)
                
                for record in records {
                    wtr.write_record(&record).unwrap();
                }
                match wtr.flush() {
                    Ok(_) => {
                        let bytes = wtr.into_inner().unwrap().into_inner().unwrap();
                        let csv = String::from_utf8(bytes).unwrap();
                        Ok(csv.clone())
                    }
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Tree => {
                // For asset list, tree format is the same as JSON
                // In practice, tree format should be handled at the command level
                // where we have access to the full hierarchy
                // convert to a simple vector for output and sort by path
                let mut assets: Vec<Asset> = self.assets.iter().map(|(_, f)| f.clone()).collect();
                assets.sort_by(|a, b| a.path().cmp(&b.path()));
                let json = serde_json::to_string_pretty(&assets);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
        }
    }
}

// Geometric search models

/// Represents a match result from the geometric search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricMatch {
    /// The matching asset details
    pub asset: AssetResponse,
    /// The similarity percentage
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
}

impl GeometricMatch {
    /// Get the asset ID
    pub fn asset_id(&self) -> &str {
        &self.asset.id
    }
    
    /// Get the asset path
    pub fn path(&self) -> &str {
        &self.asset.path
    }
    
    /// Get the similarity score (0.0 to 100.0)
    pub fn score(&self) -> f64 {
        self.match_percentage
    }
}

/// Represents a 4x4 transformation matrix
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformationMatrix {
    /// The 4x4 matrix values in row-major order
    pub matrix: [f64; 16],
}

/// Represents the response from the geometric search API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricSearchResponse {
    /// The list of matching assets
    pub matches: Vec<GeometricMatch>,
}

impl CsvRecordProducer for GeometricSearchResponse {
    /// Get the CSV header row for GeometricSearchResponse records
    fn csv_header() -> Vec<String> {
        vec!["ASSET_ID".to_string(), "PATH".to_string(), "SCORE".to_string()]
    }

    /// Convert the GeometricSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches.iter().map(|m| {
            vec![
                m.asset_id().to_string(),
                m.path().to_string(),
                format!("{:.2}", m.score())
            ]
        }).collect()
    }
}

impl JsonProducer for GeometricSearchResponse {}

impl OutputFormatter for GeometricSearchResponse {
    type Item = GeometricSearchResponse;

    /// Format the GeometricSearchResponse according to the specified output format
    /// 
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
    /// 
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => Ok(self.to_json()?),
            OutputFormat::Csv => Ok(self.to_csv_with_header()?),
            OutputFormat::Tree => Ok(self.to_json()?), // For geometric search, tree format is the same as JSON
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_creation() {
        let id: u32 = 100;
        let uuid: String = "test-uuid".to_string();
        let name: String = "some_folder_name".to_string();
        let path: String = "some/path".to_string();

        let folder = Folder::new(Some(id), Some(uuid.clone()), name.clone(), path.clone());
        assert_eq!(Some(id), folder.id());
        assert_eq!(Some(&uuid), folder.uuid());
        assert_eq!(name, folder.name());
        assert_eq!(path, folder.path());
    }

    #[test]
    fn test_folder_builder() {
        let id: u32 = 110;
        let uuid: String = "test-uuid".to_string();
        let name: String = "some_other_name".to_string();
        let path: String = "some/path".to_string();

        let folder = Folder::builder().id(id).uuid(uuid.clone()).name(&name).path(path.clone()).build().unwrap();
        assert_eq!(Some(id), folder.id());
        assert_eq!(Some(uuid), folder.uuid().cloned());
        assert_eq!(name, folder.name());
        assert_eq!(path, folder.path());
    }

    #[test]
    fn test_output_format() {
        let id: u32 = 120;
        let uuid: String = "test-uuid".to_string();
        let name: String = "folder_name".to_string();
        let path: String = "folder_name".to_string();

        let folder = Folder::builder().id(id).uuid(uuid.clone()).name(&name).path(path.clone()).build().unwrap();
        let json = folder.format(OutputFormat::Json).unwrap();
        let json_expected = r#"{
  "id": 120,
  "uuid": "test-uuid",
  "name": "folder_name",
  "path": "folder_name"
}"#;
        assert_eq!(json_expected, json);

        let csv = folder.format(OutputFormat::Csv).unwrap();
        let csv_expected = r#"NAME,PATH
folder_name,folder_name
"#;
        assert_eq!(csv_expected, csv);
    }
}
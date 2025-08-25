use crate::format::{
    CsvRecordProducer, FormattingError, JsonProducer, OutputFormat, OutputFormatter,
};
use csv::Writer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufWriter;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("missing property value {name:?}")]
    MissingPropertyValue { name: String },
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Folder {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uuid: Option<String>,
    name: String,
    path: String,
}

impl Folder {
    pub fn new(id: Option<u32>, uuid: Option<String>, name: String, path: String) -> Folder {
        Folder { id, uuid, name, path }
    }
    
    pub fn from_folder_response(folder_response: FolderResponse, path: String) -> Folder {
        Folder { 
            id: None, 
            uuid: Some(folder_response.id), 
            name: folder_response.name, 
            path 
        }
    }

    #[allow(dead_code)]
    pub fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }

    #[allow(dead_code)]
    pub fn id(&self) -> Option<u32> {
        self.id
    }
    
    #[allow(dead_code)]
    pub fn uuid(&self) -> Option<&String> {
        self.uuid.as_ref()
    }

    #[allow(dead_code)]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }
    
    pub fn path(&self) -> String {
        self.path.clone()
    }

    pub fn builder() -> FolderBuilder {
        FolderBuilder::new()
    }
}

impl CsvRecordProducer for Folder {
    fn csv_header() -> Vec<String> {
        vec!["NAME".to_string(), "PATH".to_string()]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![self.name(), self.path()]]
    }
    
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

    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => Ok(self.to_json()?),
            OutputFormat::Csv => Ok(self.to_csv_with_header()?),
            OutputFormat::Tree => Ok(self.to_json()?), // For single folder, tree format is the same as JSON
        }
    }
}

pub struct FolderBuilder {
    id: Option<u32>,
    uuid: Option<String>,
    name: Option<String>,
    path: Option<String>,
}

impl FolderBuilder {
    fn new() -> FolderBuilder {
        FolderBuilder {
            id: None,
            uuid: None,
            name: None,
            path: None,
        }
    }

    pub fn id(&mut self, id: u32) -> &mut FolderBuilder {
        self.id = Some(id);
        self
    }
    
    pub fn uuid(&mut self, uuid: String) -> &mut FolderBuilder {
        self.uuid = Some(uuid);
        self
    }
    
    pub fn name(&mut self, name: &String) -> &mut FolderBuilder {
        self.name = Some(name.clone());
        self
    }
    
    pub fn path(&mut self, path: String) -> &mut FolderBuilder {
        self.path = Some(path);
        self
    }

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderList {
    folders: HashMap<u32, Folder>,
}

impl FolderList {
    pub fn empty() -> FolderList {
        FolderList {
            folders: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    pub fn insert(&mut self, folder: Folder) {
        // Use the full key to avoid hash collisions
        let key = format!("{}:{}", folder.name(), folder.path());
        let hash_key = key.chars().fold(0u32, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u32));
        self.folders.insert(hash_key, folder.clone());
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, id: &u32) {
        self.folders.remove(id);
    }

    #[allow(dead_code)]
    pub fn get(&self, id: &u32) -> Option<&Folder> {
        self.folders.get(id)
    }

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
    fn from_iter<I: IntoIterator<Item = Folder>>(iter: I) -> FolderList {
        let mut folders = FolderList::empty();
        for f in iter {
            folders.insert(f);
        }

        folders
    }
}

impl CsvRecordProducer for FolderList {
    fn csv_header() -> Vec<String> {
        Folder::csv_header()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for (_, folder) in &self.folders {
            records.push(folder.as_csv_records()[0].clone());
        }

        records
    }
    
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantSetting {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "tenantRole")]
    pub tenant_role: String,
    #[serde(rename = "userEnabled")]
    pub user_enabled: bool,
    #[serde(rename = "tenantDisplayName")]
    pub tenant_display_name: String,
    #[serde(rename = "tenantShortName")]
    pub tenant_short_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub settings: Vec<TenantSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentUserResponse {
    pub user: User,
}

impl CurrentUserResponse {
    pub fn get_tenant_by_name(&self, name: &str) -> Option<&TenantSetting> {
        self.user.settings.iter().find(|setting| {
            setting.tenant_display_name == name || setting.tenant_short_name == name
        })
    }
    
    pub fn get_tenant_by_id(&self, id: &str) -> Option<&TenantSetting> {
        self.user.settings.iter().find(|setting| setting.tenant_id == id)
    }
}

// Folder models for Physna V3 API

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderResponse {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub name: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "assetsCount")]
    pub assets_count: u32,
    #[serde(rename = "foldersCount")]
    pub folders_count: u32,
    #[serde(rename = "parentFolderId", skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

// Asset models for Physna V3 API

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetResponse {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub path: String,
    #[serde(rename = "folderId")]
    pub folder_id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub state: String,
    #[serde(rename = "isAssembly")]
    pub is_assembly: bool,
    #[serde(rename = "metadata")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    #[serde(rename = "parentFolderId", skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleAssetResponse {
    #[serde(rename = "asset")]
    pub asset: AssetResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetListResponse {
    pub assets: Vec<AssetResponse>,
    #[serde(rename = "pageData")]
    pub page_data: PageData,
}

impl FolderResponse {
    pub fn to_folder(&self, path: String) -> Folder {
        Folder::from_folder_response(self.clone(), path)
    }
}

impl AssetListResponse {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleFolderResponse {
    #[serde(rename = "folder")]
    pub folder: FolderResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderListResponse {
    pub folders: Vec<FolderResponse>,
    #[serde(rename = "pageData")]
    pub page_data: PageData,
}

impl FolderListResponse {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageData {
    pub total: usize,
    #[serde(rename = "perPage")]
    pub per_page: usize,
    #[serde(rename = "currentPage")]
    pub current_page: usize,
    #[serde(rename = "lastPage")]
    pub last_page: usize,
    #[serde(rename = "startIndex")]
    pub start_index: usize,
    #[serde(rename = "endIndex")]
    pub end_index: usize,
}

// Asset models for Physna V3 API

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uuid: Option<String>,
    name: String,
    path: String,
    #[serde(rename = "fileSize", skip_serializing_if = "Option::is_none")]
    file_size: Option<u64>,
    #[serde(rename = "fileType", skip_serializing_if = "Option::is_none")]
    file_type: Option<String>,
    #[serde(rename = "processingStatus", skip_serializing_if = "Option::is_none")]
    processing_status: Option<String>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(rename = "updatedAt", skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

impl Asset {
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

    #[allow(dead_code)]
    pub fn set_id(&mut self, id: u32) {
        self.id = Some(id);
    }

    #[allow(dead_code)]
    pub fn id(&self) -> Option<u32> {
        self.id
    }
    
    #[allow(dead_code)]
    pub fn uuid(&self) -> Option<&String> {
        self.uuid.as_ref()
    }

    #[allow(dead_code)]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }
    
    pub fn path(&self) -> String {
        self.path.clone()
    }
    
    pub fn file_size(&self) -> Option<u64> {
        self.file_size
    }
    
    pub fn file_type(&self) -> Option<&String> {
        self.file_type.as_ref()
    }
    
    pub fn processing_status(&self) -> Option<&String> {
        self.processing_status.as_ref()
    }
    
    pub fn created_at(&self) -> Option<&String> {
        self.created_at.as_ref()
    }
    
    pub fn updated_at(&self) -> Option<&String> {
        self.updated_at.as_ref()
    }
}

impl CsvRecordProducer for Asset {
    fn csv_header() -> Vec<String> {
        vec!["NAME".to_string(), "PATH".to_string(), "TYPE".to_string(), "STATE".to_string()]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.name(), 
            self.path(), 
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default()
        ]]
    }
}

impl JsonProducer for Asset {}

impl OutputFormatter for Asset {
    type Item = Asset;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetList {
    assets: HashMap<u32, Asset>, // ID -> Asset
}

impl AssetList {
    pub fn empty() -> AssetList {
        AssetList {
            assets: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn insert(&mut self, asset: Asset) {
        if let Some(id) = asset.id() {
            self.assets.insert(id, asset);
        }
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, id: &u32) {
        self.assets.remove(id);
    }

    #[allow(dead_code)]
    pub fn get(&self, id: &u32) -> Option<&Asset> {
        self.assets.get(id)
    }

    #[allow(dead_code)]
    pub fn find_by_name(&self, name: &String) -> Option<&Asset> {
        let result = self.assets.iter().find(|(_, f)| f.name.eq(name));

        match result {
            Some((_key, folder)) => Some(folder),
            None => None,
        }
    }
    
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
    fn from_iter<I: IntoIterator<Item = Asset>>(iter: I) -> AssetList {
        let mut assets = AssetList::empty();
        for a in iter {
            assets.insert(a);
        }

        assets
    }
}

impl CsvRecordProducer for AssetList {
    fn csv_header() -> Vec<String> {
        Asset::csv_header()
    }

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
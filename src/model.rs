//! Data models for the Physna CLI client.
//!
//! This module contains all the data structures used throughout the application,
//! including models for folders, assets, tenants, and API responses.
//! It also includes implementations for formatting these models in
//! various output formats like JSON, CSV, and tree representations.
//!
//! The models follow a layered approach:
//! - API response models (e.g., AssetResponse, FolderResponse) - direct mapping from API JSON
//! - Internal models (e.g., Asset, Folder) - business logic models with additional functionality
//! - Collection models (e.g., AssetList, FolderList) - collections of individual models
//!
//! All models implement serialization/deserialization with serde,
//! output formatting capabilities, and appropriate error handling.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use csv::Writer;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::BufWriter;
use thiserror::Error;
use uuid::Uuid;

/// Error types that can occur when working with models
#[derive(Debug, Error)]
pub enum ModelError {
    /// Error when a required property value is missing
    #[error("missing property value {name:?}")]
    MissingPropertyValue { name: String },
}

/// Normalize a path with these rules:
/// 1) Remove leading "/HOME" if present
/// 2) Remove any trailing '/'
/// 3) Collapse multiple consecutive '/' into a single '/'
/// 4) Ensure the result starts with exactly one '/'
///
/// Examples:
///   "/myroot/mysub/more/"         -> "/myroot/mysub/more"
///   "myroot/mysub/more"           -> "/myroot/mysub/more"
///   "/HOME/myroot/mysub/more/"    -> "/myroot/mysub/more"
///   "/HOME"                       -> "/"
///   "////"                        -> "/"
///   "/myroot//mysub///more/"      -> "/myroot/mysub/more"
pub fn normalize_path(path: impl AsRef<str>) -> String {
    let mut s = path.as_ref().trim();

    // Case-insensitive check for prefix "/HOME/"
    if s.to_ascii_lowercase().starts_with("/home/") {
        // SAFETY: only slice the original string, not the lowercase temp
        s = &s[5..]; // remove `/HOME` (5 chars)
    } else if s.eq_ignore_ascii_case("/home") {
        return "/".into();
    }

    // Remove trailing '/'
    s = s.trim_end_matches('/');

    // Split by '/' and filter out empty parts to collapse multiple consecutive slashes
    let parts: Vec<&str> = s.split('/').filter(|part| !part.is_empty()).collect();
    let result = parts.join("/");

    // Handle the case where the original path was just slashes (e.g. "/", "//", "///")
    let without_leading = if !result.is_empty() {
        result.as_str()
    } else {
        ""
    };

    // Ensure exactly one leading '/'
    let mut out = String::with_capacity(without_leading.len() + 1);
    out.push('/');
    out.push_str(without_leading);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_basic_cases() {
        assert_eq!(normalize_path("/myroot/mysub/more/"), "/myroot/mysub/more");
        assert_eq!(normalize_path("myroot/mysub/more"), "/myroot/mysub/more");
        assert_eq!(
            normalize_path("/HOME/myroot/mysub/more/"),
            "/myroot/mysub/more"
        );
        assert_eq!(normalize_path("/HOME"), "/");
        assert_eq!(normalize_path("////"), "/");
    }

    #[test]
    fn test_normalize_path_consecutive_slashes() {
        assert_eq!(
            normalize_path("/myroot//mysub///more/"),
            "/myroot/mysub/more"
        );
        assert_eq!(normalize_path("Root//Folder"), "/Root/Folder");
        assert_eq!(
            normalize_path("//double//slash//test"),
            "/double/slash/test"
        );
        assert_eq!(normalize_path("///"), "/");
        assert_eq!(normalize_path(""), "/");
    }

    #[test]
    fn test_normalize_path_home_handling() {
        assert_eq!(normalize_path("/HOME"), "/");
        assert_eq!(normalize_path("/home"), "/");
        assert_eq!(normalize_path("/HOME/"), "/");
        assert_eq!(normalize_path("/home/"), "/");
        assert_eq!(normalize_path("/HOME/test"), "/test");
        assert_eq!(normalize_path("/home/test"), "/test");
        assert_eq!(normalize_path("/HOME/test/"), "/test");
        assert_eq!(normalize_path("/home/test/"), "/test");

        // Ensure case insensitivity
        assert_eq!(normalize_path("/HoMe"), "/");
        assert_eq!(normalize_path("/hOmE/test"), "/test");
    }

    #[test]
    fn test_normalize_path_edge_cases() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("   "), "/");
        assert_eq!(normalize_path("   /   "), "/");
        assert_eq!(normalize_path("   /test/   "), "/test");
        assert_eq!(normalize_path("test"), "/test");
        assert_eq!(normalize_path("test/"), "/test");
        assert_eq!(normalize_path("/test"), "/test");
        assert_eq!(normalize_path("/////test"), "/test");
        assert_eq!(normalize_path("test/////"), "/test");
    }

    #[test]
    fn test_normalize_path_trailing_slashes() {
        assert_eq!(normalize_path("/test/"), "/test");
        assert_eq!(normalize_path("/test//"), "/test");
        assert_eq!(normalize_path("/test///"), "/test");
        assert_eq!(normalize_path("test/"), "/test");
        assert_eq!(normalize_path("test//"), "/test");
        assert_eq!(normalize_path("test///"), "/test");
    }

    #[test]
    fn test_normalize_path_leading_slashes() {
        assert_eq!(normalize_path("//test"), "/test");
        assert_eq!(normalize_path("///test"), "/test");
        assert_eq!(normalize_path("////test"), "/test");
        assert_eq!(normalize_path("/////test"), "/test");
    }
}

/// Represents a folder in the Physna system
///
/// This struct represents a folder entity in the Physna system with both
/// internal tracking properties and API-related identifiers.
///
/// Folders form a hierarchical structure in Physna and can contain both
/// subfolders and assets. The path property represents the full path to
/// the folder from the root.
///
/// # Fields
/// * `uuid` - Unique identifier from the Physna API (required for API operations)
/// * `name` - Display name of the folder
/// * `path` - Full path of the folder in the hierarchy (e.g., "/Root/Parent/Child")
///
/// # Examples
/// ```
/// use pcli2::model::Folder;
/// use uuid::Uuid;
///
/// let folder = Folder::new(
///     Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
///     "My Folder".to_string(),
///     "/Root/My Folder".to_string(),
///     0,  // assets count
///     0   // folders count
/// );
/// ```
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Folder {
    /// UUID of the folder from the API
    #[serde(rename = "id")]
    uuid: Uuid,
    /// Name of the folder
    name: String,
    /// Full path of the folder
    path: String,
    /// Number of assets in the folder
    #[serde(rename = "assetsCount")]
    assets_count: u32,
    /// Number of subfolders in the folder
    #[serde(rename = "foldersCount")]
    folders_count: u32,
}

impl Folder {
    /// Create a new Folder instance
    ///
    /// # Arguments
    /// * `uuid` - UUID from the API
    /// * `name` - Name of the folder
    /// * `path` - Full path of the folder
    /// * `assets_count` - Number of assets in the folder
    /// * `folders_count` - Number of subfolders in the folder
    pub fn new(
        uuid: Uuid,
        name: String,
        path: String,
        assets_count: u32,
        folders_count: u32,
    ) -> Folder {
        Folder {
            uuid,
            name,
            path,
            assets_count,
            folders_count,
        }
    }

    /// Create a Folder from a FolderResponse with a specified path
    ///
    /// # Arguments
    /// * `folder_response` - The API response containing folder data
    /// * `path` - The full path for this folder
    pub fn from_folder_response(folder_response: FolderResponse, path: String) -> Folder {
        Folder {
            uuid: folder_response.uuid,
            name: folder_response.name,
            path,
            assets_count: folder_response.assets_count,
            folders_count: folder_response.folders_count,
        }
    }

    /// Get the UUID of the folder
    pub fn uuid(&self) -> &Uuid {
        self.uuid.as_ref()
    }

    /// Get the number of assets in the folder
    pub fn assets_count(&self) -> u32 {
        self.assets_count
    }

    /// Get the number of subfolders in the folder
    pub fn folders_count(&self) -> u32 {
        self.folders_count
    }

    /// Set the name of the folder
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

    /// Set the path of the folder
    pub fn set_path(&mut self, path: String) {
        self.path = path;
    }

    /// Create a new FolderBuilder for constructing Folder instances
    pub fn builder() -> FolderBuilder {
        FolderBuilder::new()
    }
}

impl From<FolderResponse> for Folder {
    fn from(fr: FolderResponse) -> Folder {
        Folder {
            uuid: fr.uuid.clone(),
            name: fr.name.clone(),
            path: "".to_string(),
            assets_count: fr.assets_count,
            folders_count: fr.folders_count,
        }
    }
}

impl From<SingleFolderResponse> for Folder {
    fn from(fr: SingleFolderResponse) -> Folder {
        Folder {
            uuid: fr.folder.uuid.clone(),
            name: fr.folder.name.clone(),
            path: "".to_string(),
            assets_count: fr.folder.assets_count,
            folders_count: fr.folder.folders_count,
        }
    }
}

impl CsvRecordProducer for Folder {
    /// Get the CSV header row for Folder records
    fn csv_header() -> Vec<String> {
        vec![
            "NAME".to_string(),
            "PATH".to_string(),
            "ASSETS_COUNT".to_string(),
            "FOLDERS_COUNT".to_string(),
            "UUID".to_string(),
        ]
    }

    /// Convert the Folder to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.name(),
            self.path(),
            self.assets_count().to_string(),
            self.folders_count().to_string(),
            self.uuid().to_string(),
        ]]
    }

    /// Generate CSV output with a header row
    fn to_csv(&self, with_headers: bool) -> Result<String, FormattingError> {
        let mut wtr = Writer::from_writer(vec![]);

        if with_headers {
            wtr.write_record(Self::csv_header()).map_err(|e| {
                FormattingError::CsvWriterError(format!("Failed to write CSV header: {}", e))
            })?;
        }

        // Sort records by folder name
        let mut records = self.as_csv_records();
        records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)

        for record in records {
            wtr.write_record(&record).map_err(|e| {
                FormattingError::CsvWriterError(format!("Failed to write CSV record: {}", e))
            })?;
        }
        let data = wtr.into_inner().map_err(|e| {
            FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
        })?;
        String::from_utf8(data).map_err(FormattingError::Utf8Error)
    }
}

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
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

/// Builder for constructing Folder instances with a fluent API
pub struct FolderBuilder {
    /// UUID of the folder from the API
    uuid: Option<Uuid>,
    /// Name of the folder
    name: Option<String>,
    /// Full path of the folder
    path: Option<String>,
    /// Number of assets in the folder
    assets_count: Option<u32>,
    /// Number of subfolders in the folder
    folders_count: Option<u32>,
}

impl FolderBuilder {
    /// Create a new FolderBuilder
    fn new() -> FolderBuilder {
        FolderBuilder {
            uuid: None,
            name: None,
            path: None,
            assets_count: None,
            folders_count: None,
        }
    }

    /// Set the UUID of the folder
    pub fn uuid(&mut self, uuid: Uuid) -> &mut FolderBuilder {
        self.uuid = Some(uuid);
        self
    }

    /// Set the number of assets in the folder
    pub fn assets_count(&mut self, assets_count: u32) -> &mut FolderBuilder {
        self.assets_count = Some(assets_count);
        self
    }

    /// Set the number of subfolders in the folder
    pub fn folders_count(&mut self, folders_count: u32) -> &mut FolderBuilder {
        self.folders_count = Some(folders_count);
        self
    }

    /// Set the name of the folder
    pub fn name(&mut self, name: &str) -> &mut FolderBuilder {
        self.name = Some(name.to_owned());
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
        let uuid = match &self.uuid {
            Some(uuid) => uuid.clone(),
            None => {
                return Err(ModelError::MissingPropertyValue {
                    name: "uuid".to_string(),
                })
            }
        };

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

        let assets_count = self.assets_count.unwrap_or(0);
        let folders_count = self.folders_count.unwrap_or(0);

        Ok(Folder::new(uuid, name, path, assets_count, folders_count))
    }
}

/// A collection of Folder instances
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderList {
    /// Map of folder hash keys to Folder instances
    folders: Vec<Folder>,
}

impl FolderList {
    /// Create a new empty FolderList
    pub fn empty() -> FolderList {
        FolderList {
            folders: Vec::new(),
        }
    }

    pub fn new(folders: Vec<Folder>) -> Self {
        FolderList {
            folders: folders.clone(),
        }
    }

    /// Check if the FolderList is empty
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// Get the number of folders in the FolderList
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    /// Find a folder in the FolderList by name
    ///
    /// # Arguments
    /// * `name` - The name of the folder to find
    ///
    /// # Returns
    /// * `Some(&Folder)` - If a folder with the specified name exists
    /// * `None` - If no folder with the specified name exists
    pub fn find_by_name(&self, name: &String) -> Option<&Folder> {
        self.folders.iter().find(|f| f.name.eq(name))
    }

    pub fn add(&mut self, folder: Folder) {
        self.folders.push(folder)
    }

    pub fn folders(&self) -> Vec<Folder> {
        self.folders.clone()
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
            folders.add(f);
        }

        folders
    }
}

// for: `for folder in &folder_list`
impl<'a> IntoIterator for &'a FolderList {
    type Item = &'a Folder;
    type IntoIter = std::slice::Iter<'a, Folder>;
    fn into_iter(self) -> Self::IntoIter {
        self.folders.iter()
    }
}

// for: `for folder in &mut folder_list`
impl<'a> IntoIterator for &'a mut FolderList {
    type Item = &'a mut Folder;
    type IntoIter = std::slice::IterMut<'a, Folder>;
    fn into_iter(self) -> Self::IntoIter {
        self.folders.iter_mut()
    }
}

// for: `for folder in folder_list` (consumes the list)
impl IntoIterator for FolderList {
    type Item = Folder;
    type IntoIter = std::vec::IntoIter<Folder>;
    fn into_iter(self) -> Self::IntoIter {
        self.folders.into_iter()
    }
}

impl From<FolderListResponse> for FolderList {
    fn from(response: FolderListResponse) -> Self {
        let folders: Vec<Folder> = response.folders.into_iter().map(|f| f.into()).collect();
        FolderList::new(folders)
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

        for folder in self.folders.iter() {
            records.push(folder.as_csv_records()[0].clone());
        }

        records
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
            OutputFormat::Json(options) => {
                // convert to a simple vector for output, sorted by name
                let mut folders: Vec<Folder> = self.folders.iter().cloned().collect();
                folders.sort_by_key(|a| a.name());
                let json = if options.pretty {
                    serde_json::to_string_pretty(&folders)
                } else {
                    serde_json::to_string(&folders)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.write_record(Self::csv_header())?;
                }

                // Sort records by folder name
                let mut records = self.as_csv_records();
                records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)

                for record in records {
                    wtr.write_record(&record).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV record: {}",
                            e
                        ))
                    })?;
                }
                let data = wtr.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
                })?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(_) => {
                // For folder list, tree format is the same as JSON
                // In practice, tree format should be handled at the command level
                // where we have access to the full hierarchy
                let mut folders: Vec<Folder> = self.folders.iter().cloned().collect();
                folders.sort_by_key(|a| a.name());
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

// Represents a tenant configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tenant {
    #[serde(rename = "id")]
    pub uuid: Uuid,
    pub name: String,
    #[serde(rename = "description")]
    pub description: String,
}

impl TryFrom<&TenantSetting> for Tenant {
    type Error = uuid::Error;

    fn try_from(tenant_setting: &TenantSetting) -> Result<Self, Self::Error> {
        Ok(Tenant {
            uuid: tenant_setting.tenant_uuid.to_owned(),
            name: tenant_setting.tenant_short_name.to_owned(),
            description: tenant_setting.tenant_display_name.to_owned(),
        })
    }
}

impl crate::format::Formattable for Tenant {
    fn format(
        &self,
        f: &crate::format::OutputFormat,
    ) -> Result<String, crate::format::FormattingError> {
        match f {
            crate::format::OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => {
                        Err(crate::format::FormattingError::FormatFailure { cause: Box::new(e) })
                    }
                }
            }
            crate::format::OutputFormat::Csv(options) => {
                // For CSV format, output header with tenant name, UUID, and description columns only if with_headers is true
                let mut wtr = csv::Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.serialize(("TENANT_NAME", "TENANT_UUID", "TENANT_DESCRIPTION"))?;
                }

                wtr.serialize((&self.name, &self.uuid.to_string(), &self.description))?;

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(crate::format::FormattingError::Utf8Error)
            }
            crate::format::OutputFormat::Tree(_) => {
                // For tree format, include name, UUID, and description
                Ok(format!(
                    "{} ({}) - {}",
                    self.name, self.uuid, self.description
                ))
            }
        }
    }
}

/// A collection of Tenant instances
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantList {
    /// Vector of Tenant instances
    tenants: Vec<Tenant>,
}

impl TenantList {
    /// Create a new empty TenantList
    pub fn empty() -> TenantList {
        TenantList {
            tenants: Vec::new(),
        }
    }

    pub fn new(tenants: Vec<Tenant>) -> Self {
        TenantList {
            tenants: tenants.clone(),
        }
    }

    /// Check if the TenantList is empty
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }

    /// Get the number of tenants in the TenantList
    pub fn len(&self) -> usize {
        self.tenants.len()
    }

    /// Find a tenant in the TenantList by name
    ///
    /// # Arguments
    /// * `name` - The name of the tenant to find
    ///
    /// # Returns
    /// * `Some(&Tenant)` - If a tenant with the specified name exists
    /// * `None` - If no tenant with the specified name exists
    pub fn find_by_name(&self, name: &String) -> Option<&Tenant> {
        self.tenants.iter().find(|t| t.name.eq(name))
    }

    pub fn add(&mut self, tenant: Tenant) {
        self.tenants.push(tenant)
    }

    pub fn tenants(&self) -> Vec<Tenant> {
        self.tenants.clone()
    }
}

impl Default for TenantList {
    fn default() -> Self {
        Self::empty()
    }
}

impl FromIterator<Tenant> for TenantList {
    /// Create a TenantList from an iterator of Tenant instances
    fn from_iter<I: IntoIterator<Item = Tenant>>(iter: I) -> TenantList {
        let mut tenants = TenantList::empty();
        for tenant in iter {
            tenants.add(tenant);
        }
        tenants
    }
}

impl<'a> IntoIterator for &'a TenantList {
    type Item = &'a Tenant;
    type IntoIter = std::slice::Iter<'a, Tenant>;

    /// Convert the TenantList to an iterator
    fn into_iter(self) -> std::slice::Iter<'a, Tenant> {
        self.tenants.iter()
    }
}

impl<'a> IntoIterator for &'a mut TenantList {
    type Item = &'a mut Tenant;
    type IntoIter = std::slice::IterMut<'a, Tenant>;

    /// Convert the TenantList to a mutable iterator
    fn into_iter(self) -> std::slice::IterMut<'a, Tenant> {
        self.tenants.iter_mut()
    }
}

impl IntoIterator for TenantList {
    type Item = Tenant;
    type IntoIter = std::vec::IntoIter<Tenant>;

    /// Convert the TenantList to an owned iterator
    fn into_iter(self) -> std::vec::IntoIter<Tenant> {
        self.tenants.into_iter()
    }
}

impl From<Vec<TenantSetting>> for TenantList {
    fn from(settings: Vec<TenantSetting>) -> Self {
        let tenants = settings
            .into_iter()
            .map(|ts| Tenant {
                uuid: ts.tenant_uuid,
                name: ts.tenant_short_name,
                description: ts.tenant_display_name,
            })
            .collect();
        TenantList::new(tenants)
    }
}

impl CsvRecordProducer for TenantList {
    /// Get the CSV header row for TenantList records
    fn csv_header() -> Vec<String> {
        vec![
            "TENANT_NAME".to_string(),
            "TENANT_UUID".to_string(),
            "TENANT_DESCRIPTION".to_string(),
        ]
    }

    /// Convert the TenantList to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.tenants
            .iter()
            .map(|tenant| {
                vec![
                    tenant.name.clone(),
                    tenant.uuid.to_string(),
                    tenant.description.clone(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for TenantList {
    type Item = TenantList;

    /// Format the TenantList according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV, or Tree)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                // convert to a simple vector for output, sorted by name
                let mut tenants: Vec<Tenant> = self.tenants.iter().cloned().collect();
                tenants.sort_by_key(|a| a.name.clone());
                let json = if options.pretty {
                    serde_json::to_string_pretty(&tenants)
                } else {
                    serde_json::to_string(&tenants)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.write_record(Self::csv_header())?;
                }

                // Sort records by tenant name
                let mut records = self.as_csv_records();
                records.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by NAME column (index 0)

                for record in records {
                    wtr.write_record(&record).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV record: {}",
                            e
                        ))
                    })?;
                }
                let data = wtr.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
                })?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            OutputFormat::Tree(_) => {
                // For tree format, include name, UUID, and description
                let mut output = String::new();
                for tenant in &self.tenants {
                    output.push_str(&format!(
                        "{} ({}) - {}\n",
                        tenant.name, tenant.uuid, tenant.description
                    ));
                }
                Ok(output)
            }
        }
    }
}

/// Represents a tenant setting for a user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantSetting {
    /// The ID of the tenant
    #[serde(rename = "tenantId")]
    pub tenant_uuid: Uuid,
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
    /// * `uuid` - The UUID of the tenant to find
    ///
    /// # Returns
    /// * `Some(&TenantSetting)` - If a tenant with the specified UUID exists
    /// * `None` - If no tenant with the specified ID exists
    pub fn get_tenant_by_uuid(&self, uuid: &Uuid) -> Option<&TenantSetting> {
        self.user
            .settings
            .iter()
            .find(|setting| setting.tenant_uuid.eq(uuid))
    }
}

// Folder models for Physna V3 API

/// Represents a folder response from the Physna V3 API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderResponse {
    /// The ID of the folder
    #[serde(rename = "id")]
    pub uuid: Uuid,
    /// The ID of the tenant that owns the folder
    #[serde(rename = "tenantId")]
    pub tenant_uuid: Uuid,
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
    pub parent_folder_uuid: Option<Uuid>,
    /// The ID of the owner, if any
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

impl FolderResponse {
    pub fn new(name: &str) -> Self {
        FolderResponse {
            uuid: Uuid::new_v4(),
            tenant_uuid: Uuid::new_v4(),
            name: name.to_string(),
            created_at: String::default(),
            updated_at: String::default(),
            assets_count: 0,
            folders_count: 0,
            parent_folder_uuid: None,
            owner_id: None,
        }
    }
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
    #[serde(rename = "startIndex")]
    pub start_index: usize,
    /// End index of items on this page
    #[serde(rename = "endIndex")]
    pub end_index: usize,
}

// Asset models for Physna V3 API

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    meta: HashMap<String, String>,
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

impl From<HashMap<String, serde_json::Value>> for AssetMetadata {
    fn from(ht: HashMap<String, serde_json::Value>) -> Self {
        let meta: HashMap<String, String> = ht
            .iter()
            .map(|(k, v)| {
                let value_string = if let Some(str_val) = v.as_str() {
                    str_val.to_string()
                } else {
                    v.to_string() // fallback to generic string representation for non-string values
                };
                (k.to_owned(), value_string)
            })
            .collect();

        Self::from(meta)
    }
}

impl OutputFormatter for AssetMetadata {
    type Item = AssetList;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = Writer::from_writer(vec![]);
                if options.with_headers {
                    wtr.write_record(vec!["NAME", "VALUE"])?;
                }

                // Sort records by folder name
                let mut records: Vec<Vec<String>> = self
                    .meta
                    .iter()
                    .map(|(k, v)| vec![k.to_owned(), v.to_owned()])
                    .collect();
                records.sort_by(|a, b| a[0].cmp(&b[0]));

                for record in records {
                    wtr.write_record(&record).map_err(|e| {
                        FormattingError::CsvWriterError(format!(
                            "Failed to write CSV record: {}",
                            e
                        ))
                    })?;
                }
                let data = wtr.into_inner().map_err(|e| {
                    FormattingError::CsvWriterError(format!("Failed to finalize CSV: {}", e))
                })?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
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
/// * `path` - Full path of the asset in the folder hierarchy (e.g., "/Root/Folder/file.stl")
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
        Some(self.name.cmp(&other.name))
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
}

impl From<&AssetResponse> for Asset {
    fn from(asset_response: &AssetResponse) -> Self {
        // Extract the name from the path (last part after the last slash)
        let name = asset_response
            .path
            .split('/')
            .next_back()
            .unwrap_or(&asset_response.path)
            .to_string();

        Asset::new(
            asset_response.uuid.to_owned(),
            name,
            asset_response.path.clone(),
            None, // file_size not in current API response
            Some(asset_response.asset_type.clone()),
            Some(asset_response.state.clone()),
            Some(asset_response.created_at.clone()),
            Some(asset_response.updated_at.clone()),
            Some(asset_response.metadata.clone().into()),
        )
    }
}

impl From<AssetResponse> for Asset {
    fn from(asset_response: AssetResponse) -> Self {
        <Asset as From<&AssetResponse>>::from(&asset_response)
    }
}

impl CsvRecordProducer for Asset {
    /// Get the CSV header row for Asset records
    fn csv_header() -> Vec<String> {
        vec![
            "NAME".to_string(),
            "PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "UUID".to_string(),
        ]
    }

    /// Convert the Asset to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.name(),
            self.path(),
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default(),
            self.uuid().to_string(),
        ]]
    }

    /// Get the extended CSV header row for Asset records including metadata
    fn csv_header_with_metadata() -> Vec<String> {
        let header = Self::csv_header();
        // We'll add metadata columns dynamically when we know what metadata keys exist
        header
    }

    /// Convert the Asset to CSV records including metadata
    fn as_csv_records_with_metadata(&self) -> Vec<Vec<String>> {
        let record = vec![
            self.name(),
            self.path(),
            self.file_type().cloned().unwrap_or_default(),
            self.processing_status().cloned().unwrap_or_default(),
            self.uuid().to_string(),
        ];

        // Add metadata values if they exist
        if let Some(_metadata) = self.metadata() {
            // We'll need to collect all unique metadata keys when building the CSV
            // For now, we just return the basic record without metadata columns
            // The metadata will be added when building the full CSV with dynamic columns
        }

        vec![record]
    }
}

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
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
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

    /// Get all assets as a vector
    ///
    /// # Returns
    /// A vector containing all assets in the AssetList
    pub fn get_all_assets(&self) -> Vec<&Asset> {
        self.assets.values().collect()
    }

    /// Convert the AssetList to CSV records including metadata
    ///
    /// This method converts the AssetList to CSV records with additional metadata columns.
    ///
    /// # Arguments
    /// * `metadata_keys` - Sorted list of metadata keys to include as columns
    ///
    /// # Returns
    /// Vector of CSV records with metadata columns
    fn as_csv_records_with_metadata(&self, metadata_keys: &[String]) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for asset in self.assets.values() {
            // Start with standard asset record
            let mut record = vec![
                asset.name(),
                asset.path(),
                asset.file_type().cloned().unwrap_or_default(),
                asset.processing_status().cloned().unwrap_or_default(),
                asset.uuid().to_string(),
            ];

            // Add metadata values for each key
            if let Some(metadata) = asset.metadata() {
                for key in metadata_keys {
                    let value = metadata.get(key).cloned().unwrap_or_default();
                    record.push(value);
                }
            } else {
                // No metadata, add empty strings for all metadata columns
                for _ in metadata_keys {
                    record.push(String::new());
                }
            }

            records.push(record);
        }

        records
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
        AssetList::from_iter(assets.into_iter())
    }
}

impl From<AssetListResponse> for AssetList {
    fn from(response: AssetListResponse) -> Self {
        response.to_asset_list()
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

        for asset in self.assets.values() {
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
            OutputFormat::Json(options) => {
                // convert to a simple vector for output and sort by path
                let mut assets: Vec<Asset> = self.assets.values().cloned().collect();
                assets.sort_by_key(|a| a.path());
                let json = if options.pretty {
                    serde_json::to_string_pretty(&assets)
                } else {
                    serde_json::to_string(&assets)
                };

                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                if options.with_metadata {
                    // For CSV with metadata, we need to collect all unique metadata keys first
                    let mut metadata_keys: std::collections::HashSet<String> =
                        std::collections::HashSet::new();

                    // Collect all unique metadata keys
                    for asset in self.assets.values() {
                        if let Some(metadata) = asset.metadata() {
                            for key in metadata.keys() {
                                metadata_keys.insert(key.clone());
                            }
                        }
                    }

                    // Convert to sorted vector for consistent column ordering
                    let mut sorted_metadata_keys: Vec<String> = metadata_keys.into_iter().collect();
                    sorted_metadata_keys.sort();

                    // Build CSV with metadata columns
                    let buf = BufWriter::new(Vec::new());
                    let mut wtr = Writer::from_writer(buf);

                    if options.with_headers {
                        // Build header with metadata columns
                        let mut header = Asset::csv_header();
                        for key in &sorted_metadata_keys {
                            header.push(key.clone());
                        }
                        wtr.write_record(&header).unwrap();
                    }

                    // Sort records by asset path
                    let mut records = self.as_csv_records_with_metadata(&sorted_metadata_keys);
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
                } else {
                    let buf = BufWriter::new(Vec::new());
                    let mut wtr = Writer::from_writer(buf);

                    if options.with_headers {
                        wtr.write_record(Self::csv_header()).unwrap();
                    }

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
            }
            OutputFormat::Tree(_) => {
                // For asset list, tree format is the same as JSON
                // In practice, tree format should be handled at the command level
                // where we have access to the full hierarchy
                // convert to a simple vector for output and sort by path
                let mut assets: Vec<Asset> = self.assets.values().cloned().collect();
                assets.sort_by_key(|a| a.path());
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
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl GeometricMatch {
    /// Get the asset ID
    pub fn asset_uuid(&self) -> &Uuid {
        &self.asset.uuid
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

/// Represents a part match with forward and reverse similarity percentages
///
/// This structure holds information about a single part match, including the
/// matching asset and both forward and reverse match percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartMatch {
    /// The matching asset details
    pub asset: AssetResponse,
    /// The forward match percentage
    #[serde(rename = "forwardMatchPercentage")]
    pub forward_match_percentage: Option<f64>,
    /// The reverse match percentage
    #[serde(rename = "reverseMatchPercentage")]
    pub reverse_match_percentage: Option<f64>,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl PartMatch {
    /// Get the asset ID
    pub fn asset_uuid(&self) -> &Uuid {
        &self.asset.uuid
    }

    /// Get the asset path
    pub fn path(&self) -> &str {
        &self.asset.path
    }

    /// Get the forward match percentage (0.0 to 100.0)
    pub fn forward_score(&self) -> f64 {
        self.forward_match_percentage.unwrap_or(0.0)
    }

    /// Get the reverse match percentage (0.0 to 100.0)
    pub fn reverse_score(&self) -> f64 {
        self.reverse_match_percentage.unwrap_or(0.0)
    }
}

/// Response structure for part search operations
///
/// This structure holds the results of a part search operation, including
/// the list of matching assets and pagination/filter information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartSearchResponse {
    /// The list of matching assets
    pub matches: Vec<PartMatch>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
    /// Filter information
    #[serde(rename = "filterData")]
    pub filter_data: Option<FilterData>,
}

impl CsvRecordProducer for PartSearchResponse {
    /// Get the CSV header row for PartSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_ID".to_string(),
            "PATH".to_string(),
            "FORWARD_SCORE".to_string(),
            "REVERSE_SCORE".to_string(),
        ]
    }

    /// Convert the PartSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    m.asset_uuid().to_string(),
                    m.path().to_string(),
                    format!("{}", m.forward_score()), // Full precision
                    format!("{}", m.reverse_score()), // Full precision
                ]
            })
            .collect()
    }
}

/// Enhanced response structure for part search that includes reference asset information
///
/// This structure extends the basic PartSearchResponse by including information about
/// the reference asset that was searched against, making it easier to understand
/// the context of the matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedPartSearchResponse {
    /// The reference asset that was searched against
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The list of matching assets
    pub matches: Vec<PartMatch>,
}

impl CsvRecordProducer for EnhancedPartSearchResponse {
    /// Get the CSV header row for EnhancedPartSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "FORWARD_MATCH_PERCENTAGE".to_string(),
            "REVERSE_MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the EnhancedPartSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    self.reference_asset.path.clone(),
                    m.path().to_string(),
                    format!("{}", m.forward_score()), // Full precision
                    format!("{}", m.reverse_score()), // Full precision
                    self.reference_asset.uuid.to_string(),
                    m.asset_uuid().to_string(),
                    m.comparison_url.clone().unwrap_or_default(),
                ]
            })
            .collect()
    }
}

impl OutputFormatter for EnhancedPartSearchResponse {
    type Item = EnhancedPartSearchResponse;

    /// Format the EnhancedPartSearchResponse according to the specified output format
    ///
    /// This method formats the EnhancedPartSearchResponse based on the requested format:
    /// - JSON: Outputs as JSON with optional pretty printing
    /// - CSV: Outputs as CSV with optional headers
    /// - Tree: Not supported for this type
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        // Extract the metadata flag from the format options
        let with_metadata = match &f {
            OutputFormat::Json(options) => options.with_metadata,
            OutputFormat::Csv(options) => options.with_metadata,
            OutputFormat::Tree(options) => options.with_metadata,
        };

        self.format_with_metadata_flag(f, with_metadata)
    }
}

impl EnhancedPartSearchResponse {
    /// Format the EnhancedPartSearchResponse with consideration for metadata flag
    pub fn format_with_metadata_flag(
        &self,
        f: OutputFormat,
        include_metadata: bool,
    ) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                // Pre-calculate the metadata keys that will be used for both header and all records to ensure consistency
                let mut header_metadata_keys = Vec::new();
                if include_metadata {
                    // Collect all unique metadata keys from ALL matches for consistent headers
                    let mut all_metadata_keys = std::collections::HashSet::new();

                    // Collect metadata keys from reference asset
                    for key in self.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Collect metadata keys from all matching assets
                    for match_result in &self.matches {
                        for key in match_result.asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();
                    header_metadata_keys = sorted_keys;
                }

                if options.with_headers {
                    if include_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = Self::csv_header();

                        // Extend headers with metadata columns using the pre-calculated keys
                        for key in &header_metadata_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(EnhancedPartSearchResponse::csv_header())?;
                    }
                }

                for match_result in &self.matches {
                    if include_metadata {
                        // Include metadata values in the output
                        let mut base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.forward_score()),
                            format!("{}", match_result.reverse_score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];

                        // Add metadata values for each key that was included in the header
                        for key in &header_metadata_keys {
                            // Add reference asset metadata value (same for all records)
                            let ref_value = self
                                .reference_asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(ref_value);

                            // Add candidate asset metadata value (specific to this match)
                            let cand_value = match_result
                                .asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(cand_value);
                        }

                        wtr.serialize(base_values.as_slice())?;
                    } else {
                        let base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.forward_score()),
                            format!("{}", match_result.reverse_score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];
                        wtr.serialize(base_values.as_slice())?;
                    }
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

/// Represents a matching pair from part search with both reference and candidate assets
///
/// This structure holds information about a single part match, including both the
/// reference asset (the one being searched) and the candidate asset (the matching one),
/// along with both forward and reverse match percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartMatchPair {
    /// The reference asset that was searched against
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The matching candidate asset
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The forward match percentage
    #[serde(rename = "forwardMatchPercentage")]
    pub forward_match_percentage: Option<f64>,
    /// The reverse match percentage
    #[serde(rename = "reverseMatchPercentage")]
    pub reverse_match_percentage: Option<f64>,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

/// Represents a pair of assets that matched in a visual search
/// This structure excludes match percentages since visual search doesn't provide them
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualMatchPair {
    /// The reference asset (the one being compared against)
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The candidate asset (the one that matched)
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl PartMatchPair {
    /// Create a new PartMatchPair from a reference asset and a part match
    pub fn from_reference_and_match(
        reference_asset: AssetResponse,
        match_result: PartMatch,
    ) -> Self {
        PartMatchPair {
            reference_asset,
            candidate_asset: match_result.asset,
            forward_match_percentage: match_result.forward_match_percentage,
            reverse_match_percentage: match_result.reverse_match_percentage,
            transformation: match_result.transformation,
            comparison_url: match_result.comparison_url,
        }
    }
}

impl CsvRecordProducer for PartMatchPair {
    /// Get the CSV header row for PartMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "SCORE_1".to_string(), // Generic score field that can be forward match % for geometric/part or empty for visual
            "SCORE_2".to_string(), // Generic score field that can be reverse match % for geometric/part or empty for visual
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the PartMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset.path.clone(),
            self.candidate_asset.path.clone(),
            format!("{}", self.forward_match_percentage.unwrap_or(0.0)),
            format!("{}", self.reverse_match_percentage.unwrap_or(0.0)),
            self.reference_asset.uuid.to_string(),
            self.candidate_asset.uuid.to_string(),
            self.comparison_url.clone().unwrap_or_default(),
        ]]
    }
}

impl CsvRecordProducer for VisualMatchPair {
    /// Get the CSV header row for VisualMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the VisualMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset.path.clone(),
            self.candidate_asset.path.clone(),
            self.reference_asset.uuid.to_string(),
            self.candidate_asset.uuid.to_string(),
            self.comparison_url.clone().unwrap_or_default(),
        ]]
    }
}

/// Represents a 4x4 transformation matrix
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformationMatrix {
    /// The 4x4 matrix values in row-major order
    pub matrix: [f64; 16],
}

/// Represents filter data in API responses
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterData {
    /// Extensions filter information
    pub extensions: Vec<FilterCount>,
    /// Folders filter information
    pub folders: Vec<FilterCount>,
    /// Metadata filter information
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Represents a filter and its count
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterCount {
    /// The filter value
    pub filter: String,
    /// The count of items matching this filter
    pub count: u32,
}

/// Represents a matching pair from folder-based geometric matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderGeometricMatch {
    /// Name of the reference asset (the one from the source folder)
    #[serde(rename = "referenceAssetName")]
    pub reference_asset_name: String,
    /// Name of the candidate asset (the one that matched)
    #[serde(rename = "candidateAssetName")]
    pub candidate_asset_name: String,
    /// Match percentage between the assets
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// Full path of the reference asset
    #[serde(rename = "referenceAssetPath")]
    pub reference_asset_path: String,
    /// Full path of the candidate asset
    #[serde(rename = "candidateAssetPath")]
    pub candidate_asset_path: String,
    /// UUID of the reference asset
    #[serde(rename = "referenceAssetUuid")]
    pub reference_asset_uuid: Uuid,
    /// UUID of the candidate asset
    #[serde(rename = "candidateAssetUuid")]
    pub candidate_asset_uuid: Uuid,
    /// URL to the Physna comparison viewer for these two assets
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: String,
}

impl CsvRecordProducer for FolderGeometricMatch {
    /// Get the CSV header row for FolderGeometricMatch records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_NAME".to_string(),
            "CANDIDATE_ASSET_NAME".to_string(),
            "MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the FolderGeometricMatch to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset_name.clone(),
            self.candidate_asset_name.clone(),
            format!("{:.2}", self.match_percentage),
            self.reference_asset_path.clone(),
            self.candidate_asset_path.clone(),
            self.reference_asset_uuid.to_string(),
            self.candidate_asset_uuid.to_string(),
            self.comparison_url.clone(),
        ]]
    }
}

/// Represents the response from folder-based geometric matching
pub type FolderGeometricMatchResponse = Vec<FolderGeometricMatch>;

// For FolderGeometricMatchResponse (Vec<FolderGeometricMatch>), we need to implement the traits manually
impl CsvRecordProducer for FolderGeometricMatchResponse {
    /// Get the CSV header row for FolderGeometricMatchResponse records
    fn csv_header() -> Vec<String> {
        FolderGeometricMatch::csv_header()
    }

    /// Convert the FolderGeometricMatchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.iter().flat_map(|m| m.as_csv_records()).collect()
    }
}

impl OutputFormatter for FolderGeometricMatchResponse {
    type Item = FolderGeometricMatchResponse;

    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => Ok(self.to_csv(options.with_headers)?),
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

/// Represents the response from the geometric search API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricSearchResponse {
    /// The list of matching assets
    pub matches: Vec<GeometricMatch>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
    /// Filter information
    #[serde(rename = "filterData")]
    pub filter_data: Option<FilterData>,
}

impl CsvRecordProducer for GeometricSearchResponse {
    /// Get the CSV header row for GeometricSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_ID".to_string(),
            "PATH".to_string(),
            "SCORE".to_string(),
        ]
    }

    /// Convert the GeometricSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    m.asset_uuid().to_string(),
                    m.path().to_string(),
                    format!("{}", m.score()), // Full precision
                ]
            })
            .collect()
    }
}

/// Structure to represent a geometric match with both reference and candidate assets.
///
/// This structure holds information about a single geometric match, including both the
/// reference asset (the one being searched) and the candidate asset (the matching one),
/// along with the similarity percentage and transformation matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricMatchPair {
    /// The reference asset that was searched against
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The matching candidate asset
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The similarity percentage
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl GeometricMatchPair {
    /// Create a new GeometricMatchPair from a reference asset and a geometric match
    pub fn from_reference_and_match(
        reference_asset: AssetResponse,
        match_result: GeometricMatch,
    ) -> Self {
        GeometricMatchPair {
            reference_asset,
            candidate_asset: match_result.asset,
            match_percentage: match_result.match_percentage,
            transformation: match_result.transformation,
            comparison_url: match_result.comparison_url,
        }
    }
}

impl CsvRecordProducer for GeometricMatchPair {
    /// Get the CSV header row for GeometricMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the GeometricMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.reference_asset.path.clone(),
            self.candidate_asset.path.clone(),
            format!("{}", self.match_percentage), // Full precision
            self.reference_asset.uuid.to_string(),
            self.candidate_asset.uuid.to_string(),
            self.comparison_url.clone().unwrap_or_default(),
        ]]
    }
}

impl OutputFormatter for GeometricMatchPair {
    type Item = GeometricMatchPair;

    /// Format the GeometricMatchPair according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = GeometricMatchPair::csv_header();

                        // Get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from candidate asset
                        for key in self.candidate_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(GeometricMatchPair::csv_header())?;
                    }
                }

                if options.with_metadata {
                    // Include metadata values in the output
                    let mut base_values = vec![
                        self.reference_asset.path.clone(),
                        self.candidate_asset.path.clone(),
                        format!("{}", self.match_percentage),
                        self.reference_asset.uuid.to_string(),
                        self.candidate_asset.uuid.to_string(),
                        self.comparison_url.clone().unwrap_or_default(),
                    ];

                    // Get unique metadata keys from both reference and candidate assets
                    let mut all_metadata_keys = std::collections::HashSet::new();

                    // Collect metadata keys from reference asset
                    for key in self.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Collect metadata keys from candidate asset
                    for key in self.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Add metadata values for each key
                    for key in &sorted_keys {
                        // Add reference asset metadata value
                        let ref_value = self
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = self
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }

                    wtr.serialize(base_values.as_slice())?;
                } else {
                    wtr.serialize((
                        &self.reference_asset.path,
                        &self.candidate_asset.path,
                        &self.match_percentage,
                        &self.reference_asset.uuid.to_string(),
                        &self.candidate_asset.uuid.to_string(),
                        &self.comparison_url.clone().unwrap_or_default(),
                    ))?;
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

/// Enhanced response structure for geometric match that includes reference asset information.
///
/// This structure extends the basic geometric search response to include information about
/// the reference asset that was searched against, providing complete context for the matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedGeometricSearchResponse {
    /// The reference asset that was searched against
    pub reference_asset: AssetResponse,
    /// The list of matching assets
    pub matches: Vec<GeometricMatch>,
}

impl CsvRecordProducer for EnhancedGeometricSearchResponse {
    /// Get the CSV header row for EnhancedGeometricSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "REFERENCE_ASSET_PATH".to_string(),
            "CANDIDATE_ASSET_PATH".to_string(),
            "MATCH_PERCENTAGE".to_string(),
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "COMPARISON_URL".to_string(),
        ]
    }

    /// Convert the EnhancedGeometricSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .map(|m| {
                vec![
                    self.reference_asset.path.clone(),     // Reference asset path
                    m.path().to_string(),                  // Candidate asset path
                    format!("{}", m.score()),              // Full precision match percentage
                    self.reference_asset.uuid.to_string(), // Reference asset UUID
                    m.asset_uuid().to_string(),            // Candidate asset UUID
                    m.comparison_url.clone().unwrap_or_default(), // Comparison URL
                ]
            })
            .collect()
    }
}

impl OutputFormatter for EnhancedGeometricSearchResponse {
    type Item = EnhancedGeometricSearchResponse;

    /// Format the EnhancedGeometricSearchResponse according to the specified output format
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = Self::csv_header();
                        // Add metadata columns - we need to get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets
                        for match_result in &self.matches {
                            for key in match_result.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(EnhancedGeometricSearchResponse::csv_header())?;
                    }
                }

                for match_result in &self.matches {
                    if options.with_metadata {
                        // Include metadata values in the output
                        let mut base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];

                        // Get ALL unique metadata keys that were used in the header
                        // (collected from reference asset and ALL match assets)
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets (same as header generation)
                        for match_result_iter in &self.matches {
                            for key in match_result_iter.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Add metadata values for each key that was included in the header
                        for key in &sorted_keys {
                            // Add reference asset metadata value (same for all records)
                            let ref_value = self
                                .reference_asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(ref_value);

                            // Add candidate asset metadata value (specific to this match)
                            let cand_value = match_result
                                .asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(cand_value);
                        }

                        wtr.serialize(base_values.as_slice())?;
                    } else {
                        wtr.serialize((
                            &self.reference_asset.path,
                            &match_result.path(),
                            &match_result.score(),
                            &self.reference_asset.uuid.to_string(),
                            &match_result.asset_uuid().to_string(),
                            &match_result.comparison_url.clone().unwrap_or_default(),
                        ))?;
                    }
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

impl EnhancedGeometricSearchResponse {
    /// Format the EnhancedGeometricSearchResponse with consideration for metadata flag
    ///
    /// # Arguments
    /// * `format` - The output format to use (JSON, CSV)
    ///
    /// # Returns
    /// * `Ok(String)` - The formatted output
    /// * `Err(FormattingError)` - If formatting fails
    pub fn format_with_metadata_option(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = Self::csv_header();
                        // Add metadata columns - we need to get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets
                        for match_result in &self.matches {
                            for key in match_result.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(EnhancedGeometricSearchResponse::csv_header())?;
                    }
                }

                for match_result in &self.matches {
                    if options.with_metadata {
                        // Include metadata values in the output
                        let mut base_values = vec![
                            self.reference_asset.path.clone(),
                            match_result.path().to_string(),
                            format!("{}", match_result.score()),
                            self.reference_asset.uuid.to_string(),
                            match_result.asset_uuid().to_string(),
                            match_result.comparison_url.clone().unwrap_or_default(),
                        ];

                        // Get ALL unique metadata keys that were used in the header
                        // (collected from reference asset and ALL match assets)
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from all matching assets (same as header generation)
                        for match_result_iter in &self.matches {
                            for key in match_result_iter.asset.metadata.keys() {
                                all_metadata_keys.insert(key.clone());
                            }
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Add metadata values for each key that was included in the header
                        for key in &sorted_keys {
                            // Add reference asset metadata value (same for all records)
                            let ref_value = self
                                .reference_asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(ref_value);

                            // Add candidate asset metadata value (specific to this match)
                            let cand_value = match_result
                                .asset
                                .metadata
                                .get(key)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            base_values.push(cand_value);
                        }

                        wtr.serialize(base_values.as_slice())?;
                    } else {
                        wtr.serialize((
                            &self.reference_asset.path,
                            &match_result.path(),
                            &match_result.score(),
                            &self.reference_asset.uuid.to_string(),
                            &match_result.asset_uuid().to_string(),
                            &match_result.comparison_url.clone().unwrap_or_default(),
                        ))?;
                    }
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

// Metadata field models for Physna V3 API

/// Represents a metadata field definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataField {
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
}

/// Represents a dependency relationship for an asset
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
}

/// Represents the response from the asset dependencies API endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDependenciesResponse {
    /// List of assets that depend on this asset
    pub dependencies: Vec<AssetDependency>,
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
        Self {
            path: response.original_asset_path,
            dependencies: response.dependencies,
        }
    }
}

impl CsvRecordProducer for AssetDependencyList {
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_PATH".to_string(),
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
                        (asset.uuid.to_string(), filename, normalized_state)
                    }
                    None => {
                        // For missing dependencies, use the path as the name and mark as missing
                        let filename = dep.path
                            .split('/')
                            .next_back()
                            .unwrap_or(&dep.path)
                            .to_string();
                        ("N/A".to_string(), filename, "missing".to_string()) // For missing dependencies
                    }
                    }
                };

                vec![
                    self.path.clone(), // ASSET_PATH (the original asset)
                    dep.path.clone(), // DEPENDENCY_PATH (the dependency path)
                    asset_uuid, // ASSET_UUID
                    asset_filename, // ASSET_NAME
                    asset_state, // ASSET_STATE (added as requested)
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

                let simplified_deps: Vec<SimplifiedAssetDependency> = self.dependencies
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
                                (Some(asset.uuid.to_string()), Some(name), Some(normalized_state))
                            }
                            None => {
                                // For missing dependencies, use the path as the name and mark as missing
                                let name = dep.path
                                    .split('/')
                                    .next_back()
                                    .unwrap_or(&dep.path)
                                    .to_string();
                                (None, Some(name), Some("missing".to_string())) // Mark missing dependencies with "missing" state
                            }
                        };

                        SimplifiedAssetDependency {
                            path: dep.path.clone(),
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

                for record in self.as_csv_records() {
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
                        .to_string()
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
                        },
                        None => {
                            // If asset details are not available, use the path directly and mark as missing
                            let name = dep.path
                                .split('/')
                                .next_back()
                                .unwrap_or(&dep.path)
                                .to_string();
                            (name, "missing".to_string())
                        }
                    };

                    let node_label = format!("{} [{}] ({} occurrences)", asset_name, asset_state, dep.occurrences);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyNode {
    asset: Asset,
    children: Option<Box<Vec<AssemblyNode>>>,
}

impl AssemblyNode {
    pub fn new(asset: Asset) -> Self {
        Self {
            asset,
            children: None,
        }
    }

    pub fn asset(&self) -> &Asset {
        &self.asset
    }

    pub fn add_child_mut(&mut self, asset: Asset) -> &mut AssemblyNode {
        let children = self.children.get_or_insert_with(|| Box::new(Vec::new()));
        children.push(AssemblyNode::new(asset));
        children.last_mut().expect("just pushed")
    }

    pub fn has_children(&self) -> bool {
        self.children.is_some()
    }

    pub fn children(&self) -> std::slice::Iter<'_, AssemblyNode> {
        self.children
            .as_deref() // Option<&Vec<AssemblyNode>>
            .map(|v| v.iter()) // Option<Iter<AssemblyNode>>
            .unwrap_or_else(|| [].iter())
    }

    pub fn children_len(&self) -> usize {
        self.children.as_deref().map_or(0, |v| v.len())
    }
}

impl From<Asset> for AssemblyNode {
    fn from(asset: Asset) -> Self {
        Self::new(asset)
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
        use ptree::TreeBuilder;

        fn build_ptree_recursive(builder: &mut TreeBuilder, node: &AssemblyNode) {
            for child in node.children() {
                let child_label = format!("{} ({})", child.asset().name(), child.asset().uuid());
                builder.begin_child(child_label);

                // Recursively add grandchildren
                build_ptree_recursive(builder, child);

                builder.end_child();
            }
        }

        let mut builder = TreeBuilder::new(format!("{} ({})", self.asset().name(), self.asset().uuid()));

        // Add all direct children of the root node
        build_ptree_recursive(&mut builder, self);

        let tree = builder.build();

        let mut buffer = Vec::new();
        ptree::write_tree(&tree, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap_or_default()
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

/// Represents asset state counts for a tenant
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct AssetStateCounts {
    /// Number of assets currently indexing/processing
    #[serde(rename = "indexing", default)]
    pub processing: Option<u32>,
    /// Number of assets that have finished processing (ready)
    #[serde(rename = "finished", default)]
    pub ready: Option<u32>,
    /// Number of assets that failed to process
    #[serde(rename = "failed", default)]
    pub failed: Option<u32>,
    /// Number of assets that are unsupported
    #[serde(rename = "unsupported", default)]
    pub unsupported: Option<u32>,
    /// Number of assets with no 3D data
    #[serde(rename = "no-3d-data", default)]
    pub no_3d_data: Option<u32>,
}

impl AssetStateCounts {
    /// Create a new AssetStateCounts instance
    pub fn new(
        processing: Option<u32>,
        ready: Option<u32>,
        failed: Option<u32>,
        unsupported: Option<u32>,
        no_3d_data: Option<u32>,
    ) -> AssetStateCounts {
        AssetStateCounts {
            processing,
            ready,
            failed,
            unsupported,
            no_3d_data,
        }
    }

    /// Helper method to get the value or 0 if None
    fn get_or_default(value: Option<u32>) -> u32 {
        value.unwrap_or(0)
    }
}

impl crate::format::Formattable for AssetStateCounts {
    fn format(
        &self,
        f: &crate::format::OutputFormat,
    ) -> Result<String, crate::format::FormattingError> {
        match f {
            crate::format::OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                match json {
                    Ok(json_str) => Ok(json_str),
                    Err(e) => Err(crate::format::FormattingError::JsonSerializationError(e)),
                }
            }
            crate::format::OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "INDEXING",
                        "FINISHED",
                        "FAILED",
                        "UNSUPPORTED",
                        "NO-3D-DATA",
                    ))?;
                }

                wtr.serialize((
                    AssetStateCounts::get_or_default(self.processing),
                    AssetStateCounts::get_or_default(self.ready),
                    AssetStateCounts::get_or_default(self.failed),
                    AssetStateCounts::get_or_default(self.unsupported),
                    AssetStateCounts::get_or_default(self.no_3d_data),
                ))?;

                let data = wtr.into_inner()?;
                let csv_string = String::from_utf8(data)?;
                Ok(csv_string)
            }
            crate::format::OutputFormat::Tree(_) => {
                // For tree format, just return a simple representation
                Ok(format!(
                    "Asset State Counts:\n  Processing: {}\n  Ready: {}\n  Failed: {}\n  Unsupported: {}\n  No 3D Data: {}",
                    AssetStateCounts::get_or_default(self.processing),
                    AssetStateCounts::get_or_default(self.ready),
                    AssetStateCounts::get_or_default(self.failed),
                    AssetStateCounts::get_or_default(self.unsupported),
                    AssetStateCounts::get_or_default(self.no_3d_data)
                ))
            }
        }
    }
}

/// Represents a match result from the text search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMatch {
    /// The matching asset details
    pub asset: AssetResponse,
    /// The relevance score of the match (may not be present in all API responses)
    #[serde(rename = "relevanceScore", default, skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f64>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl TextMatch {
    /// Get the asset path
    pub fn path(&self) -> String {
        self.asset.path.clone()
    }

    /// Get the asset UUID
    pub fn asset_uuid(&self) -> Uuid {
        self.asset.uuid
    }

    /// Get the relevance score
    pub fn score(&self) -> f64 {
        self.relevance_score.unwrap_or(0.0)
    }
}

impl CsvRecordProducer for TextMatch {
    /// Get the CSV header row for TextMatch records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_NAME".to_string(),
            "ASSET_PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "RELEVANCE_SCORE".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_URL".to_string(),
        ]
    }

    /// Convert the TextMatch to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        // Extract the asset name from the path (last segment after the final '/')
        let asset_name = self.asset.path.split('/').last().unwrap_or(&self.asset.path);

        // Build the asset URL using the template: {baseUrl}/tenants/{tenantId}/asset/{assetUuid}
        let asset_url = if let Some(ref comparison_url) = self.comparison_url {
            // Extract base URL from the comparison URL and build the asset URL
            let url_parts: Vec<&str> = comparison_url.split("/compare?").collect();
            if let Some(base_url) = url_parts.first() {
                // Check if the base URL already contains the tenant path to avoid duplication
                if base_url.contains("/tenants/") {
                    // If the base URL already has tenant info, just replace compare with asset
                    format!("{}/asset/{}",
                        base_url,
                        self.asset.uuid
                    )
                } else {
                    // If the base URL doesn't have tenant info, add it
                    format!("{}/tenants/{}/asset/{}",
                        base_url,
                        self.asset.tenant_id,
                        self.asset.uuid
                    )
                }
            } else {
                comparison_url.replace("compare?", "asset/").replace("&", "")
            }
        } else {
            // If no comparison URL is available, construct a basic URL
            format!("https://app.physna.com/tenants/{}/asset/{}",
                self.asset.tenant_id,
                self.asset.uuid
            )
        };

        vec![vec![
            asset_name.to_string(), // ASSET_NAME
            self.asset.path.clone(), // ASSET_PATH
            self.asset.asset_type.clone(), // TYPE
            self.asset.state.clone(), // STATE
            self.asset.is_assembly.to_string(), // IS_ASSEMBLY
            format!("{}", self.relevance_score.unwrap_or(0.0)), // RELEVANCE_SCORE
            self.asset.uuid.to_string(), // ASSET_UUID
            asset_url, // ASSET_URL
        ]]
    }
}

/// Response structure for text search operations
///
/// This structure holds the results of a text search operation, including
/// the list of matching assets and pagination/filter information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSearchResponse {
    /// The list of matching assets
    pub matches: Vec<TextMatch>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
    /// Filter information
    #[serde(rename = "filterData")]
    pub filter_data: Option<FilterData>,
}

/// Enhanced response structure for text search that includes search query information
///
/// This structure extends the basic TextSearchResponse by including information about
/// the search query that was performed, making it easier to understand
/// the context of the matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedTextSearchResponse {
    /// The search query that was performed
    #[serde(rename = "searchQuery")]
    pub search_query: String,
    /// The list of matching assets
    pub matches: Vec<TextMatch>,
}

impl CsvRecordProducer for EnhancedTextSearchResponse {
    /// Get the CSV header row for EnhancedTextSearchResponse records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_NAME".to_string(),
            "ASSET_PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "RELEVANCE_SCORE".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_URL".to_string(),
        ]
    }

    /// Convert the EnhancedTextSearchResponse to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        self.matches
            .iter()
            .flat_map(|m| {
                m.as_csv_records()
                    .into_iter()
                    .collect::<Vec<Vec<String>>>()
            })
            .collect()
    }
}

/// Represents a pair of assets that matched in a text search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMatchPair {
    /// The reference asset (the one being searched against)
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The candidate asset (the one that matched)
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The relevance score
    #[serde(rename = "relevanceScore")]
    pub relevance_score: f64,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl CsvRecordProducer for TextMatchPair {
    /// Get the CSV header row for TextMatchPair records
    fn csv_header() -> Vec<String> {
        vec![
            "ASSET_NAME".to_string(),
            "ASSET_PATH".to_string(),
            "TYPE".to_string(),
            "STATE".to_string(),
            "IS_ASSEMBLY".to_string(),
            "RELEVANCE_SCORE".to_string(),
            "ASSET_UUID".to_string(),
            "ASSET_URL".to_string(),
        ]
    }

    /// Convert the TextMatchPair to CSV records
    fn as_csv_records(&self) -> Vec<Vec<String>> {
        // Extract the asset name from the path (last segment after the final '/')
        let asset_name = self.reference_asset.path.split('/').last().unwrap_or(&self.reference_asset.path);

        // Build the asset URL using the template: {baseUrl}/tenants/{tenantId}/asset/{assetUuid}
        let asset_url = if let Some(ref comparison_url) = self.comparison_url {
            // Extract base URL from the comparison URL and build the asset URL
            let url_parts: Vec<&str> = comparison_url.split("/compare?").collect();
            if let Some(base_url) = url_parts.first() {
                // Check if the base URL already contains the tenant path to avoid duplication
                if base_url.contains("/tenants/") {
                    // If the base URL already has tenant info, just replace compare with asset
                    format!("{}/asset/{}",
                        base_url,
                        self.reference_asset.uuid
                    )
                } else {
                    // If the base URL doesn't have tenant info, add it
                    format!("{}/tenants/{}/asset/{}",
                        base_url,
                        self.reference_asset.tenant_id,
                        self.reference_asset.uuid
                    )
                }
            } else {
                comparison_url.replace("compare?", "asset/").replace("&", "")
            }
        } else {
            // If no comparison URL is available, construct a basic URL
            format!("https://app.physna.com/tenants/{}/asset/{}",
                self.reference_asset.tenant_id,
                self.reference_asset.uuid
            )
        };

        vec![vec![
            asset_name.to_string(), // ASSET_NAME
            self.reference_asset.path.clone(), // ASSET_PATH
            self.reference_asset.asset_type.clone(), // TYPE
            self.reference_asset.state.clone(), // STATE
            self.reference_asset.is_assembly.to_string(), // IS_ASSEMBLY
            format!("{}", self.relevance_score), // RELEVANCE_SCORE
            self.reference_asset.uuid.to_string(), // ASSET_UUID
            asset_url, // ASSET_URL
        ]]
    }
}

impl From<&TextMatch> for TextMatchPair {
    fn from(text_match: &TextMatch) -> Self {
        TextMatchPair {
            reference_asset: text_match.asset.clone(), // For text search, we'll treat the matched asset as both ref and candidate
            candidate_asset: text_match.asset.clone(),
            relevance_score: text_match.relevance_score.unwrap_or(0.0),
            comparison_url: text_match.comparison_url.clone(),
        }
    }
}

impl OutputFormatter for TextMatchPair {
    type Item = TextMatchPair;

    /// Format the TextMatchPair according to the specified output format
    fn format(&self, f: OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    if options.with_metadata {
                        // Include metadata columns in the header
                        let mut base_headers = TextMatchPair::csv_header();

                        // Get unique metadata keys from both reference and candidate assets
                        let mut all_metadata_keys = std::collections::HashSet::new();

                        // Collect metadata keys from reference asset
                        for key in self.reference_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Collect metadata keys from candidate asset
                        for key in self.candidate_asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }

                        // Sort metadata keys for consistent column ordering
                        let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                        sorted_keys.sort();

                        // Extend headers with metadata columns
                        for key in &sorted_keys {
                            base_headers.push(format!("REF_{}", key.to_uppercase()));
                            base_headers.push(format!("CAND_{}", key.to_uppercase()));
                        }

                        wtr.serialize(base_headers.as_slice())?;
                    } else {
                        wtr.serialize(TextMatchPair::csv_header())?;
                    }
                }

                if options.with_metadata {
                    // Include metadata values in the output
                    let mut base_values = vec![
                        self.reference_asset.path.clone(),
                        self.candidate_asset.path.clone(),
                        format!("{}", self.relevance_score),
                        self.reference_asset.uuid.to_string(),
                        self.candidate_asset.uuid.to_string(),
                        self.comparison_url.clone().unwrap_or_default(),
                    ];

                    // Get unique metadata keys from both reference and candidate assets
                    let mut all_metadata_keys = std::collections::HashSet::new();

                    // Collect metadata keys from reference asset
                    for key in self.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Collect metadata keys from candidate asset
                    for key in self.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Add metadata values for each key
                    for key in &sorted_keys {
                        // Add reference asset metadata value
                        let ref_value = self
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = self
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }

                    wtr.serialize(base_values.as_slice())?;
                } else {
                    wtr.serialize(vec![
                        self.reference_asset.path.clone(),
                        self.candidate_asset.path.clone(),
                        format!("{}", self.relevance_score),
                        self.reference_asset.uuid.to_string(),
                        self.candidate_asset.uuid.to_string(),
                        self.comparison_url.clone().unwrap_or_default(),
                    ])?;
                }

                let data = wtr.into_inner()?;
                String::from_utf8(data).map_err(FormattingError::Utf8Error)
            }
            _ => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

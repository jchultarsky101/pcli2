//! Folders and tenants.

use super::*;

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
/// * `path` - Full path of the folder in the hierarchy (e.g., "/Home/Parent/Child")
///
/// # Examples
/// ```
/// use pcli2::model::Folder;
/// use uuid::Uuid;
///
/// let folder = Folder::new(
///     Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
///     "My Folder".to_string(),
///     "/Home/My Folder".to_string(),
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
    /// The folder's description; absent when it has none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
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
            description: None,
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
            description: folder_response.description,
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

    /// The folder's description, if it has one
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Create a new FolderBuilder for constructing Folder instances
    pub fn builder() -> FolderBuilder {
        FolderBuilder::new()
    }
}

impl From<FolderResponse> for Folder {
    fn from(fr: FolderResponse) -> Folder {
        Folder {
            uuid: fr.uuid,
            name: fr.name.clone(),
            path: "".to_string(),
            assets_count: fr.assets_count,
            folders_count: fr.folders_count,
            description: fr.description,
        }
    }
}

impl From<SingleFolderResponse> for Folder {
    fn from(fr: SingleFolderResponse) -> Folder {
        Folder {
            uuid: fr.folder.uuid,
            name: fr.folder.name.clone(),
            path: "".to_string(),
            assets_count: fr.folder.assets_count,
            folders_count: fr.folder.folders_count,
            description: fr.folder.description,
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
            Some(uuid) => *uuid,
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

    /// Get an iterator over all folders in the FolderList
    ///
    /// # Returns
    /// An iterator over the folders
    pub fn iter(&self) -> impl Iterator<Item = &Folder> {
        self.folders.iter()
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

    /// Get an iterator over all tenants in the TenantList
    ///
    /// # Returns
    /// An iterator over the tenants
    pub fn iter(&self) -> impl Iterator<Item = &Tenant> {
        self.tenants.iter()
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
    // Optional in the specification (the contract test found this): a tenant
    // without them used to fail the whole tenant listing.
    #[serde(rename = "tenantDisplayName", default)]
    pub tenant_display_name: String,
    /// The short name of the tenant
    #[serde(rename = "tenantShortName", default)]
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

impl CurrentUserResponse {}

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
    /// The folder's description; the API leaves the field out when there is none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
            description: None,
        }
    }
}

#[cfg(test)]
mod folder_description_model_tests {
    use super::*;

    const WITH: &str = r#"{"folder":{"id":"b6ca24cc-eb90-4d30-807a-2a5b62084b1c","name":"Rail","description":"Rail parts","tenantId":"68555ebf-f09c-4861-96b1-692d2ec10de7","createdAt":"2026-09-23T21:55:09.848Z","updatedAt":"2026-09-23T21:55:10.313Z","assetsCount":0,"foldersCount":0}}"#;

    #[test]
    fn the_description_is_read_when_the_api_sends_one() {
        let folder: Folder = serde_json::from_str::<SingleFolderResponse>(WITH)
            .unwrap()
            .into();
        assert_eq!(folder.description(), Some("Rail parts"));
        let json = serde_json::to_value(&folder).unwrap();
        assert_eq!(json["description"], "Rail parts");
    }

    #[test]
    fn a_folder_without_one_serializes_as_before() {
        let without = WITH.replace(r#""description":"Rail parts","#, "");
        let folder: Folder = serde_json::from_str::<SingleFolderResponse>(&without)
            .unwrap()
            .into();
        assert_eq!(folder.description(), None);
        // No `description` key: JSON output for such folders is unchanged, and a
        // folder cache written by an older pcli2 still loads.
        let json = serde_json::to_string(&folder).unwrap();
        assert!(!json.contains("description"), "{json}");
        let cached: Folder = serde_json::from_str(
            r#"{"id":"b6ca24cc-eb90-4d30-807a-2a5b62084b1c","name":"Rail","path":"/Rail","assetsCount":0,"foldersCount":0}"#,
        )
        .unwrap();
        assert_eq!(cached.description(), None);
    }
}

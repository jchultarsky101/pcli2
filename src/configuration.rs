use dirs::config_dir;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};
use url::Url;

const DEFAULT_APPLICATION_ID: &'static str = "pcli2";
const DEFAULT_CONFIGURATION_FILE_NAME: &'static str = "config.yml";

const DEFAULT_TENANT: &'static str = "default_tenant";
const DEFAULT_OUTPUT_FORMAT: &'static str = "default_output_format";

const JSON: &'static str = "json";
const JSON_PRETTY: &'static str = "json_pretty";
const CSV: &'static str = "csv";
const CSV_PRETTY: &'static str = "csv_pretty";
const TABLE: &'static str = "table";
const TABLE_PRETTY: &'static str = "table_pretty";
const TREE: &'static str = "tree";
const TREE_PRETTY: &'static str = "tree_pretty";

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("failed to resolve the configuration directory")]
    FailedToFindConfigurationDirectory,
    // #[error("invalid configuration property name '{name:?}'")]
    // InvalidPropertyName { name: String },
    // #[error("Invalid value '{value:?} for property '{name:?}'")]
    // InvalidValueForProperty { name: String, value: String },
    #[error("failed to load configuration data, because of: {cause:?}")]
    FailedToLoadData { cause: String },
    #[error("failed to write configuration data to file, because of: {cause:?}")]
    FailedToWriteData { cause: String },
    #[error("invalid property name \"{name:?}\"")]
    InvalidPropertyName { name: String },
    #[error("invalid output format \"{format:?}\"")]
    InvalidOutputFormat { format: String },
    #[error("failed to set property value for property due to: \"{cause:?}\"")]
    FailedToSetValue { cause: String },
    #[error("missing value for property \"{name:?}\"")]
    MissingRequiredPropertyValue { name: String },
}

#[derive(Debug, PartialEq)]
enum ConfigurationPropertyName {
    DefaultTenant,
    DefaultOutputFormat,
}

impl FromStr for ConfigurationPropertyName {
    type Err = ConfigurationError;

    fn from_str(name: &str) -> Result<ConfigurationPropertyName, ConfigurationError> {
        match name.to_lowercase().as_str() {
            DEFAULT_TENANT => Ok(ConfigurationPropertyName::DefaultTenant),
            DEFAULT_OUTPUT_FORMAT => Ok(ConfigurationPropertyName::DefaultOutputFormat),
            _ => Err(ConfigurationError::InvalidPropertyName {
                name: name.to_string(),
            }),
        }
    }
}

impl std::fmt::Display for ConfigurationPropertyName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigurationPropertyName::DefaultTenant => write!(f, "default_tenant"),
            ConfigurationPropertyName::DefaultOutputFormat => write!(f, "default_output_format"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantConfiguration {
    tenant_id: String,
    api_url: Url,
    oidc_url: Url,
    client_id: String,
    client_secret: String,
}

impl TenantConfiguration {
    pub fn new(
        tenant_id: String,
        api_url: Url,
        oidc_url: Url,
        client_id: String,
        client_secret: String,
    ) -> TenantConfiguration {
        TenantConfiguration {
            tenant_id,
            api_url,
            oidc_url,
            client_id,
            client_secret,
        }
    }

    pub fn set_tenant_id(&mut self, tenant_id: String) {
        self.tenant_id = tenant_id.clone();
    }

    pub fn tenant_id(&self) -> String {
        self.tenant_id.clone()
    }

    pub fn set_api_url(&mut self, api_url: Url) {
        self.api_url = api_url.clone();
    }

    pub fn api_url(&self) -> Url {
        self.api_url.clone()
    }

    pub fn set_oidc_url(&mut self, oidc_url: Url) {
        self.oidc_url = oidc_url.clone();
    }

    pub fn oidc_url(&self) -> Url {
        self.oidc_url.clone()
    }

    pub fn set_client_id(&mut self, client_id: String) {
        self.client_id = client_id.clone();
    }

    pub fn client_id(&self) -> String {
        self.client_id.clone()
    }

    pub fn set_client_secret(&mut self, client_secret: String) {
        self.client_secret = client_secret.clone();
    }

    pub fn client_secret(&self) -> String {
        self.client_secret.clone()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Csv,
    CsvPretty,
    #[default]
    Json,
    JsonPretty,
    Table,
    TablePretty,
    Tree,
    TreePretty,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::CsvPretty => write!(f, "csv_pretty"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::JsonPretty => write!(f, "json_pretty"),
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::TablePretty => write!(f, "table_pretty"),
            OutputFormat::Tree => write!(f, "tree"),
            OutputFormat::TreePretty => write!(f, "tree_pretty"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = ConfigurationError;

    fn from_str(format_str: &str) -> Result<OutputFormat, ConfigurationError> {
        let normalized_format = format_str.to_lowercase();
        let normalized_format = normalized_format.as_str();
        match normalized_format {
            JSON => Ok(OutputFormat::Json),
            JSON_PRETTY => Ok(OutputFormat::JsonPretty),
            CSV => Ok(OutputFormat::Csv),
            CSV_PRETTY => Ok(OutputFormat::CsvPretty),
            TABLE => Ok(OutputFormat::Table),
            TABLE_PRETTY => Ok(OutputFormat::TablePretty),
            TREE => Ok(OutputFormat::Tree),
            TREE_PRETTY => Ok(OutputFormat::TreePretty),
            _ => Err(ConfigurationError::InvalidOutputFormat {
                format: format_str.to_string(),
            }),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    default_tenant: Option<String>,
    default_format: Option<OutputFormat>,
    tenants: HashMap<String, TenantConfiguration>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            default_tenant: None,
            default_format: Some(OutputFormat::Json),
            tenants: HashMap::new(),
        }
    }
}

impl Configuration {
    pub fn get_default_configuration_file_path() -> Result<PathBuf, ConfigurationError> {
        let configuration_directory = config_dir();
        match configuration_directory {
            Some(configuration_directory) => {
                let mut default_config_file_path = configuration_directory;
                default_config_file_path.push(DEFAULT_APPLICATION_ID);
                default_config_file_path.push(DEFAULT_CONFIGURATION_FILE_NAME);

                Ok(default_config_file_path)
            }
            None => Err(ConfigurationError::FailedToFindConfigurationDirectory),
        }
    }

    pub fn load_default() -> Result<Configuration, ConfigurationError> {
        let default_file_path = Configuration::get_default_configuration_file_path()?;
        Configuration::load_from_file(default_file_path)
    }

    pub fn load_from_file(path: PathBuf) -> Result<Configuration, ConfigurationError> {
        match fs::read_to_string(path.clone()) {
            Ok(configuration) => {
                let configuration = serde_yaml::from_str(&configuration);
                match configuration {
                    Ok(configuration) => Ok(configuration),
                    Err(e) => Err(ConfigurationError::FailedToLoadData {
                        cause: format!(
                            "failed to read configuration file from path {}. Cause: {}",
                            path.into_os_string().into_string().unwrap(),
                            e.to_string()
                        ),
                    }),
                }
            }
            Err(e) => Err(ConfigurationError::FailedToLoadData {
                cause: format!(
                    "failed to read the configuration file from path {}. Cause: {}",
                    path.into_os_string().into_string().unwrap(),
                    e.to_string()
                ),
            }),
        }
    }

    pub fn save_to_file(&self, path: PathBuf) -> Result<(), ConfigurationError> {
        // first check if the parent directory exists and try to create it if not
        let configuration_directory = path.parent();
        match configuration_directory {
            Some(path) => {
                // this operation only executes if the directory does not exit
                match fs::create_dir_all(path) {
                    Ok(()) => (),
                    Err(_) => return Err(ConfigurationError::FailedToFindConfigurationDirectory),
                }
            }
            None => return Err(ConfigurationError::FailedToFindConfigurationDirectory),
        }

        let configuration = self.clone();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path);

        match file {
            Ok(file) => match serde_yaml::to_writer(file, configuration) {
                Ok(()) => Ok(()),
                Err(e) => Err(ConfigurationError::FailedToWriteData {
                    cause: e.to_string(),
                }),
            },
            Err(e) => Err(ConfigurationError::FailedToWriteData {
                cause: e.to_string(),
            }),
        }
    }

    pub fn save_to_default(&self) -> Result<(), ConfigurationError> {
        self.save_to_file(Self::get_default_configuration_file_path()?)
    }

    pub fn get_default_tenant(&self) -> Option<String> {
        self.default_tenant.clone()
    }

    pub fn set_default_tenant(&mut self, default_tenant: Option<String>) {
        self.default_tenant = default_tenant;
    }

    pub fn get_default_format(&self) -> Option<OutputFormat> {
        self.default_format.clone()
    }

    pub fn set_default_format(&mut self, default_format: Option<OutputFormat>) {
        self.default_format = default_format;
    }

    pub fn get_all_valid_property_names() -> Vec<String> {
        let mut result = Vec::new();

        result.push(ConfigurationPropertyName::DefaultTenant.to_string());
        result.push(ConfigurationPropertyName::DefaultOutputFormat.to_string());

        result
    }

    pub fn get(&self, name: String) -> Option<String> {
        let name = ConfigurationPropertyName::from_str(name.to_uppercase().as_str());

        match name {
            Ok(name) => match name {
                ConfigurationPropertyName::DefaultTenant => match self.get_default_tenant() {
                    Some(tenant) => Some(tenant.to_string()),
                    None => None,
                },
                ConfigurationPropertyName::DefaultOutputFormat => match self.get_default_format() {
                    Some(format) => Some(format.to_string()),
                    None => None,
                },
            },
            Err(_) => None,
        }
    }

    pub fn set(&mut self, name: String, value: Option<String>) -> Result<(), ConfigurationError> {
        let name = ConfigurationPropertyName::from_str(name.to_uppercase().as_str());

        match name {
            Ok(name) => match name {
                ConfigurationPropertyName::DefaultTenant => Ok(self.set_default_tenant(value)),
                ConfigurationPropertyName::DefaultOutputFormat => match value {
                    Some(value) => {
                        let format = OutputFormat::from_str(value.as_str())?;
                        Ok(self.set_default_format(Some(format)))
                    }
                    None => Err(ConfigurationError::MissingRequiredPropertyValue {
                        name: (DEFAULT_OUTPUT_FORMAT.to_string()),
                    }),
                },
            },
            Err(e) => Err(ConfigurationError::FailedToSetValue {
                cause: format!("{}", e),
            }),
        }
    }

    pub fn has_tenants(&self) -> bool {
        !self.tenants.is_empty()
    }

    pub fn add_tenant(
        &mut self,
        tenant_alias: Option<String>,
        tenant: TenantConfiguration,
    ) -> Result<(), ConfigurationError> {
        let alias = match tenant_alias {
            Some(alias) => alias,
            None => tenant.tenant_id.clone(),
        };
        self.tenants.insert(alias, tenant.clone());

        Ok(())
    }

    /// Returns an Option of an owned instance of TenantConfiguration
    /// if one exists, or None
    pub fn get_tenant(&self, tenant_id: &String) -> Option<TenantConfiguration> {
        let tenant = self.tenants.get(tenant_id);

        match tenant {
            Some(tenant) => Some(tenant.clone()),
            None => None,
        }
    }

    pub fn delete_tenant(&mut self, tenant_id: &String) {
        self.tenants.remove(tenant_id);
    }

    pub fn delete_all_tenants(&mut self) {
        self.tenants.clear()
    }

    pub fn get_all_tenant_aliases(&self) -> Vec<String> {
        self.tenants.keys().map(|k| k.to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_create_default() {
        let format = OutputFormat::default();
        assert_eq!(format, OutputFormat::Json);
    }

    #[test]
    fn test_output_format_to_string() {
        assert_eq!(OutputFormat::Csv.to_string(), CSV);
        assert_eq!(OutputFormat::CsvPretty.to_string(), CSV_PRETTY);
        assert_eq!(OutputFormat::Json.to_string(), JSON);
        assert_eq!(OutputFormat::JsonPretty.to_string(), JSON_PRETTY);
        assert_eq!(OutputFormat::Table.to_string(), TABLE);
        assert_eq!(OutputFormat::TablePretty.to_string(), TABLE_PRETTY);
        assert_eq!(OutputFormat::Tree.to_string(), TREE);
        assert_eq!(OutputFormat::TreePretty.to_string(), TREE_PRETTY);
    }

    #[test]
    fn test_format_from_string() {
        assert_eq!(OutputFormat::from_str(JSON).unwrap(), OutputFormat::Json);
        assert_eq!(
            OutputFormat::from_str(JSON_PRETTY).unwrap(),
            OutputFormat::JsonPretty
        );
        assert_eq!(OutputFormat::from_str(CSV).unwrap(), OutputFormat::Csv);
        assert_eq!(
            OutputFormat::from_str(CSV_PRETTY).unwrap(),
            OutputFormat::CsvPretty
        );
        assert_eq!(OutputFormat::from_str(TABLE).unwrap(), OutputFormat::Table);
        assert_eq!(
            OutputFormat::from_str(TABLE_PRETTY).unwrap(),
            OutputFormat::TablePretty
        );
        assert_eq!(OutputFormat::from_str(TREE).unwrap(), OutputFormat::Tree);
        assert_eq!(
            OutputFormat::from_str(TREE_PRETTY).unwrap(),
            OutputFormat::TreePretty
        );
    }

    #[test]
    #[should_panic]
    fn test_fail_on_invalid_name_for_output_format_from_string() {
        OutputFormat::from_str("invalid_format_name").unwrap();
    }

    #[test]
    fn test_create_default_configuration() {
        let configuration = Configuration::default();
        assert_eq!(
            configuration,
            Configuration {
                default_tenant: None,
                default_format: Some(OutputFormat::Json),
                tenants: HashMap::new(),
            }
        );
    }

    #[test]
    fn test_write_configuration_file() {
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();
        let configuration = Configuration::default();
        configuration.save_to_file(path.to_path_buf()).unwrap();
        path.close().unwrap();
    }

    #[test]
    fn test_read_configuration_file() {
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();
        let mut configuration = Configuration::default();
        configuration.set_default_tenant(Some("mytenant".to_string()));
        configuration.save_to_file(path.to_path_buf()).unwrap();

        let configuration2 = Configuration::load_from_file(path.to_path_buf()).unwrap();

        assert_eq!(configuration2, configuration);
    }

    #[test]
    fn test_set_default_tenant() {
        let mut configuration = Configuration::default();
        let tenant = String::from("mytenant");
        configuration.set_default_tenant(Some(tenant.clone()));
        assert_eq!(Some(tenant), configuration.get_default_tenant());
    }

    #[test]
    fn test_set_default_output_format() {
        let mut configuration = Configuration::default();
        let format = OutputFormat::Csv;
        configuration.set_default_format(Some(format.clone()));
        assert_eq!(Some(format), configuration.get_default_format());
    }

    #[test]
    fn test_debug_on_configuration_property_name() {
        let name = ConfigurationPropertyName::DefaultTenant;
        assert_eq!(format!("{:?}", name), "DefaultTenant");
    }

    #[test]
    fn test_from_string_for_configuration_property_name() {
        assert_eq!(
            ConfigurationPropertyName::from_str(DEFAULT_TENANT).unwrap(),
            ConfigurationPropertyName::DefaultTenant
        );
        assert_eq!(
            ConfigurationPropertyName::from_str(DEFAULT_OUTPUT_FORMAT).unwrap(),
            ConfigurationPropertyName::DefaultOutputFormat
        );
    }

    #[test]
    #[should_panic]
    fn test_fail_on_incorrect_configuration_property_name() {
        let _ = ConfigurationPropertyName::from_str("invalid_name").unwrap();
    }

    #[test]
    fn test_display_for_configuration_property_name() {
        assert_eq!(
            format!("{}", ConfigurationPropertyName::DefaultTenant),
            DEFAULT_TENANT
        );
        assert_eq!(
            format!("{}", ConfigurationPropertyName::DefaultOutputFormat),
            DEFAULT_OUTPUT_FORMAT
        );
    }

    #[test]
    fn test_get_default_configuration_file_path() {
        use dirs;
        let mut default_config_file_path = dirs::config_dir().unwrap();
        default_config_file_path.push(DEFAULT_APPLICATION_ID);
        default_config_file_path.push(DEFAULT_CONFIGURATION_FILE_NAME);

        assert_eq!(
            Configuration::get_default_configuration_file_path().unwrap(),
            default_config_file_path,
        );
    }

    #[test]
    fn test_load_default_configuration() {
        // make a copy of the original file
        let new_configuration = Configuration::default();
        new_configuration.save_to_default().unwrap();
        let new_configuration = Configuration::load_default().unwrap();
        assert_eq!(new_configuration, Configuration::default());
    }

    #[test]
    #[should_panic]
    fn test_fail_if_reading_nonexisting_config_file() {
        Configuration::load_from_file(PathBuf::from("/this/file/does/not/exist")).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_fail_on_malformed_yaml_file() {
        use std::fs;
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = &file.into_temp_path();
        let yaml = r#"this is not valid YAML content"#;
        fs::write(path.to_path_buf(), yaml).unwrap();

        Configuration::load_from_file(path.to_path_buf()).unwrap();
    }

    #[test]
    fn test_get_all_valid_property_names() {
        let names = Configuration::get_all_valid_property_names();
        let known_names = vec![
            DEFAULT_TENANT.to_string(),
            DEFAULT_OUTPUT_FORMAT.to_string(),
        ];

        assert_eq!(names, known_names);
    }

    #[test]
    fn test_get_property_value() {
        let mut configuration = Configuration::default();

        assert_eq!(configuration.get(DEFAULT_TENANT.to_string()), None);

        configuration.set_default_format(None);
        assert_eq!(configuration.get(DEFAULT_OUTPUT_FORMAT.to_string()), None);

        let my_tenant = "mytenant".to_string();
        let my_format = OutputFormat::Table;
        configuration.set_default_tenant(Some(my_tenant.clone()));
        configuration.set_default_format(Some(my_format.clone()));

        let tenant = configuration.get(DEFAULT_TENANT.to_string()).unwrap();
        assert_eq!(Some(tenant), Some(my_tenant));

        let format = configuration
            .get(DEFAULT_OUTPUT_FORMAT.to_string())
            .unwrap();
        assert_eq!(Some(format), Some(my_format.to_string()));
    }

    #[test]
    #[should_panic]
    fn test_fail_on_invalid_property_name() {
        let configuration = Configuration::default();

        configuration
            .get("invalid property name".to_string())
            .unwrap();
    }

    #[test]
    fn test_set_configuration_value() {
        let mut configuration = Configuration::default();
        let my_tenant = "my_tenant".to_string();
        let my_format = OutputFormat::Table;

        configuration
            .set(DEFAULT_TENANT.to_string(), Some(my_tenant.clone()))
            .unwrap();
        assert_eq!(configuration.get_default_tenant(), Some(my_tenant));

        configuration
            .set(
                DEFAULT_OUTPUT_FORMAT.to_string(),
                Some(my_format.to_string()),
            )
            .unwrap();
        assert_eq!(
            Some(format!("{}", configuration.get_default_format().unwrap())),
            Some(format!("{}", my_format))
        );
    }

    #[test]
    #[should_panic]
    fn test_fail_on_invalid_property_name_for_set() {
        let mut configuration = Configuration::default();
        let name = "this is invalid".to_string();

        configuration
            .set(name, Some("some value".to_string()))
            .unwrap();
    }

    #[test]
    #[should_panic]
    fn test_fail_on_empty_format_value_for_set() {
        let mut configuration = Configuration::default();

        configuration
            .set(DEFAULT_OUTPUT_FORMAT.to_string(), None)
            .unwrap();
    }

    #[test]
    fn test_new_for_tenant_configuration() {
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();
        let tenant_config_one = TenantConfiguration {
            tenant_id: tenant_id.clone(),
            api_url: api_url.clone(),
            oidc_url: oidc_url.clone(),
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
        };
        let tenant_config_two = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        assert_eq!(tenant_config_one, tenant_config_two);
    }

    #[test]
    fn test_debug_for_tenant_configuration() {
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();
        let json = r#"TenantConfiguration { tenant_id: "my_tenant", api_url: Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("my_tenant.physna.com")), port: None, path: "/api/v2", query: None, fragment: None }, oidc_url: Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("authentication.com")), port: None, path: "/", query: None, fragment: None }, client_id: "my_client_id", client_secret: "my_client_secret" }"#;

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        assert_eq!(format!("{:?}", tenant), format!("{}", json));
    }

    #[test]
    fn test_add_tenant() {
        let mut configuration = Configuration::default();

        let tenant_alias = "my_alias".to_string();
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        configuration
            .add_tenant(Some(tenant_alias.clone()), tenant.clone())
            .unwrap();

        let tenant2 = configuration.get_tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);
    }

    #[test]
    fn test_get_tenant() {
        let mut configuration = Configuration::default();

        let tenant_alias = "my_alias".to_string();
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        configuration
            .add_tenant(Some(tenant_alias.clone()), tenant.clone())
            .unwrap();

        let tenant2 = configuration.get_tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);

        let invalid_tenant_id = "invalid ID".to_string();
        let tenant2 = configuration.get_tenant(&invalid_tenant_id);
        assert_eq!(tenant2, None);
    }

    #[test]
    fn test_delete_tenant() {
        let mut configuration = Configuration::default();

        let tenant_alias = "my_alias".to_string();
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        // first add a tenant
        configuration
            .add_tenant(Some(tenant_alias.clone()), tenant.clone())
            .unwrap();

        // check that the tenant was correctly added
        let tenant2 = configuration.get_tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);

        // delete the tenant
        configuration.delete_tenant(&tenant_alias);

        // make sure that there are no more tenants
        assert!(!configuration.has_tenants());
    }

    #[test]
    fn test_has_tenants() {
        let configuration = Configuration::default();
        assert!(!configuration.has_tenants());
    }

    #[test]
    fn test_delete_all_tenants() {
        // create configuration
        let mut configuration = Configuration::default();

        // create a tenant configuration
        let tenant_alias = "my_alias".to_string();
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        // first add the tenant
        configuration
            .add_tenant(Some(tenant_alias.clone()), tenant.clone())
            .unwrap();

        // check that the tenant was correctly added
        let tenant2 = configuration.get_tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);

        // delete the tenant
        configuration.delete_all_tenants();

        // make sure that there are no more tenants
        assert!(!configuration.has_tenants());
    }

    #[test]
    fn test_get_tenant_ids() {
        let mut tenant_aliases = vec![
            "tenant_1".to_string(),
            "tenant_2".to_string(),
            "tenant_3".to_string(),
        ];
        tenant_aliases.sort();

        let mut configuration = Configuration::default();

        // create a tenant configuration
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let client_secret = "my_client_secret".to_string();

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
            client_secret.clone(),
        );

        configuration
            .add_tenant(Some(tenant_aliases[0].clone()), tenant.clone())
            .unwrap();
        configuration
            .add_tenant(Some(tenant_aliases[1].clone()), tenant.clone())
            .unwrap();
        configuration
            .add_tenant(Some(tenant_aliases[2].clone()), tenant.clone())
            .unwrap();

        let mut produced_ids = configuration.get_all_tenant_aliases();
        produced_ids.sort();
        assert_eq!(produced_ids, tenant_aliases);
    }

    #[test]
    fn test_configuration_tenant_setters() {
        let wrong = "wrong_value".to_string();
        let wrong_url = Url::parse("https://wrong.com").unwrap();
        let mut tenant = TenantConfiguration::new(
            wrong.clone(),
            wrong_url.clone(),
            wrong_url.clone(),
            wrong.clone(),
            wrong.clone(),
        );

        let tenant_id = "my_tenant".to_string();
        tenant.set_tenant_id(tenant_id.clone());
        assert_eq!(tenant.tenant_id(), tenant_id);

        let api_url = Url::parse("https://my_api_url.com").unwrap();
        tenant.set_api_url(api_url.clone());
        assert_eq!(tenant.api_url(), api_url);

        let oidc_url = Url::parse("https://my_oidc_url.com").unwrap();
        tenant.set_oidc_url(oidc_url.clone());
        assert_eq!(tenant.oidc_url(), oidc_url);

        let client_id = "my_client_id".to_string();
        tenant.set_client_id(client_id.clone());
        assert_eq!(tenant.client_id(), client_id);

        let client_secret = "my_client_secret".to_string();
        tenant.set_client_secret(client_secret.clone());
        assert_eq!(tenant.client_secret(), client_secret);
    }
}

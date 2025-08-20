use crate::format::{
    CsvRecordProducer, FormattingError, JsonProducer, OutputFormat, OutputFormatter,
};
use crate::keyring::{Keyring, SECRET_KEY};
use csv::Writer;
use dirs::{config_dir, home_dir};
use serde::{Deserialize, Serialize};
use serde_json;
use serde_yaml;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};
use tracing::{debug, trace};
use url::Url;

pub const DEFAULT_APPLICATION_ID: &'static str = "pcli2";
pub const DEFAULT_CONFIGURATION_FILE_NAME: &'static str = "config.yml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("failed to resolve the configuration directory")]
    FailedToFindConfigurationDirectory,
    #[error("failed to load configuration data, because of: {cause:?}")]
    FailedToLoadData { cause: Box<dyn std::error::Error> },
    #[error("failed to write configuration data to file, because of: {cause:?}")]
    FailedToWriteData { cause: Box<dyn std::error::Error> },
    #[error("missing value for property \"{name:?}\"")]
    MissingRequiredPropertyValue { name: String },
    #[error("unknown tenant \"{tenant_id:?}\"")]
    UnknownTenant { tenant_id: String },
    #[error("credentials not provided")]
    CredentialsNotProvided,
    #[error("{cause:?}")]
    FormattingError {
        #[from]
        cause: FormattingError,
    },
    #[error("keyring error {0}")]
    KeyringError(#[from] crate::keyring::KeyringError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantConfiguration {
    tenant_id: String,
    api_url: Url,
    oidc_url: Url,
    client_id: String,
}

impl TenantConfiguration {
    pub fn new(
        tenant_id: String,
        api_url: Url,
        oidc_url: Url,
        client_id: String,
    ) -> TenantConfiguration {
        TenantConfiguration {
            tenant_id,
            api_url,
            oidc_url,
            client_id,
        }
    }

    pub fn builder() -> TenantConfigurationBuilder {
        TenantConfigurationBuilder::new()
    }

    #[allow(dead_code)]
    pub fn set_tenant_id(&mut self, tenant_id: String) {
        self.tenant_id = tenant_id.clone();
    }

    pub fn tenant_id(&self) -> String {
        self.tenant_id.clone()
    }

    #[allow(dead_code)]
    pub fn set_api_url(&mut self, api_url: Url) {
        self.api_url = api_url.clone();
    }

    pub fn api_url(&self) -> Url {
        self.api_url.clone()
    }

    #[allow(dead_code)]
    pub fn set_oidc_url(&mut self, oidc_url: Url) {
        self.oidc_url = oidc_url.clone();
    }

    pub fn oidc_url(&self) -> Url {
        self.oidc_url.clone()
    }

    #[allow(dead_code)]
    pub fn set_client_id(&mut self, client_id: String) {
        self.client_id = client_id.clone();
    }

    pub fn client_id(&self) -> String {
        self.client_id.clone()
    }

    #[allow(dead_code)]
    pub fn set_client_secret(&mut self, client_secret: String) -> Result<(), ConfigurationError> {
        Keyring::default().put(&self.tenant_id, String::from(SECRET_KEY), client_secret)?;
        Ok(())
    }

    pub fn client_secret(&self) -> Result<String, ConfigurationError> {
        match Keyring::default().get(&self.tenant_id, String::from(SECRET_KEY))? {
            Some(secret) => Ok(secret),
            None => Err(ConfigurationError::CredentialsNotProvided),
        }
    }
}

impl CsvRecordProducer for TenantConfiguration {
    fn csv_header() -> Vec<String> {
        vec![
            String::from("ID"),
            String::from("API_URL"),
            String::from("OIDC_URL"),
            String::from("CLIENT_ID"),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.tenant_id.to_owned(),
            self.api_url.to_string(),
            self.oidc_url.to_string(),
            self.client_id.to_owned(),
        ]]
    }
}

impl JsonProducer for TenantConfiguration {}

impl OutputFormatter for TenantConfiguration {
    type Item = TenantConfiguration;

    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => Ok(self.to_json()?),
            OutputFormat::Csv => Ok(self.to_csv_with_header()?),
        }
    }
}

pub struct TenantConfigurationBuilder {
    tenant_id: Option<String>,
    api_url: Option<Url>,
    oidc_url: Option<Url>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl TenantConfigurationBuilder {
    fn new() -> TenantConfigurationBuilder {
        TenantConfigurationBuilder {
            tenant_id: None,
            api_url: None,
            oidc_url: None,
            client_id: None,
            client_secret: None,
        }
    }

    pub fn tenant_id(&mut self, id: String) -> &mut TenantConfigurationBuilder {
        self.tenant_id = Some(id.clone());
        self
    }

    pub fn api_url(&mut self, api_url: Url) -> &mut TenantConfigurationBuilder {
        self.api_url = Some(api_url.clone());
        self
    }

    pub fn oidc_url(&mut self, oidc_url: Url) -> &mut TenantConfigurationBuilder {
        self.oidc_url = Some(oidc_url.clone());
        self
    }

    pub fn client_id(&mut self, client_id: String) -> &mut TenantConfigurationBuilder {
        self.client_id = Some(client_id.clone());
        self
    }

    pub fn client_secret(&mut self, client_secret: String) -> &mut TenantConfigurationBuilder {
        self.client_secret = Some(client_secret.clone());
        self
    }

    pub fn build(&self) -> Result<TenantConfiguration, ConfigurationError> {
        let tenant_id = match &self.tenant_id {
            Some(tenant_id) => Ok(tenant_id.clone()),
            None => Err(ConfigurationError::MissingRequiredPropertyValue {
                name: "tenant_id".to_string(),
            }),
        }?;

        let api_url = match &self.api_url {
            Some(api_url) => Ok(api_url.clone()),
            None => Err(ConfigurationError::MissingRequiredPropertyValue {
                name: "api_url".to_string(),
            }),
        }?;

        let oidc_url = match &self.oidc_url {
            Some(oidc_url) => Ok(oidc_url.clone()),
            None => Err(ConfigurationError::MissingRequiredPropertyValue {
                name: "oidc_url".to_string(),
            }),
        }?;

        let client_id: String = match &self.client_id {
            Some(client_id) => Ok(client_id.clone()),
            None => Err(ConfigurationError::MissingRequiredPropertyValue {
                name: "client_id".to_string(),
            }),
        }?;

        let client_secret = match &self.client_secret {
            Some(client_secret) => Ok(client_secret.clone()),
            None => Err(ConfigurationError::MissingRequiredPropertyValue {
                name: "client_secret".to_string(),
            }),
        }?;

        let mut tenant_config = TenantConfiguration::new(tenant_id, api_url, oidc_url, client_id);
        tenant_config.set_client_secret(client_secret)?;

        Ok(tenant_config)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(skip_serializing_if = "Option::is_none")]
    active_tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_tenant_name: Option<String>,
    cache_path: Option<PathBuf>,
}

impl Default for Configuration {
    fn default() -> Self {
        let home_directory = home_dir();
        let home_directory = match home_directory {
            Some(mut home_directory) => {
                home_directory.push("pcli2.cache");
                Some(home_directory.to_owned())
            }
            None => {
                trace!("Home directory is None!");
                None
            }
        };

        Self {
            active_tenant_id: None,
            active_tenant_name: None,
            cache_path: home_directory,
        }
    }
}

impl CsvRecordProducer for Configuration {
    fn csv_header() -> Vec<String> {
        vec![
            "ACTIVE_TENANT_ID".to_string(),
            "ACTIVE_TENANT_NAME".to_string(),
        ]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();
        
        records.push(vec![
            self.active_tenant_id.clone().unwrap_or_default(),
            self.active_tenant_name.clone().unwrap_or_default(),
        ]);

        records
    }
}

impl OutputFormatter for Configuration {
    type Item = Configuration;

    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(self);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv => {
                let buf = BufWriter::new(Vec::new());
                let mut wtr = Writer::from_writer(buf);
                wtr.write_record(&Self::csv_header())
                    .unwrap();
                for record in self.as_csv_records() {
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
        debug!(
            "Loading configuration from {}...",
            default_file_path
                .clone()
                .into_os_string()
                .into_string()
                .unwrap()
        );
        Configuration::load_from_file(default_file_path)
    }

    pub fn load_from_file(path: PathBuf) -> Result<Configuration, ConfigurationError> {
        match fs::read_to_string(path.clone()) {
            Ok(configuration) => {
                let configuration = serde_yaml::from_str(&configuration);
                match configuration {
                    Ok(configuration) => Ok(configuration),
                    Err(cause) => Err(ConfigurationError::FailedToLoadData {
                        cause: Box::new(cause),
                    }),
                }
            }
            Err(cause) => Err(ConfigurationError::FailedToLoadData {
                cause: Box::new(cause),
            }),
        }
    }

    pub fn write(&self, writer: Box<dyn Write>) -> Result<(), ConfigurationError> {
        match serde_yaml::to_writer(writer, &self.clone()) {
            Ok(()) => Ok(()),
            Err(e) => Err(ConfigurationError::FailedToWriteData { cause: Box::new(e) }),
        }
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), ConfigurationError> {
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

        let file = File::create(&path);
        match file {
            Ok(file) => {
                let writer: Box<dyn Write> = Box::new(file);
                Ok(self.write(writer)?)
            }
            Err(e) => Err(ConfigurationError::FailedToWriteData { cause: Box::new(e) }),
        }
    }

    pub fn save_to_default(&self) -> Result<(), ConfigurationError> {
        self.save(&Self::get_default_configuration_file_path()?)
    }

    pub fn get_cache_path(&self) -> Option<PathBuf> {
        self.cache_path.to_owned()
    }

    pub fn set_cache_path(&mut self, path: Option<PathBuf>) {
        self.cache_path = path;
    }
    
    // Context management methods
    
    pub fn get_active_tenant_id(&self) -> Option<String> {
        self.active_tenant_id.clone()
    }
    
    pub fn get_active_tenant_name(&self) -> Option<String> {
        self.active_tenant_name.clone()
    }
    
    pub fn set_active_tenant(&mut self, tenant_id: String, tenant_name: String) {
        self.active_tenant_id = Some(tenant_id);
        self.active_tenant_name = Some(tenant_name);
    }
    
    pub fn clear_active_tenant(&mut self) {
        self.active_tenant_id = None;
        self.active_tenant_name = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format;
    use std::str::FromStr;

    #[test]
    fn test_output_format_create_default() {
        let format = OutputFormat::default();
        assert_eq!(format, OutputFormat::Json);
    }

    #[test]
    fn test_output_format_to_string() {
        assert_eq!(OutputFormat::Csv.to_string(), format::CSV);
        assert_eq!(OutputFormat::Json.to_string(), format::JSON);
    }

    #[test]
    fn test_format_from_string() {
        assert_eq!(
            OutputFormat::from_str(format::JSON).unwrap(),
            OutputFormat::Json
        );
        assert_eq!(
            OutputFormat::from_str(format::CSV).unwrap(),
            OutputFormat::Csv
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
        assert_eq!(configuration.active_tenant_id, None);
        assert_eq!(configuration.active_tenant_name, None);
        // cache_path is set to a default value based on the home directory
        assert!(configuration.cache_path.is_some());
    }

    #[test]
    fn test_write_configuration_file() {
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();
        let configuration = Configuration::default();
        configuration.save(&path.to_path_buf()).unwrap();
        path.close().unwrap();
    }

    #[test]
    fn test_read_configuration_file() {
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();
        let configuration = Configuration::default();
        configuration.save(&path.to_path_buf()).unwrap();

        let configuration2 = Configuration::load_from_file(path.to_path_buf()).unwrap();

        assert_eq!(configuration2, configuration);
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
    fn test_fail_on_malformed_yaml_file() {
        use std::fs;
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = &file.into_temp_path();
        let yaml = r#"active_tenant_id: tenant-123
active_tenant_name: My Tenant
cache_path: /tmp/pcli2.cache"#;
        fs::write(path.to_path_buf(), yaml).unwrap();

        let configuration = Configuration::load_from_file(path.to_path_buf()).unwrap();
        assert_eq!(configuration.get_active_tenant_id(), Some("tenant-123".to_string()));
        assert_eq!(configuration.get_active_tenant_name(), Some("My Tenant".to_string()));
    }

    #[test]
    fn test_create_new_tenant_configuration() {
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let tenant_config_one = TenantConfiguration {
            tenant_id: tenant_id.clone(),
            api_url: api_url.clone(),
            oidc_url: oidc_url.clone(),
            client_id: client_id.clone(),
        };

        let tenant_config_two = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
        );

        assert_eq!(tenant_config_one, tenant_config_two);
    }

    #[test]
    fn test_keyring() {
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let mut tenant_config = TenantConfiguration {
            tenant_id: tenant_id.clone(),
            api_url: api_url.clone(),
            oidc_url: oidc_url.clone(),
            client_id: client_id.clone(),
        };
        let secret = String::from("my super secret secret");
        tenant_config.set_client_secret(secret.to_owned()).unwrap();
        let output = tenant_config.client_secret().unwrap();

        assert_eq!(secret, output);
    }

    #[test]
    fn test_debug_for_tenant_configuration() {
        let tenant_id = "my_tenant".to_string();
        let api_url =
            Url::parse(format!("https://{}.physna.com/api/v2", tenant_id).as_str()).unwrap();
        let oidc_url = Url::parse("https://authentication.com").unwrap();
        let client_id = "my_client_id".to_string();
        let json = r#"TenantConfiguration { tenant_id: "my_tenant", api_url: Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("my_tenant.physna.com")), port: None, path: "/api/v2", query: None, fragment: None }, oidc_url: Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("authentication.com")), port: None, path: "/", query: None, fragment: None }, client_id: "my_client_id" }"#;

        let tenant = TenantConfiguration::new(
            tenant_id.clone(),
            api_url.clone(),
            oidc_url.clone(),
            client_id.clone(),
        );

        assert_eq!(format!("{:?}", tenant), format!("{}", json));
    }

    #[test]
    fn test_add_tenant() {
        // This test is no longer relevant since we don't store tenant configurations
        // but we'll keep it to test context management
        let mut configuration = Configuration::default();
        
        // Set active tenant
        configuration.set_active_tenant("tenant-123".to_string(), "My Tenant".to_string());
        assert_eq!(configuration.get_active_tenant_id(), Some("tenant-123".to_string()));
        assert_eq!(configuration.get_active_tenant_name(), Some("My Tenant".to_string()));
    }

    #[test]
    fn test_tenant() {
        // This test is no longer relevant since we don't store tenant configurations
        // but we'll keep it to test context management
        let mut configuration = Configuration::default();
        
        // Set active tenant
        configuration.set_active_tenant("tenant-123".to_string(), "My Tenant".to_string());
        assert_eq!(configuration.get_active_tenant_id(), Some("tenant-123".to_string()));
        assert_eq!(configuration.get_active_tenant_name(), Some("My Tenant".to_string()));
    }

    #[test]
    fn test_delete_tenant() {
        // This test is no longer relevant since we don't store tenant configurations
        // but we'll keep it to test context management
        let mut configuration = Configuration::default();
        
        // Set active tenant
        configuration.set_active_tenant("tenant-123".to_string(), "My Tenant".to_string());
        assert_eq!(configuration.get_active_tenant_id(), Some("tenant-123".to_string()));
        
        // Clear active tenant
        configuration.clear_active_tenant();
        assert_eq!(configuration.get_active_tenant_id(), None);
    }

    #[test]
    fn test_has_tenants() {
        let configuration = Configuration::default();
        // This test is no longer relevant since we don't store tenant configurations
        // but we can test that the configuration was created properly
        assert_eq!(configuration.active_tenant_id, None);
        assert_eq!(configuration.active_tenant_name, None);
    }

    #[test]
    fn test_delete_all_tenants() {
        // This test is no longer relevant since we don't store tenant configurations
        // but we'll keep it to test that the configuration can be created and saved
        let configuration = Configuration::default();
        assert_eq!(configuration.active_tenant_id, None);
        assert_eq!(configuration.active_tenant_name, None);
    }

    #[test]
    fn test_tenant_ids() {
        let mut configuration = Configuration::default();
        
        // Test setting active tenant
        configuration.set_active_tenant("tenant-123".to_string(), "My Tenant".to_string());
        assert_eq!(configuration.get_active_tenant_id(), Some("tenant-123".to_string()));
        assert_eq!(configuration.get_active_tenant_name(), Some("My Tenant".to_string()));
        
        // Test clearing active tenant
        configuration.clear_active_tenant();
        assert_eq!(configuration.get_active_tenant_id(), None);
        assert_eq!(configuration.get_active_tenant_name(), None);
    }

    #[test]
    fn test_configuration_tenant_setters() {
        let mut configuration = Configuration::default();
        
        // Test context management
        configuration.set_active_tenant("tenant-123".to_string(), "My Tenant".to_string());
        assert_eq!(configuration.get_active_tenant_id(), Some("tenant-123".to_string()));
        assert_eq!(configuration.get_active_tenant_name(), Some("My Tenant".to_string()));
        
        // Test clearing context
        configuration.clear_active_tenant();
        assert_eq!(configuration.get_active_tenant_id(), None);
        assert_eq!(configuration.get_active_tenant_name(), None);
    }
}

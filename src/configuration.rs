use crate::format::{FormattingError, OutputFormat, OutputFormatter};
use crate::security::{Keyring, KeyringError, SECRET_KEY};
use csv::Writer;
use dirs::config_dir;
use log::trace;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_yaml;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};
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
    #[error("security error {0}")]
    KeyringError(#[from] KeyringError),
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

impl OutputFormatter for TenantConfiguration {
    type Item = TenantConfiguration;

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
                wtr.write_record(&["ID", "API_URL", "OIDC_URL", "CLIENT_ID"])
                    .unwrap();
                wtr.write_record(&[
                    self.tenant_id(),
                    self.api_url().to_string(),
                    self.oidc_url().to_string(),
                    self.client_id(),
                ])
                .unwrap();
                match wtr.flush() {
                    Ok(_) => {
                        let bytes = wtr.into_inner().unwrap().into_inner().unwrap();
                        let csv = String::from_utf8(bytes).unwrap();
                        Ok(csv.clone())
                    }
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            _ => Err(FormattingError::UnsupportedOutputFormat {
                format: format.to_string(),
            }),
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
    tenants: HashMap<String, TenantConfiguration>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }

    pub fn validate_tenant(
        &self,
        tenant_id: &String,
    ) -> Result<TenantConfiguration, ConfigurationError> {
        trace!("Validating tenant ID of \"{}\"...", tenant_id);
        match self.tenant(tenant_id) {
            Some(tenant) => {
                trace!("Tenant ID \"{}\" is valid.", tenant_id);
                Ok(tenant)
            }
            None => Err(ConfigurationError::UnknownTenant {
                tenant_id: tenant_id.clone(),
            }),
        }
    }

    pub fn add_tenant(
        &mut self,
        tenant_alias: Option<&String>,
        tenant: &TenantConfiguration,
    ) -> Result<(), ConfigurationError> {
        let alias = match tenant_alias {
            Some(alias) => alias.clone(),
            None => tenant.tenant_id.clone(),
        };
        trace!("Adding tenant {}...", alias);
        self.tenants.insert(alias, tenant.clone());

        Ok(())
    }

    /// Returns an Option of an owned instance of TenantConfiguration
    /// if one exists, or None
    pub fn tenant(&self, tenant_id: &String) -> Option<TenantConfiguration> {
        let tenant = self.tenants.get(tenant_id);

        match tenant {
            Some(tenant) => Some(tenant.clone()),
            None => None,
        }
    }

    pub fn delete_tenant(&mut self, tenant_id: &String) {
        trace!("Deleting tenant {}...", tenant_id);
        self.tenants.remove(tenant_id);
    }

    #[allow(dead_code)]
    pub fn delete_all_tenants(&mut self) {
        self.tenants.clear()
    }

    #[allow(dead_code)]
    pub fn get_all_tenant_aliases(&self) -> Vec<String> {
        self.tenants.keys().map(|k| k.to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format;

    #[test]
    fn test_output_format_create_default() {
        let format = OutputFormat::default();
        assert_eq!(format, OutputFormat::Json);
    }

    #[test]
    fn test_output_format_to_string() {
        assert_eq!(OutputFormat::Csv.to_string(), format::CSV);
        assert_eq!(OutputFormat::Json.to_string(), format::JSON);
        assert_eq!(OutputFormat::Tree.to_string(), format::TREE);
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
        assert_eq!(
            OutputFormat::from_str(format::TREE).unwrap(),
            OutputFormat::Tree
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
    fn test_debug_on_configuration_property_name() {
        let name = ConfigurationPropertyName::DefaultTenant;
        assert_eq!(format!("{:?}", name), "DefaultTenant");
    }

    #[test]
    #[should_panic]
    fn test_fail_on_incorrect_configuration_property_name() {
        let _ = ConfigurationPropertyName::from_str("invalid_name").unwrap();
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
            .add_tenant(Some(&tenant_alias.clone()), &tenant)
            .unwrap();

        let tenant2 = configuration.tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);
    }

    #[test]
    fn test_tenant() {
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
            .add_tenant(Some(&tenant_alias.clone()), &tenant)
            .unwrap();

        let tenant2 = configuration.tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);

        let invalid_tenant_id = "invalid ID".to_string();
        let tenant2 = configuration.tenant(&invalid_tenant_id);
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
            .add_tenant(Some(&tenant_alias.clone()), &tenant)
            .unwrap();

        // check that the tenant was correctly added
        let tenant2 = configuration.tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);

        // delete the tenant
        configuration.delete_tenant(&tenant_alias);

        // make sure that there are no more tenants
        assert!(configuration.is_empty());
    }

    #[test]
    fn test_has_tenants() {
        let configuration = Configuration::default();
        assert!(configuration.is_empty());
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
            .add_tenant(Some(&tenant_alias.clone()), &tenant)
            .unwrap();

        // check that the tenant was correctly added
        let tenant2 = configuration.tenant(&tenant_alias).unwrap();
        assert_eq!(tenant2, tenant);

        // delete the tenant
        configuration.delete_all_tenants();

        // make sure that there are no more tenants
        assert!(configuration.is_empty());
    }

    #[test]
    fn test_tenant_ids() {
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
            .add_tenant(Some(&tenant_aliases[0].clone()), &tenant)
            .unwrap();
        configuration
            .add_tenant(Some(&tenant_aliases[1].clone()), &tenant)
            .unwrap();
        configuration
            .add_tenant(Some(&tenant_aliases[2].clone()), &tenant)
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

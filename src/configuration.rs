use dirs::home_dir;
use log::trace;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};

const DEFAULT_CONFIGURATION_FILE_NAME: &str = ".pcli2.conf";
const DEFAULT_TENANT: &'static str = "DEFAULT_TENANT";
const DEFAULT_OUTPUT_FORMAT: &'static str = "DEFAULT_OUTPUT_FORMAT";

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
    #[error("failed to determine the user's home directory")]
    FailedToFindHomeDirectory,
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
}

#[derive(Debug)]
enum ConfigurationPropertyName {
    DefaultTenant,
    DefaultOutputFormat,
}

impl FromStr for ConfigurationPropertyName {
    type Err = ConfigurationError;

    fn from_str(name: &str) -> Result<ConfigurationPropertyName, ConfigurationError> {
        match name.to_uppercase().as_str() {
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

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TenantConfiguration {
    cliend_id: String,
    client_secret: String,
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
    tenants: Option<HashMap<String, TenantConfiguration>>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            default_tenant: None,
            default_format: Some(OutputFormat::Json),
            tenants: None,
        }
    }
}

impl Configuration {
    pub fn get_default_configuration_file_path() -> Result<PathBuf, ConfigurationError> {
        let home_directory = home_dir();
        match home_directory {
            Some(home_directory) => {
                let mut default_config_file_path = home_directory;
                default_config_file_path.push(DEFAULT_CONFIGURATION_FILE_NAME);

                Ok(default_config_file_path)
            }
            None => Err(ConfigurationError::FailedToFindHomeDirectory),
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
        let configuration = self.clone();

        trace!(
            "Saving the configuration to file {}...",
            path.clone()
                .into_os_string()
                .into_string()
                .unwrap_or_default()
        );

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path);

        match file {
            Ok(file) => match serde_yaml::to_writer(file, configuration) {
                Ok(()) => {
                    trace!("Configuration saved.");
                    Ok(())
                }
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
        trace!("Setting default_tenant to value of {:?}...", default_tenant);
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
        trace!("Retrieving value for configuration property {:?}...", name);

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
        trace!("Setting the value for configuration property {:?}...", name);

        let name = ConfigurationPropertyName::from_str(name.to_uppercase().as_str());

        match name {
            Ok(name) => match name {
                ConfigurationPropertyName::DefaultTenant => Ok(self.set_default_tenant(value)),
                ConfigurationPropertyName::DefaultOutputFormat => match value {
                    Some(value) => {
                        let format = OutputFormat::from_str(value.as_str())?;
                        Ok(self.set_default_format(Some(format)))
                    }
                    None => Err(ConfigurationError::InvalidPropertyName {
                        name: ("".to_string()),
                    }),
                },
            },
            Err(e) => Err(ConfigurationError::FailedToSetValue {
                cause: format!("{}", e),
            }),
        }
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
    fn test_create_default_configuration() {
        let configuration = Configuration::default();
        assert_eq!(
            configuration,
            Configuration {
                default_tenant: None,
                default_format: Some(OutputFormat::Json),
                tenants: None
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
}

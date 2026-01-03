use crate::{
    format::{Formattable, FormattingError, OutputFormat},
    model::Tenant,
};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};
use tracing::debug;
use uuid::Uuid;

pub const DEFAULT_APPLICATION_ID: &str = "pcli2";
pub const DEFAULT_CONFIGURATION_FILE_NAME: &str = "config.yml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("failed to resolve the configuration directory")]
    FailedToFindConfigurationDirectory,
    #[error("failed to load configuration data, because of: {cause:?}")]
    FailedToLoadData { cause: Box<dyn std::error::Error> },
    #[error("failed to write configuration data to file, because of: {cause:?}")]
    FailedToWriteData { cause: Box<dyn std::error::Error> },
    #[error("missing value for property {name:?}")]
    MissingRequiredPropertyValue { name: String },
    #[error("{cause:?}")]
    FormattingError {
        #[from]
        cause: FormattingError,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(skip_serializing_if = "Option::is_none")]
    active_tenant_uuid: Option<Uuid>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            active_tenant_uuid: None,
        }
    }
}

impl Configuration {
    pub fn active_tenant_uuid(&self) -> Option<&Uuid> {
        self.active_tenant_uuid.as_ref()
    }

    pub fn get_default_configuration_file_path() -> Result<PathBuf, ConfigurationError> {
        // Check for PCLI2_CONFIG_DIR environment variable first
        if let Ok(config_dir_str) = std::env::var("PCLI2_CONFIG_DIR") {
            let mut config_path = PathBuf::from(config_dir_str);
            config_path.push(DEFAULT_CONFIGURATION_FILE_NAME);
            return Ok(config_path);
        }

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

    /// Load default configuration, creating a default one if none exists
    /// This is more user-friendly for first-time users
    pub fn load_or_create_default() -> Result<Configuration, ConfigurationError> {
        let default_file_path = Configuration::get_default_configuration_file_path()?;
        debug!(
            "Loading or creating configuration from {}...",
            default_file_path
                .clone()
                .into_os_string()
                .into_string()
                .unwrap()
        );

        // Try to load existing configuration
        match Configuration::load_from_file(default_file_path.clone()) {
            Ok(config) => Ok(config),
            Err(e) => {
                // Check if this is a "file not found" error
                match &e {
                    ConfigurationError::FailedToLoadData { cause } => {
                        if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
                            if io_err.kind() == std::io::ErrorKind::NotFound {
                                debug!(
                                    "Configuration file not found, creating default configuration"
                                );
                                let default_config = Configuration::default();

                                // Try to save the default configuration
                                match default_config.save(&default_file_path) {
                                    Ok(()) => {
                                        debug!("Default configuration created successfully");
                                        Ok(default_config)
                                    }
                                    Err(save_error) => {
                                        // If we can't save, return the original error with more context
                                        Err(ConfigurationError::FailedToLoadData {
                                            cause: Box::new(std::io::Error::other(
                                                format!("Configuration file not found and failed to create default configuration. Tried to create at: {:?}. Error: {}", 
                                                       default_file_path, save_error)
                                            ))
                                        })
                                    }
                                }
                            } else {
                                Err(e)
                            }
                        } else {
                            Err(e)
                        }
                    }
                    _ => Err(e),
                }
            }
        }
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

        let file = File::create(path);
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

    // Context management methods
    pub fn get_active_tenant_uuid(&self) -> Option<Uuid> {
        self.active_tenant_uuid.clone()
    }

    pub fn set_active_tenant(&mut self, tenant: &Tenant) {
        self.active_tenant_uuid = Some(tenant.uuid);
    }

    pub fn clear_active_tenant(&mut self) {
        self.active_tenant_uuid = None;
    }
}

impl Formattable for Configuration {
    fn format(&self, f: &OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            },
            OutputFormat::Csv(options) => {
                let uuid_str = self.active_tenant_uuid.unwrap_or_default().to_string();
                if options.with_headers {
                    Ok(format!("ACTIVE_TENANT_UUID\n{}", uuid_str))
                } else {
                    Ok(uuid_str)
                }
            },
            OutputFormat::Tree(_) => Err(FormattingError::UnsupportedOutputFormat(f.to_string())),
        }
    }
}

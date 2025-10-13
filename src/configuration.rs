use crate::format::{
    CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter,
};
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
        let records: Vec<Vec<String>> = vec![
            vec![
                self.active_tenant_id.clone().unwrap_or_default(),
                self.active_tenant_name.clone().unwrap_or_default(),
            ]
        ];

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
                wtr.write_record(Self::csv_header())
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
            OutputFormat::Tree => {
                // For configuration, tree format is the same as JSON
                let json = serde_json::to_string_pretty(self);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
        }
    }
}

impl Configuration {
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
                                debug!("Configuration file not found, creating default configuration");
                                let default_config = Configuration::default();
                                
                                // Try to save the default configuration
                                match default_config.save(&default_file_path) {
                                    Ok(()) => {
                                        debug!("Default configuration created successfully");
                                        Ok(default_config)
                                    },
                                    Err(save_error) => {
                                        // If we can't save, return the original error with more context
                                        Err(ConfigurationError::FailedToLoadData {
                                            cause: Box::new(std::io::Error::new(
                                                std::io::ErrorKind::Other,
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
                    },
                    _ => Err(e)
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
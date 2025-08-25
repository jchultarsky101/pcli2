use std::fs;
use std::path::PathBuf;
use dirs::config_dir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevKeyringError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Credentials {
    client_id: String,
    client_secret: String,
    access_token: Option<String>,
}

pub struct DevKeyring {
    file_path: PathBuf,
    credentials: Option<Credentials>,
}

impl Default for DevKeyring {
    fn default() -> DevKeyring {
        let mut file_path = config_dir().unwrap_or_else(|| PathBuf::from("."));
        file_path.push("pcli2");
        file_path.push("dev_credentials.json");
        
        DevKeyring {
            file_path,
            credentials: None,
        }
    }
}

impl DevKeyring {
    pub fn new() -> Self {
        Self::default()
    }

    fn load_credentials(&mut self) -> Result<(), DevKeyringError> {
        if self.file_path.exists() {
            let content = fs::read_to_string(&self.file_path)?;
            self.credentials = Some(serde_json::from_str(&content)?);
        } else {
            self.credentials = None;
        }
        Ok(())
    }

    fn save_credentials(&self) -> Result<(), DevKeyringError> {
        // Create directory if it doesn't exist
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        if let Some(credentials) = &self.credentials {
            let content = serde_json::to_string_pretty(credentials)?;
            fs::write(&self.file_path, content)?;
        }
        Ok(())
    }

    pub fn get(&mut self, _tenant: &String, key: String) -> Result<Option<String>, DevKeyringError> {
        self.load_credentials()?;
        
        if let Some(credentials) = &self.credentials {
            match key.as_str() {
                "client-id" => Ok(Some(credentials.client_id.clone())),
                "client-secret" => Ok(Some(credentials.client_secret.clone())),
                "access-token" => Ok(credentials.access_token.clone()),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn put(&mut self, _tenant: &String, key: String, value: String) -> Result<(), DevKeyringError> {
        self.load_credentials()?;
        
        let mut credentials = self.credentials.take().unwrap_or(Credentials {
            client_id: String::new(),
            client_secret: String::new(),
            access_token: None,
        });
        
        match key.as_str() {
            "client-id" => credentials.client_id = value,
            "client-secret" => credentials.client_secret = value,
            "access-token" => credentials.access_token = Some(value),
            _ => {} // Ignore unknown keys
        }
        
        self.credentials = Some(credentials);
        self.save_credentials()
    }

    pub fn delete(&mut self, _tenant: &String, key: String) -> Result<(), DevKeyringError> {
        self.load_credentials()?;
        
        if let Some(mut credentials) = self.credentials.take() {
            match key.as_str() {
                "access-token" => credentials.access_token = None,
                "client-id" => credentials.client_id = String::new(),
                "client-secret" => credentials.client_secret = String::new(),
                _ => {} // Ignore unknown keys
            }
            
            self.credentials = Some(credentials);
            self.save_credentials()?;
        }
        
        Ok(())
    }
}
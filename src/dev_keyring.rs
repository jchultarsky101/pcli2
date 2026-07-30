use dirs::config_dir;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing;

#[derive(Debug, Error)]
pub enum DevKeyringError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Restrict a credential file to its owner (`0600` on Unix).
///
/// This file holds the client secret and a live access token in plain text. Written
/// with the process umask it lands at `0644`, readable by every other account and
/// every unprivileged process on the machine. Anything stored here is only as private
/// as its mode bits, so the mode is set explicitly on every write rather than left to
/// whatever umask happened to be in effect when the file was first created.
///
/// Windows has no equivalent mode bits and inherits directory ACLs instead, so this is
/// a no-op there.
#[cfg(unix)]
fn restrict_to_owner(path: &std::path::Path) -> Result<(), DevKeyringError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &std::path::Path) -> Result<(), DevKeyringError> {
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EnvironmentCredentials {
    client_id: String,
    client_secret: String,
    access_token: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AllCredentials {
    environments: std::collections::HashMap<String, EnvironmentCredentials>,
}

pub struct DevKeyring {
    file_path: PathBuf,
    credentials: Option<AllCredentials>,
}

impl Default for DevKeyring {
    fn default() -> DevKeyring {
        // Try to get the config directory, fallback to current directory if it fails
        let config_base = config_dir().unwrap_or_else(|| {
            // If config_dir fails, try to create a .config directory in the home directory
            if let Some(home_dir) = dirs::home_dir() {
                let mut home_config = home_dir;
                home_config.push(".config");
                if home_config.exists() {
                    return home_config;
                } else {
                    // Create .config directory if it doesn't exist
                    if fs::create_dir_all(&home_config).is_ok() {
                        return home_config;
                    }
                }
            }
            // Fallback to current directory
            PathBuf::from(".")
        });

        let mut file_path = config_base;
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
            // Repair a file written before the mode was enforced, so an existing
            // world-readable credential file is tightened on first use rather than
            // waiting for the next login to rewrite it. A failure here is not worth
            // blocking the read over - the credentials are still usable, just as
            // exposed as they already were - so it is logged and stepped over.
            if let Err(e) = restrict_to_owner(&self.file_path) {
                tracing::warn!(
                    "Could not restrict permissions on '{}': {}. \
                     It may be readable by other users on this machine.",
                    self.file_path.display(),
                    e
                );
            }

            let content = fs::read_to_string(&self.file_path)?;
            match serde_json::from_str::<AllCredentials>(&content) {
                Ok(parsed_credentials) => {
                    self.credentials = Some(parsed_credentials);
                }
                Err(_) => {
                    // If parsing fails, start with empty credentials
                    self.credentials = Some(AllCredentials {
                        environments: std::collections::HashMap::new(),
                    });
                }
            }
        } else {
            self.credentials = Some(AllCredentials {
                environments: std::collections::HashMap::new(),
            });
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
            restrict_to_owner(&self.file_path)?;
        }
        Ok(())
    }

    pub fn get(&mut self, tenant: &str, key: String) -> Result<Option<String>, DevKeyringError> {
        // Don't fail if loading credentials fails, just return None
        if self.load_credentials().is_err() {
            // If we can't load credentials, return None for the requested key
            return match key.as_str() {
                "access-token" => Ok(None),
                "client-id" => Ok(None),
                "client-secret" => Ok(None),
                _ => Ok(None),
            };
        }

        if let Some(all_credentials) = &self.credentials {
            if let Some(env_credentials) = all_credentials.environments.get(tenant) {
                match key.as_str() {
                    "client-id" => {
                        tracing::debug!(
                            "Retrieved client-id from dev_keyring for environment: {}",
                            tenant
                        );
                        Ok(Some(env_credentials.client_id.clone()))
                    }
                    "client-secret" => {
                        tracing::debug!(
                            "Retrieved client-secret from dev_keyring for environment: {}",
                            tenant
                        );
                        Ok(Some(env_credentials.client_secret.clone()))
                    }
                    "access-token" => {
                        tracing::debug!(
                            "Retrieved access-token from dev_keyring for environment: {}",
                            tenant
                        );
                        Ok(env_credentials.access_token.clone())
                    }
                    _ => Ok(None),
                }
            } else {
                tracing::debug!(
                    "No credentials found in dev_keyring for environment: {}, key: {}",
                    tenant,
                    key
                );
                Ok(None)
            }
        } else {
            tracing::debug!(
                "No credentials found in dev_keyring for environment: {}, key: {}",
                tenant,
                key
            );
            Ok(None)
        }
    }

    pub fn put(&mut self, tenant: &str, key: String, value: String) -> Result<(), DevKeyringError> {
        tracing::debug!("Storing {} in dev_keyring for environment: {}", key, tenant);

        // Load existing credentials, but don't fail if the file doesn't exist or is corrupted
        let existing_credentials = match self.load_credentials() {
            Ok(_) => self.credentials.take(),
            Err(_) => None, // If loading fails, start with empty credentials
        };

        let mut all_credentials = existing_credentials.unwrap_or_else(|| AllCredentials {
            environments: std::collections::HashMap::new(),
        });

        // Get or create environment-specific credentials
        let env_credentials = all_credentials
            .environments
            .entry(tenant.to_string())
            .or_insert_with(|| EnvironmentCredentials {
                client_id: String::new(),
                client_secret: String::new(),
                access_token: None,
            });

        match key.as_str() {
            "client-id" => {
                env_credentials.client_id = value;
                tracing::debug!(
                    "Stored client-id in dev_keyring for environment: {}",
                    tenant
                );
            }
            "client-secret" => {
                env_credentials.client_secret = value;
                tracing::debug!(
                    "Stored client-secret in dev_keyring for environment: {}",
                    tenant
                );
            }
            "access-token" => {
                env_credentials.access_token = Some(value);
                tracing::debug!(
                    "Stored access-token in dev_keyring for environment: {}",
                    tenant
                );
            }
            _ => {} // Ignore unknown keys
        }

        self.credentials = Some(all_credentials);
        self.save_credentials()
    }

    pub fn delete(&mut self, tenant: &str, key: String) -> Result<(), DevKeyringError> {
        // Load existing credentials, but don't fail if the file doesn't exist or is corrupted
        let existing_credentials = match self.load_credentials() {
            Ok(_) => self.credentials.take(),
            Err(_) => None, // If loading fails, there's nothing to delete
        };

        if let Some(mut all_credentials) = existing_credentials {
            if let Some(env_credentials) = all_credentials.environments.get_mut(tenant) {
                match key.as_str() {
                    "access-token" => env_credentials.access_token = None,
                    "client-id" => env_credentials.client_id = String::new(),
                    "client-secret" => env_credentials.client_secret = String::new(),
                    _ => {} // Ignore unknown keys
                }
            }

            self.credentials = Some(all_credentials);
            self.save_credentials()?;
        }

        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A unique scratch path per test, since these touch the real filesystem.
    fn scratch_path() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pcli2_dev_keyring_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        path.push("dev_credentials.json");
        path
    }

    fn keyring_at(file_path: PathBuf) -> DevKeyring {
        DevKeyring {
            file_path,
            credentials: None,
        }
    }

    fn mode_of(path: &std::path::Path) -> u32 {
        fs::metadata(path)
            .expect("file exists")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn written_credentials_are_owner_only() {
        // The file holds a client secret and a live access token in plain text. Left
        // to the umask it lands at 0644 - readable by every other account on the box.
        let path = scratch_path();
        let mut keyring = keyring_at(path.clone());
        keyring
            .put("shared", "client-secret".to_string(), "s3cret".to_string())
            .expect("put should succeed");

        assert_eq!(mode_of(&path), 0o600, "credential file must be owner-only");

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_existing_world_readable_file_is_tightened_on_read() {
        // Files written before the mode was enforced must be repaired on first use,
        // not left exposed until the next login happens to rewrite them.
        let path = scratch_path();
        fs::create_dir_all(path.parent().unwrap()).expect("create scratch dir");
        fs::write(&path, r#"{"environments":{}}"#).expect("seed file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("loosen");
        assert_eq!(mode_of(&path), 0o644, "precondition: starts world-readable");

        let mut keyring = keyring_at(path.clone());
        let _ = keyring.get("shared", "client-id".to_string());

        assert_eq!(mode_of(&path), 0o600, "reading must repair the mode");

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_missing_file_does_not_error_on_read() {
        // The no-credentials-yet path must stay quiet rather than failing on a chmod
        // of a file that is not there.
        let mut keyring = keyring_at(scratch_path());
        assert!(keyring.get("shared", "client-id".to_string()).is_ok());
    }
}

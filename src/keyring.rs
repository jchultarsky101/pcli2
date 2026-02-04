#[cfg(not(feature = "dev-keyring"))]
mod implementation {
    use keyring::Entry;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum KeyringError {
        #[error("keyring error")]
        KeyringAccessError(#[from] KeyringErrorInternal),
    }

    impl From<keyring::Error> for KeyringError {
        fn from(error: keyring::Error) -> Self {
            KeyringError::KeyringAccessError(KeyringErrorInternal::CannotAccessKeyringEntity(error))
        }
    }

    #[derive(Debug, Error)]
    pub enum KeyringErrorInternal {
        #[error("keyring error")]
        CannotAccessKeyringEntity(#[from] keyring::Error),
    }

    pub struct Keyring {}

    impl Default for Keyring {
        fn default() -> Keyring {
            Keyring {}
        }
    }

    impl Keyring {
        pub fn get(&self, tenant: &str, key: String) -> Result<Option<String>, KeyringError> {
            let key = [tenant, key.as_str()].join(":");
            let entry = Entry::new("pcli2", key.as_str())?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(e) => match e {
                    keyring::Error::NoEntry => Ok(None),
                    _ => Err(KeyringError::from(e)),
                },
            }
        }

        pub fn put(&self, tenant: &str, key: String, value: String) -> Result<(), KeyringError> {
            let key = [tenant, key.as_str()].join(":");
            let entry = Entry::new("pcli2", key.as_str())?;
            entry.set_password(value.as_str())?;
            Ok(())
        }

        pub fn delete(&self, tenant: &str, key: String) -> Result<(), KeyringError> {
            let key = [tenant, key.as_str()].join(":");
            let entry = Entry::new("pcli2", key.as_str())?;
            entry.delete_password()?;
            Ok(())
        }

        /// Get multiple credential values for an environment in a single operation
        /// This helps reduce multiple keyring access calls that might trigger multiple authorization prompts
        pub fn get_environment_credentials(
            &self,
            tenant: &str,
        ) -> Result<(Option<String>, Option<String>, Option<String>), KeyringError> {
            let access_token = self.get(tenant, "access-token".to_string()).ok().flatten();
            let client_id = self.get(tenant, "client-id".to_string()).ok().flatten();
            let client_secret = self.get(tenant, "client-secret".to_string()).ok().flatten();

            Ok((access_token, client_id, client_secret))
        }
    }
}

#[cfg(feature = "dev-keyring")]
mod implementation {
    use crate::dev_keyring::DevKeyring;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum KeyringError {
        #[error("keyring error: {0}")]
        KeyringAccessError(String),
    }

    #[derive(Default)]
    pub struct Keyring {
        dev_keyring: DevKeyring,
    }

    impl Keyring {
        pub fn get(&mut self, tenant: &str, key: String) -> Result<Option<String>, KeyringError> {
            self.dev_keyring
                .get(tenant, key)
                .map_err(|e| KeyringError::KeyringAccessError(format!("{:?}", e)))
        }

        pub fn put(
            &mut self,
            tenant: &str,
            key: String,
            value: String,
        ) -> Result<(), KeyringError> {
            self.dev_keyring
                .put(tenant, key, value)
                .map_err(|e| KeyringError::KeyringAccessError(format!("{:?}", e)))
        }

        pub fn delete(&mut self, tenant: &str, key: String) -> Result<(), KeyringError> {
            self.dev_keyring
                .delete(tenant, key)
                .map_err(|e| KeyringError::KeyringAccessError(format!("{:?}", e)))
        }

        /// Get multiple credential values for an environment in a single operation
        /// This helps reduce multiple keyring access calls that might trigger multiple authorization prompts
        #[allow(clippy::type_complexity)]
        pub fn get_environment_credentials(
            &mut self,
            tenant: &str,
        ) -> Result<(Option<String>, Option<String>, Option<String>), KeyringError> {
            let access_token = self
                .dev_keyring
                .get(tenant, "access-token".to_string())
                .ok()
                .flatten();
            let client_id = self
                .dev_keyring
                .get(tenant, "client-id".to_string())
                .ok()
                .flatten();
            let client_secret = self
                .dev_keyring
                .get(tenant, "client-secret".to_string())
                .ok()
                .flatten();

            Ok((access_token, client_id, client_secret))
        }
    }
}

pub use implementation::*;

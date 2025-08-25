#[cfg(not(feature = "dev-keyring"))]
mod implementation {
    use keyring::Entry;
    use thiserror::Error;
    use tracing::error;

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
        pub fn get(&self, tenant: &String, key: String) -> Result<Option<String>, KeyringError> {
            let key = [tenant.clone(), key].join(":");
            let entry = Entry::new("pcli2", key.as_str())?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(e) => match e {
                    keyring::Error::NoEntry => Ok(None),
                    _ => Err(KeyringError::from(e)),
                },
            }
        }

        pub fn put(&self, tenant: &String, key: String, value: String) -> Result<(), KeyringError> {
            let key = [tenant.clone(), key].join(":");
            let entry = Entry::new("pcli2", key.as_str())?;
            entry.set_password(value.as_str())?;
            Ok(())
        }

        pub fn delete(&self, tenant: &String, key: String) -> Result<(), KeyringError> {
            let key = [tenant.clone(), key].join(":");
            let entry = Entry::new("pcli2", key.as_str())?;
            entry.delete_password()?;
            Ok(())
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

    pub struct Keyring {
        dev_keyring: DevKeyring,
    }

    impl Default for Keyring {
        fn default() -> Keyring {
            Keyring {
                dev_keyring: DevKeyring::default(),
            }
        }
    }

    impl Keyring {
        pub fn get(&mut self, tenant: &String, key: String) -> Result<Option<String>, KeyringError> {
            self.dev_keyring.get(tenant, key).map_err(|e| KeyringError::KeyringAccessError(format!("{:?}", e)))
        }

        pub fn put(&mut self, tenant: &String, key: String, value: String) -> Result<(), KeyringError> {
            self.dev_keyring.put(tenant, key, value).map_err(|e| KeyringError::KeyringAccessError(format!("{:?}", e)))
        }

        pub fn delete(&mut self, tenant: &String, key: String) -> Result<(), KeyringError> {
            self.dev_keyring.delete(tenant, key).map_err(|e| KeyringError::KeyringAccessError(format!("{:?}", e)))
        }
    }
}

pub use implementation::*;
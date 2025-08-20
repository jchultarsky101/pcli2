use crate::keyring::Keyring;
use thiserror::Error;

pub const API_KEY_ENTRY: &str = "api-key";

#[derive(Debug, Error)]
pub enum ApiKeyError {
    #[error("API key not found")]
    ApiKeyNotFound,
    #[error("Keyring error: {0}")]
    KeyringError(#[from] crate::keyring::KeyringError),
}

pub fn store_api_key(api_key: &str) -> Result<(), ApiKeyError> {
    let keyring = Keyring::default();
    keyring.put(&"default".to_string(), API_KEY_ENTRY.to_string(), api_key.to_string())?;
    Ok(())
}

pub fn get_api_key() -> Result<String, ApiKeyError> {
    let keyring = Keyring::default();
    match keyring.get(&"default".to_string(), API_KEY_ENTRY.to_string())? {
        Some(api_key) => Ok(api_key),
        None => Err(ApiKeyError::ApiKeyNotFound),
    }
}

pub fn delete_api_key() -> Result<(), ApiKeyError> {
    let keyring = Keyring::default();
    keyring.delete(&"default".to_string(), API_KEY_ENTRY.to_string())?;
    Ok(())
}
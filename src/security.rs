use super::configuration::TenantConfiguration;
use crate::client::*;
use jsonwebtoken::decode_header;
use keyring::Entry;
use thiserror::Error;
use tracing::{error, trace};

pub const SECRET_KEY: &str = "secret";
const TOKEN_KEY: &str = "token";

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("access denied")]
    AccessDenied,
    #[error("invalid credential")]
    InvalidCredentials,
    #[error("keyring error")]
    KeyrinError(#[from] KeyringError),
    #[error("failed to decode token")]
    FailedToDecodeToken,
    #[error("securiy error")]
    ConfigurationError {
        #[from]
        cause: crate::configuration::ConfigurationError,
    },
    #[error("client error")]
    RequestError(#[from] ClientError),
}

#[derive(Debug, Error)]
pub enum KeyringError {
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
    fn format_key(&self, tenant: String, key: String) -> String {
        [tenant, key].join(":").to_owned()
    }

    pub fn get(&self, tenant: &String, key: String) -> Result<Option<String>, KeyringError> {
        let key = self.format_key(tenant.to_owned(), key);
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
        let key = self.format_key(tenant.to_owned(), key);
        let entry = Entry::new("pcli2", key.as_str())?;
        entry.set_password(value.as_str())?;
        Ok(())
    }

    pub fn delete(&self, tenant: &String, key: String) -> Result<(), KeyringError> {
        let key = self.format_key(tenant.to_owned(), key);
        let entry = Entry::new("pcli2", key.as_str())?;
        entry.delete_password()?;
        Ok(())
    }
}

pub struct TenantSession {
    token: Option<String>,
}

impl TenantSession {
    pub fn token(&self) -> Option<String> {
        self.token.clone()
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token.to_owned());
    }

    fn get_token_from_keyring(tenant: &String) -> Result<Option<String>, SecurityError> {
        match Keyring::default().get(tenant, String::from(TOKEN_KEY))? {
            Some(token) => Ok(Some(token)),
            None => Ok(None),
        }
    }

    pub fn save_token_to_keyring(tenant: &String, token: &String) -> Result<(), SecurityError> {
        Keyring::default().put(tenant, String::from(TOKEN_KEY), token.to_owned())?;
        Ok(())
    }

    pub fn delete_token_from_keystore(tenant: &String) -> Result<(), SecurityError> {
        Keyring::default().delete(tenant, String::from(TOKEN_KEY))?;
        Ok(())
    }

    fn validate_token(token: &String) -> Result<String, SecurityError> {
        match decode_header(token) {
            Ok(_header) => return Ok(token.to_owned()),
            Err(_) => return Err(SecurityError::FailedToDecodeToken),
        }
    }

    async fn force_login(
        client: PhysnaHttpClient,
        tenant_config: TenantConfiguration,
    ) -> Result<TenantSession, SecurityError> {
        trace!("Logging in...");
        match Keyring::default().get(&tenant_config.tenant_id(), String::from(SECRET_KEY))? {
            Some(secret) => {
                let response = client.request_new_token_from_provider(secret).await;
                match response {
                    Ok(token) => {
                        Self::save_token_to_keyring(&tenant_config.tenant_id(), &token)?;
                        Ok(TenantSession { token: Some(token) })
                    }
                    Err(_) => Err(SecurityError::AccessDenied),
                }
            }
            None => Err(SecurityError::InvalidCredentials),
        }
    }

    /// Creates a new API session
    ///
    pub async fn login(tenant_config: TenantConfiguration) -> Result<TenantSession, SecurityError> {
        let tenant = tenant_config.tenant_id();
        trace!("Attemting to login for tenant \"{}\"...", &tenant);

        let client = PhysnaHttpClient::new(tenant_config.to_owned())?;
        let token = Self::get_token_from_keyring(&tenant)?;
        match token {
            Some(token) => {
                trace!("Found an existing token for this tenant. Validating...");
                match Self::validate_token(&token) {
                    Ok(token) => {
                        trace!("The existing token is still valid.");
                        Ok(TenantSession { token: Some(token) })
                    }
                    Err(_) => Self::force_login(client, tenant_config).await,
                }
            }
            None => Self::force_login(client, tenant_config).await,
        }
    }

    /// Invalidates the API session if one exists for this tenant
    ///
    pub fn logoff(tenant_config: TenantConfiguration) -> Result<(), SecurityError> {
        let tenant = tenant_config.tenant_id();
        trace!("Logging off for tenant \"{}\"...", &tenant);
        Self::delete_token_from_keystore(&tenant)?;
        Ok(())
    }
}

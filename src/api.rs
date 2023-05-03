use std::cell::RefCell;

use crate::{
    client::{self, PhysnaHttpClient},
    configuration::{Configuration, ConfigurationError},
    model::FolderList,
    security::{SecurityError, TenantSession},
};
use log::trace;

/// Error emmitted by the Api
///
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unknown tenant {tenant:?}")]
    UnknownTenant { tenant: String },
    #[error("configuration error, cause: {cause:?}")]
    ConfigurationError {
        #[from]
        cause: ConfigurationError,
    },
    #[error("security error, cause: {cause:?}")]
    SecurityError {
        #[from]
        cause: SecurityError,
    },
    #[error("invalid tenant {0}")]
    InvalidTenant(String),
    #[error("http error: {0}")]
    RequestError(#[from] client::ClientError),
    #[error("unsupported operation")]
    #[allow(dead_code)]
    UnsupportedOperation,
}

/// Physna API client
///
pub struct Api {
    configuration: RefCell<Configuration>,
}

impl Api {
    /// Creates a new instance of the API
    ///
    ///
    pub fn new(configuration: &RefCell<Configuration>) -> Api {
        Api {
            configuration: configuration.clone(),
        }
    }

    pub fn login(&self, tenant_id: &String) -> Result<TenantSession, ApiError> {
        let tenant_configuration = &self.configuration.borrow().tenant(tenant_id);
        match tenant_configuration {
            Some(tenant_configuration) => {
                let session = TenantSession::login(tenant_configuration.to_owned())?;
                Ok(session)
            }
            None => Err(ApiError::InvalidTenant(tenant_id.to_owned())),
        }
    }

    pub fn logoff(&self, tenant_id: &String) -> Result<(), ApiError> {
        let tenant_configuration = &self.configuration.borrow().tenant(tenant_id);
        match tenant_configuration {
            Some(tenant_configuration) => {
                TenantSession::logoff(tenant_configuration.to_owned())?;
                Ok(())
            }
            None => Err(ApiError::InvalidTenant(tenant_id.to_owned())),
        }
    }

    /// Returns the list of folders currently available for the specified tenant
    ///
    pub fn get_list_of_folders(
        &self,
        tenant_id: &String,
        retry: bool,
    ) -> Result<FolderList, ApiError> {
        trace!("Listing all folders for tenant \"{}\"...", tenant_id);

        let tenant_configuration = self.configuration.borrow().tenant(tenant_id);
        match tenant_configuration {
            Some(tenant_configuration) => {
                let mut session = TenantSession::login(tenant_configuration.to_owned())?;
                let client = PhysnaHttpClient::new(tenant_configuration)?;
                let response = client.get_list_of_folders(&mut session);
                let response = match response {
                    Ok(response) => response,
                    Err(e) => match e {
                        client::ClientError::Unauthorized => {
                            if retry {
                                self.logoff(tenant_id)?;
                                return self.get_list_of_folders(tenant_id, false);
                            } else {
                                return Err(ApiError::from(e));
                            }
                        }
                        _ => return Err(ApiError::from(e)),
                    },
                };

                // convert the HTTP response object to model object
                let folders = response.to_folder_list();
                Ok(folders)
            }
            None => Err(ApiError::InvalidTenant(tenant_id.to_owned())),
        }
    }
}

use std::{cell::RefCell, marker::PhantomData};

use crate::{
    client::{self, PhysnaHttpClient},
    configuration::{Configuration, ConfigurationError},
    model::{Folder, FolderList},
    security::{SecurityError, TenantSession},
};
use tracing::trace;

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

pub struct ApiUninitialized {}
pub struct ApiInitialized {}

/// Physna API client
///
pub struct Api<State = ApiUninitialized> {
    configuration: RefCell<Configuration>,
    state: PhantomData<State>,
}

impl Api<ApiUninitialized> {
    pub fn initialize(configuration: &RefCell<Configuration>) -> Api<ApiInitialized> {
        Api {
            configuration: configuration.clone(),
            state: PhantomData::<ApiInitialized>,
        }
    }
}

impl Api<ApiInitialized> {
    pub async fn login(&self, tenant_id: &String) -> Result<TenantSession, ApiError> {
        let tenant_configuration = &self.configuration.borrow().tenant(tenant_id);
        match tenant_configuration {
            Some(tenant_configuration) => {
                let session = TenantSession::login(tenant_configuration.to_owned()).await?;
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
    pub async fn get_list_of_folders(
        &self,
        tenant_id: &String,
        retry: bool,
    ) -> Result<FolderList, ApiError> {
        trace!("Listing all folders for tenant \"{}\"...", tenant_id);

        let tenant_configuration = self.configuration.borrow().tenant(tenant_id);
        match tenant_configuration {
            Some(tenant_configuration) => {
                let mut session = TenantSession::login(tenant_configuration.to_owned()).await?;
                let client = PhysnaHttpClient::new(tenant_configuration)?;
                let response = client.get_list_of_folders(&mut session).await;
                let response = match response {
                    Ok(response) => response,
                    Err(e) => match e {
                        client::ClientError::Unauthorized => {
                            if retry {
                                // retry if so specified
                                self.logoff(tenant_id)?;

                                client.get_list_of_folders(&mut session).await?
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

    /// Returns the list of folders currently available for the specified tenant
    ///
    pub async fn get_folder(
        &self,
        tenant_id: &String,
        folder_id: &u32,
        retry: bool,
    ) -> Result<Folder, ApiError> {
        trace!(
            "Retrieving folder details for tenant \"{}\", folder {}...",
            tenant_id,
            folder_id
        );

        let tenant_configuration = self.configuration.borrow().tenant(tenant_id);
        match tenant_configuration {
            Some(tenant_configuration) => {
                let mut session = TenantSession::login(tenant_configuration.to_owned()).await?;
                let client = PhysnaHttpClient::new(tenant_configuration)?;
                let response = client.get_folder(&mut session, folder_id).await;
                let response = match response {
                    Ok(response) => response,
                    Err(e) => match e {
                        client::ClientError::Unauthorized => {
                            if retry {
                                self.logoff(tenant_id)?;

                                client.get_folder(&mut session, folder_id).await?
                            } else {
                                return Err(ApiError::from(e));
                            }
                        }
                        _ => return Err(ApiError::from(e)),
                    },
                };

                // convert the HTTP response object to model object
                let folder = response.to_folder();
                Ok(folder)
            }
            None => Err(ApiError::InvalidTenant(tenant_id.to_owned())),
        }
    }
}

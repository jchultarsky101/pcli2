use std::cell::RefCell;

use crate::{
    configuration::{Configuration, ConfigurationError},
    model::{Folder, FolderList},
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
    #[error("unsupported operation")]
    #[allow(dead_code)]
    UnsupportedOperation,
}

pub struct UnauthorizedApi {}
pub struct AuthorizedApi {}

/// Physna API client
///
pub struct Api<State = UnauthorizedApi> {
    configuration: RefCell<Configuration>,
    state: std::marker::PhantomData<State>,
}

impl Api {
    /// Creates a new instance of the API
    ///
    ///
    pub fn new(configuration: &RefCell<Configuration>) -> Api {
        Api {
            configuration: configuration.clone(),
            state: std::marker::PhantomData::<UnauthorizedApi>,
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
    pub fn list_folders(&self, tenant_id: &String) -> Result<FolderList, ApiError> {
        trace!("Listing all folders for tenant \"{}\"...", tenant_id);
        let _tenant = self.configuration.borrow().validate_tenant(tenant_id)?;

        let mut folders = FolderList::empty();
        folders.insert(
            Folder::builder()
                .id(1)
                .name(&"first folder".to_string())
                .build()
                .unwrap(),
        );
        folders.insert(
            Folder::builder()
                .id(2)
                .name(&"second folder".to_string())
                .build()
                .unwrap(),
        );

        Ok(folders.clone())
        // Err(ApiError::UnsupportedOperation)
    }
}

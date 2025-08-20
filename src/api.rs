use std::{cell::RefCell, marker::PhantomData};

use crate::{
    configuration::{Configuration, ConfigurationError},
    model::{Folder, FolderList},
    keyring::{KeyringError, TenantSession},
};
use tracing::trace;

/// Error emmitted by the Api
///
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("configuration error, cause: {cause:?}")]
    ConfigurationError {
        #[from]
        cause: ConfigurationError,
    },
    #[error("keyring error, cause: {cause:?}")]
    KeyringError {
        #[from]
        cause: KeyringError,
    },
    #[error("http error: {0}")]
    RequestError(#[from] crate::client::ClientError),
    #[error("unsupported operation")]
    #[allow(dead_code)]
    UnsupportedOperation,
}

pub struct ApiUninitialized {}
pub struct ApiInitialized {}

/// Physna API client
///
pub struct Api<State = ApiUninitialized> {
    state: PhantomData<State>,
}

impl Api<ApiUninitialized> {
    pub fn initialize(_configuration: &RefCell<Configuration>) -> Api<ApiInitialized> {
        Api {
            state: PhantomData::<ApiInitialized>,
        }
    }
}

impl Api<ApiInitialized> {
    pub async fn login(&self, tenant_id: &String) -> Result<TenantSession, ApiError> {
        // This is a placeholder implementation since we're moving to Physna V3 API
        println!("Would login to tenant: {} (but using Physna V3 API approach)", tenant_id);
        Ok(TenantSession::default())
    }

    pub fn logoff(&self, tenant_id: &String) -> Result<(), ApiError> {
        // This is a placeholder implementation since we're moving to Physna V3 API
        println!("Would logoff from tenant: {} (but using Physna V3 API approach)", tenant_id);
        Ok(())
    }

    /// Returns the list of folders currently available for the specified tenant
    ///
    pub async fn get_list_of_folders(
        &self,
        tenant_id: &String,
        _retry: bool,
    ) -> Result<FolderList, ApiError> {
        trace!("Listing all folders for tenant \"{}\"...", tenant_id);
        // This is a placeholder implementation since we're moving to Physna V3 API
        println!("Would list folders for tenant: {} (but using Physna V3 API approach)", tenant_id);
        Ok(FolderList::empty())
    }

    /// Returns the list of folders currently available for the specified tenant
    ///
    pub async fn get_folder(
        &self,
        tenant_id: &String,
        folder_id: &u32,
        _retry: bool,
    ) -> Result<Folder, ApiError> {
        trace!(
            "Retrieving folder details for tenant \"{}\", folder {}...",
            tenant_id,
            folder_id
        );
        // This is a placeholder implementation since we're moving to Physna V3 API
        println!("Would get folder {} for tenant: {} (but using Physna V3 API approach)", folder_id, tenant_id);
        Ok(Folder::new(*folder_id, "Sample Folder".to_string()))
    }
}

use std::{cell::RefCell, marker::PhantomData};

use crate::{
    configuration::{Configuration, ConfigurationError},
    model::{Folder, FolderList},
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
        Ok(Folder::new(*folder_id, "unknown".to_string(), "Sample Folder".to_string()))
    }
}
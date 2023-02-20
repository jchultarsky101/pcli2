use std::cell::RefCell;

use crate::{
    configuration::Configuration,
    model::{Folder, FolderList},
};
use log::trace;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unsupported operation")]
    UnsupportedOperation,
}

pub struct Api {
    configuration: RefCell<Configuration>,
}

impl Api {
    pub fn new(configuration: &RefCell<Configuration>) -> Api {
        Api {
            configuration: configuration.clone(),
        }
    }

    pub fn get_all_folders(&self, tenant: &String) -> Result<FolderList, ApiError> {
        trace!("Listing all folders for tenant {}...", tenant);

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

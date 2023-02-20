use crate::model::{Folder, FolderList};
use log::trace;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unsupported operation")]
    UnsupportedOperation,
}

pub struct Api {}

impl Api {
    pub fn get_all_folders() -> Result<FolderList, ApiError> {
        trace!("Listing all folders...");

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

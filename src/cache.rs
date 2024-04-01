use crate::{configuration::Configuration, model::FolderList};
use bincode;
use cacache;
use log;
use std::{cell::RefCell, path::PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("failed to cache data")]
    CacacheError(#[from] cacache::Error),
}

#[derive(Debug, Clone)]
pub struct Cache {
    path: Option<PathBuf>,
}

impl Cache {
    pub fn new(configuration: &RefCell<Configuration>) -> Self {
        log::debug!("Configuration: {:?}", configuration.borrow());
        Self {
            path: configuration.borrow().get_cache_path(),
        }
    }

    fn get_folders_key(&self, tenant_id: &str) -> String {
        let mut key: String = tenant_id.to_owned();
        key.push_str(".folders");
        key.to_owned()
    }

    pub async fn get_folders(&self, tenant_id: &str) -> Option<FolderList> {
        if self.path.as_ref().is_some() {
            let folders: Option<FolderList> =
                match cacache::read(self.path.as_ref().unwrap(), self.get_folders_key(tenant_id))
                    .await
                    .ok()
                {
                    Some(encoded) => {
                        let decoded: FolderList = bincode::deserialize(&encoded).unwrap();
                        Some(decoded)
                    }
                    None => None,
                };

            folders
        } else {
            log::warn!("Cache path is None");
            None
        }
    }

    pub async fn save_folders(
        &self,
        tenant_id: &str,
        folders: &FolderList,
    ) -> Result<(), CacheError> {
        if self.path.as_ref().is_some() {
            log::trace!(
                "💾 Saving folders to cache at {}",
                self.path
                    .clone()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap()
            );
            let encoded = bincode::serialize(&folders).unwrap();
            cacache::write(
                self.path.as_ref().unwrap(),
                self.get_folders_key(tenant_id),
                encoded,
            )
            .await?;
        }
        Ok(())
    }
}

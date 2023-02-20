use std::cell::RefCell;

use crate::{
    configuration::{Configuration, TenantConfiguration},
    model::{Folder, FolderList},
};
use log::trace;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unknown tenant {tenant:?}")]
    UnknownTenant { tenant: String },
    #[error("unsupported operation")]
    #[allow(dead_code)]
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

    fn validate_tenant(&self, tenant_id: &String) -> Result<TenantConfiguration, ApiError> {
        trace!("Validating tenant ID of \"{}\"...", tenant_id);
        match self.configuration.borrow().tenant(tenant_id) {
            Some(tenant) => {
                trace!("Tenant ID {} is valid.", tenant_id);
                Ok(tenant)
            }
            None => Err(ApiError::UnknownTenant {
                tenant: tenant_id.clone(),
            }),
        }
    }

    pub fn get_all_folders(&self, tenant_id: &String) -> Result<FolderList, ApiError> {
        trace!("Listing all folders for tenant \"{}\"...", tenant_id);
        let _tenant = self.validate_tenant(tenant_id)?;

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

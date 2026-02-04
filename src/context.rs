//! Context builder for the Physna CLI client.
//!
//! This module provides a centralized way to initialize and manage the common
//! execution context that most CLI commands need, including configuration,
//! API client, and tenant information.

use crate::{
    configuration::Configuration,
    error::CliError,
    model::Tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use uuid::Uuid;

/// Execution context containing common resources needed by CLI commands.
pub struct ExecutionContext {
    pub configuration: Configuration,
    pub api: PhysnaApiClient,
    pub tenant: Tenant,
}

impl ExecutionContext {
    /// Create a new execution context from command-line arguments.
    ///
    /// This method handles the common initialization pattern:
    /// 1. Load configuration
    /// 2. Create API client
    /// 3. Resolve tenant
    ///
    /// # Arguments
    ///
    /// * `sub_matches` - The command-line argument matches containing the command parameters
    ///
    /// # Returns
    ///
    /// * `Ok(ExecutionContext)` - The initialized execution context
    /// * `Err(CliError)` - If initialization fails
    pub async fn from_args(sub_matches: &ArgMatches) -> Result<Self, CliError> {
        let configuration = Configuration::load_or_create_default()?;
        let mut api = PhysnaApiClient::try_default()?;
        let tenant = crate::param_utils::get_tenant(&mut api, sub_matches, &configuration).await?;

        Ok(ExecutionContext {
            configuration,
            api,
            tenant,
        })
    }

    /// Get a reference to the tenant UUID.
    pub fn tenant_uuid(&self) -> &Uuid {
        &self.tenant.uuid
    }

    /// Get a mutable reference to the API client.
    pub fn api(&mut self) -> &mut PhysnaApiClient {
        &mut self.api
    }

    /// Get a reference to the configuration.
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    /// Get a reference to the tenant.
    pub fn tenant(&self) -> &Tenant {
        &self.tenant
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_context_builder_exists() {
        // This test verifies that the ExecutionContext struct exists
        // More comprehensive tests would require mocking
    }
}

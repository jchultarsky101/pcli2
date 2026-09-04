use crate::{
    commands::params::PARAMETER_TENANT, configuration::Configuration, error::CliError,
    format::OutputFormat, model::Tenant, physna_v3::PhysnaApiClient,
};
use clap::ArgMatches;
use tracing::{debug, trace};
use uuid::Uuid;

/// Resolve a tenant by name
///
/// This function handles the case where users provide a tenant name
/// via the --tenant parameter, and resolves names to UUID by
/// calling the list_tenants API endpoint.
///
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant_name` - The tenant name
///
/// # Returns
/// * `Ok(Tenant)` - The resolved tenant
/// * `Err(CliError)` - If the tenant cannot be found
async fn resolve_tenant_by_name(
    client: &mut PhysnaApiClient,
    tenant_name: &String,
) -> Result<Tenant, CliError> {
    debug!("Resolving tenant by name: {}", tenant_name);

    // The cached list first; on a miss, one fresh fetch, since a tenant granted
    // since the cache was written is the common reason for a miss.
    for refresh in [false, true] {
        let tenants = crate::tenant_cache::TenantCache::get_all_tenants(client, refresh).await?;
        if let Some(tenant) = tenants.iter().find(|t| t.tenant_short_name.eq(tenant_name)) {
            return Ok(tenant.try_into()?);
        }
    }
    Err(CliError::TenantNotFound {
        identifier: tenant_name.to_owned(),
    })
}

pub async fn get_format_parameter_value(sub_matches: &ArgMatches) -> OutputFormat {
    trace!("Resolving output format options...");

    // Use the new format utilities for consistent handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);

    trace!("Format: {}", format_params.format_str);
    trace!(
        "With headers: {}",
        format_params.format_options.with_headers
    );
    trace!(
        "With metadata: {}",
        format_params.format_options.with_metadata
    );
    trace!("Pretty: {}", format_params.format_options.pretty);

    format_params.format
}

pub async fn resolve_tenant_by_uuid(
    client: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
) -> Result<Tenant, CliError> {
    debug!("Resolving tenant by UID: {}", tenant_uuid);

    for refresh in [false, true] {
        let tenants = crate::tenant_cache::TenantCache::get_all_tenants(client, refresh).await?;
        if let Some(tenant) = tenants.iter().find(|t| t.tenant_uuid.eq(tenant_uuid)) {
            return Ok(tenant.try_into()?);
        }
    }
    Err(CliError::TenantNotFound {
        identifier: tenant_uuid.to_string(),
    })
}

/// Helper function to get tenant from parameter or configuration with resolution
pub async fn get_tenant(
    client: &mut PhysnaApiClient,
    sub_matches: &ArgMatches,
    configuration: &Configuration,
) -> Result<Tenant, CliError> {
    match sub_matches.get_one::<String>(PARAMETER_TENANT) {
        Some(tenant_name) => {
            // The help text promises "ID or alias"; honour both.
            let tenant = match Uuid::parse_str(tenant_name) {
                Ok(uuid) => resolve_tenant_by_uuid(client, &uuid).await?,
                Err(_) => resolve_tenant_by_name(client, tenant_name).await?,
            };
            Ok(tenant)
        }
        None => {
            if let Some(active_tenant_uuid) = configuration.get_active_tenant_uuid() {
                let tenant = resolve_tenant_by_uuid(client, &active_tenant_uuid).await?;
                Ok(tenant)
            } else {
                Err(CliError::MissingRequiredArgument("No tenant specified and no active tenant selected. Use 'pcli2 tenant use' to select a tenant, or specify a tenant with --tenant.".to_string()))
            }
        }
    }
}

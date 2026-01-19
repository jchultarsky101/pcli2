use clap::ArgMatches;
use uuid::Uuid;
use crate::{commands::params::{PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_METADATA, PARAMETER_PRETTY, PARAMETER_TENANT}, configuration::Configuration, error::CliError, format::{OutputFormat, OutputFormatOptions}, model::Tenant, physna_v3::PhysnaApiClient};
use tracing::{debug, trace};


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
    
    // First, try to list all tenants to see if we can resolve the identifier
    let tenants = crate::tenant_cache::TenantCache::get_all_tenants(client, false).await?;

    // Look for an exact match by tenant ID first
    match tenants.iter().find(|t| t.tenant_short_name.eq(tenant_name)) {
        Some(tenant) => Ok(tenant.try_into()?),
        None => Err(CliError::TenantNotFound {identifier: tenant_name.to_owned(),}),
    }
}

pub async fn get_format_parameter_value(sub_matches: &ArgMatches) -> OutputFormat {

    trace!("Resolving output format options...");

    // Get format parameter - Clap handles environment variable precedence automatically
    // 1. User specified --format (explicit command line)
    // 2. Environment variable PCLI2_FORMAT (when no explicit format provided)
    // 3. Default value of "json" (set in the argument definition)
    let format_string = sub_matches.get_one::<String>(PARAMETER_FORMAT).unwrap().clone();

    let with_metadata = sub_matches.get_flag(PARAMETER_METADATA);

    // Get headers flag - Clap handles environment variable precedence automatically
    // 1. User specified --headers (explicit command line)
    // 2. Environment variable PCLI2_HEADERS (when no explicit headers flag provided)
    // 3. Default value of false (implicit for boolean flags)
    let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);

    let pretty = sub_matches.get_flag(PARAMETER_PRETTY);

    trace!("Format: {}", format_string);
    trace!("With headers: {}", with_headers);
    trace!("With metadata: {}", with_metadata);
    trace!("Pretty: {}", pretty);

    let options = OutputFormatOptions { with_metadata, with_headers, pretty };

    // Using clap, we allow only valid values for the --format parameter. Because of that it is safe to unwrap.
    OutputFormat::from_string_with_options(&format_string, options).unwrap().to_owned()
}

pub async fn resolve_tenant_by_uuid(
    client: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
) -> Result<Tenant, CliError> {
    debug!("Resolving tenant by UID: {}", tenant_uuid);
    
    // First, try to list all tenants to see if we can resolve the identifier
    let tenants = crate::tenant_cache::TenantCache::get_all_tenants(client, false).await?;

    // Look for an exact match by tenant ID first
    match tenants.iter().find(|t| t.tenant_uuid.eq(tenant_uuid)) {
        Some(tenant) => Ok(tenant.try_into()?),
        None => Err(CliError::TenantNotFound {identifier: tenant_uuid.to_string(),}),
    }
}

/// Helper function to get tenant from parameter or configuration with resolution
pub async fn get_tenant(
    client: &mut PhysnaApiClient,
    sub_matches: &ArgMatches,
    configuration: &Configuration,
) -> Result<Tenant, CliError> {
    match sub_matches.get_one::<String>(PARAMETER_TENANT) {
        Some(tenant_name) => {
            let tenant = resolve_tenant_by_name(client, tenant_name).await?;
            Ok(tenant)
        },
        None => {
            if let Some(active_tenant_uuid) = configuration.get_active_tenant_uuid() {
                let tenant = resolve_tenant_by_uuid(client, &active_tenant_uuid).await?;
                Ok(tenant)
            } else {
                return Err(CliError::MissingRequiredArgument("No tenant specified and no active tenant selected. Use 'pcli2 context set tenant' to select a tenant, or specify a tenant with --tenant.".to_string()));
            }
        }
    }
}

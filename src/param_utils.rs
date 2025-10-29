//! Parameter parsing utilities for the Physna CLI client.
//!
//! This module provides common parameter parsing and validation utilities
//! to improve consistency across different command handlers and reduce code duplication.

use std::str::FromStr;
use crate::physna_v3::{PhysnaApiClient, ApiError};
use crate::format::OutputFormat;
use crate::configuration::Configuration;
use clap::ArgMatches;

/// Helper function to safely extract and parse format parameter with default fallback
pub fn extract_format_param_with_default(sub_matches: &ArgMatches) -> Result<OutputFormat, ApiError> {
    let format_str = sub_matches.get_one::<String>("format")
        .cloned()
        .unwrap_or_else(|| "json".to_string());
    
    OutputFormat::from_str(&format_str)
        .map_err(|e| ApiError::ConflictError(
            format!("Unsupported output format: {}: {}", format_str, e)
        ))
}

/// Helper function to get tenant from parameter or configuration with resolution
pub async fn get_tenant_id(
    client: &mut PhysnaApiClient,
    sub_matches: &ArgMatches,
    configuration: &Configuration,
) -> Result<String, ApiError> {
    let tenant_identifier = match sub_matches.get_one::<String>("tenant") {
        Some(tenant_id) => tenant_id.clone(),
        None => {
            if let Some(active_tenant_id) = configuration.get_active_tenant_id() {
                active_tenant_id
            } else {
                return Err(ApiError::ConflictError("Missing required argument: tenant".to_string()));
            }
        }
    };
    
    resolve_tenant_identifier_to_id(client, tenant_identifier).await
}

/// Resolve a tenant name or ID to a tenant ID
/// 
/// This function handles the case where users provide either a tenant name or ID
/// via the --tenant parameter. It checks if the provided identifier looks like
/// a UUID (tenant ID) or a human-readable name, and resolves names to IDs by
/// calling the list_tenants API endpoint.
/// 
/// # Arguments
/// * `client` - The Physna API client
/// * `tenant_identifier` - The tenant name or ID to resolve
/// 
/// # Returns
/// * `Ok(String)` - The resolved tenant ID
/// * `Err(ApiError)` - If the tenant cannot be found
async fn resolve_tenant_identifier_to_id(
    client: &mut PhysnaApiClient,
    tenant_identifier: String,
) -> Result<String, ApiError> {
    tracing::debug!("Resolving tenant identifier: {}", tenant_identifier);
    
    // First, try to list all tenants to see if we can resolve the identifier
    let tenants = client.list_tenants().await.map_err(|e| {
        ApiError::RetryFailed(format!("Failed to list tenants during resolution: {}", e))
    })?;
    
    // Look for an exact match by tenant ID first
    for tenant in &tenants {
        if tenant.tenant_id == tenant_identifier {
            tracing::debug!("Tenant identifier {} appears to be a direct ID match", tenant_identifier);
            return Ok(tenant.tenant_id.clone());
        }
    }
    
    // Then look for a match by name
    for tenant in &tenants {
        if tenant.tenant_display_name == tenant_identifier || 
           tenant.tenant_short_name.as_str() == tenant_identifier {
            tracing::debug!("Resolved tenant identifier '{}' to ID '{}'", tenant_identifier, tenant.tenant_id);
            return Ok(tenant.tenant_id.clone());
        }
    }
    
    // If we can't find the tenant, return an error
    Err(ApiError::ConflictError(format!("Tenant '{}' not found", tenant_identifier)))
}

/// Helper function to extract a required string parameter
pub fn extract_required_param(sub_matches: &ArgMatches, param_name: &str) -> Result<String, ApiError> {
    sub_matches.get_one::<String>(param_name)
        .cloned()
        .ok_or_else(|| ApiError::ConflictError(format!("Missing required argument: {}", param_name)))
}

/// Helper function to extract an optional string parameter with a default value
pub fn extract_optional_param_with_default(sub_matches: &ArgMatches, param_name: &str, default: &str) -> String {
    sub_matches.get_one::<String>(param_name)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// Helper function to extract a parameter that has been parsed as a specific type
pub fn extract_typed_param<T: Clone + Send + Sync + 'static>(sub_matches: &ArgMatches, param_name: &str) -> Result<T, ApiError> {
    sub_matches.get_one::<T>(param_name)
        .cloned()
        .ok_or_else(|| ApiError::ConflictError(format!("Missing required argument: {}", param_name)))
}

/// Helper function to extract an optional typed parameter
pub fn extract_optional_typed_param<T: Clone + Send + Sync + 'static>(sub_matches: &ArgMatches, param_name: &str) -> Option<T> {
    sub_matches.get_one::<T>(param_name).cloned()
}

/// Helper function to validate a threshold parameter (0.0 to 100.0)
pub fn validate_threshold_param(threshold: f64) -> Result<(), ApiError> {
    if !(0.0..=100.0).contains(&threshold) {
        Err(ApiError::ConflictError("Threshold must be between 0.00 and 100.00".to_string()))
    } else {
        Ok(())
    }
}

/// Helper function to validate a parameter that is required when a condition is met
pub fn validate_conditional_requirement(
    condition: bool,
    param_value: Option<&String>,
    param_name: &str,
) -> Result<(), ApiError> {
    if condition && param_value.is_none() {
        Err(ApiError::ConflictError(format!("Missing required argument: {}", param_name)))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_optional_param_with_default() {
        // Create a mock ArgMatches for testing
        use clap::{Arg, Command};
        let cmd = Command::new("test")
            .arg(Arg::new("test_arg").long("test-arg"));
        let matches = cmd.try_get_matches_from(vec!["test"]).unwrap();
        
        // Test with default when argument is not provided
        assert_eq!(extract_optional_param_with_default(&matches, "test_arg", "default"), "default");
    }

    #[test]
    fn test_validate_threshold_param() {
        assert!(validate_threshold_param(50.0).is_ok());
        assert!(validate_threshold_param(0.0).is_ok());
        assert!(validate_threshold_param(100.0).is_ok());
        assert!(validate_threshold_param(-1.0).is_err());
        assert!(validate_threshold_param(101.0).is_err());
    }

    #[test]
    fn test_validate_threshold_edge_cases() {
        assert!(validate_threshold_param(0.0).is_ok());
        assert!(validate_threshold_param(100.0).is_ok());
        assert!(validate_threshold_param(-0.1).is_err());
        assert!(validate_threshold_param(100.1).is_err());
    }

    #[test]
    fn test_extract_format_param_with_default() {
        use clap::{Arg, Command};
        
        let cmd = Command::new("test")
            .arg(Arg::new("format").long("format").default_value("json"));
        let matches = cmd.try_get_matches_from(vec!["test", "--format", "csv"]).unwrap();
        
        // Test that it doesn't panic and returns a valid result
        let result = extract_format_param_with_default(&matches);
        assert!(result.is_ok());
    }
}
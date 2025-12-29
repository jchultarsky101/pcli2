use clap::ArgMatches;
use std::str::FromStr;
use tracing::trace;
use crate::{
    commands::params::{PARAMETER_FORMAT, PARAMETER_NAME},
    configuration::Configuration,
    error_utils,
    format::OutputFormat, model::Tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
    actions::CliActionError
};


pub async fn list_all_tenants(sub_matches: &ArgMatches) -> Result<(), CliActionError> {
    
    let format_str = sub_matches.get_one::<String>(PARAMETER_FORMAT).cloned().unwrap_or_else(|| "json".to_string());
    let format = OutputFormat::from_str(&format_str).unwrap();
    let mut api = PhysnaApiClient::try_default()?;

    match api.list_tenants().await {
        Ok(tenants) => {
            match format {
                OutputFormat::Json(_) => {
                    // For JSON format, output a single array containing all tenants
                    let json = serde_json::to_string_pretty(&tenants)?;
                    println!("{}", json);
                }
                OutputFormat::Csv(_) => {
                    // For CSV format, output header with both tenant name and UUID columns
                    let mut wtr = csv::Writer::from_writer(vec![]);
                    wtr.write_record(&["TENANT_NAME", "TENANT_UUID"])?;

                    for tenant in tenants {
                        let tenant = Tenant::try_from(&tenant)?;
                        wtr.serialize(tenant)?;
                    }

                    let csv_string = String::from_utf8(wtr.into_inner()?)?;
                    println!("{}", csv_string);
                }
                OutputFormat::Tree(_) => {
                    return Err(CliActionError::UnsupportedOutputFormat(format_str))
                }
            }
            Ok(())
        }
        Err(e) => {
            error_utils::report_error(&e);
            Ok(())
        }
    }
}

pub async fn print_active_tenant_name() -> Result<(), CliActionError> {
    trace!("Executing 'context get' command");

    let configuration = Configuration::load_default()?;

    if let Some(active_tenant_uuid) = configuration.active_tenant_uuid() {
        let mut api = PhysnaApiClient::try_default()?;
        let tenants = api.list_tenants().await?;
        let active_tenant = tenants.into_iter().find(|t| t.tenant_uuid.eq(active_tenant_uuid));
        match active_tenant {
            Some(active_tenant) => {
                println!("{}", active_tenant.tenant_short_name);
            },
            None => {
                println!("No active tenant selected");
            }
        }
    } else {
        println!("No active tenant selected");
    }

    Ok(())
}

pub async fn set_active_tenant(sub_matches: &ArgMatches) -> Result<(), CliActionError> {

    let name = sub_matches.get_one::<String>(PARAMETER_NAME);
    let mut api = PhysnaApiClient::try_default()?;
    let tenants = api.list_tenants().await?;
            
    // If no name was provided, show interactive selection
    let selected_tenant = if let Some(name) = name {
        // Find tenant by name (existing logic)
        tenants.iter().find(|t| t.tenant_short_name == *name).cloned()
    } else {
        // Interactive selection using TUI
        if tenants.is_empty() {
            eprintln!("No tenants available");
            return Ok(());
        }
        
        // Create options for the select menu
        let options: Vec<String> = tenants.iter()
            .map(|tenant| format!("{}: {} ({})", tenant.tenant_short_name, tenant.tenant_display_name, tenant.tenant_uuid))
            .collect();
        
        // Use inquire to create an interactive selection
        let ans = inquire::Select::new("Select a tenant:", options)
            .with_help_message("Choose the tenant you want to set as active")
            .prompt();
            
        match ans {
            Ok(choice) => {
                let tenant_name = choice.split_once(':').map(|(before, _)| before.trim()).unwrap();
                trace!("User selected tenant: {}", tenant_name);
                // Find the tenant that matches the selection
                tenants.iter().find(|t| t.tenant_short_name == tenant_name).cloned()
            }
            Err(_) => {
                eprintln!("No tenant selected");
                return Ok(());
            }
        }
    };
                    
    // Set the active tenant in configuration
    let mut configuration = Configuration::load_default()?;
    if let Some(selected_tenant) = selected_tenant {
        let tenant = Tenant::try_from(&selected_tenant)?;
        configuration.set_active_tenant(&tenant);
                
        // Save configuration
        configuration.save_to_default()?;
    } else {
            eprintln!("Tenant '{}' not found", name.unwrap()); // Safe to unwrap since we checked above
    }

    Ok(())
}

pub async fn clear_active_tenant() -> Result<(), CliActionError> {
    
    let mut configuration = Configuration::load_default()?;
    configuration.clear_active_tenant();
    match configuration.save_to_default() {
        Ok(()) => {
            Ok(())
        }
        Err(e) => {
            eprintln!("Error saving configuration: {}", e);
            Err(CliActionError::ConfigurationError(e))
        }
    }
}

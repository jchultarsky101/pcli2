use clap::ArgMatches;
use tracing::trace;
use crate::{
    commands::params::{PARAMETER_NAME},
    configuration::Configuration,
    error_utils,
    format::{OutputFormat, Formattable, FormattingError, OutputFormatter},
    model::Tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
    actions::CliActionError
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextInfo {
    pub active_tenant_uuid: Option<uuid::Uuid>,
    pub active_tenant_short_name: Option<String>,
    pub active_tenant_display_name: Option<String>,
}

impl ContextInfo {
    pub async fn from_configuration(configuration: &Configuration) -> Result<ContextInfo, CliActionError> {
        if let Some(active_tenant_uuid) = configuration.active_tenant_uuid() {
            let mut api = PhysnaApiClient::try_default()?;
            let tenants = api.list_tenants().await?;
            let active_tenant = tenants.into_iter().find(|t| t.tenant_uuid.eq(active_tenant_uuid));

            match active_tenant {
                Some(active_tenant) => {
                    Ok(ContextInfo {
                        active_tenant_uuid: Some(active_tenant.tenant_uuid),
                        active_tenant_short_name: Some(active_tenant.tenant_short_name),
                        active_tenant_display_name: Some(active_tenant.tenant_display_name),
                    })
                },
                None => {
                    Ok(ContextInfo {
                        active_tenant_uuid: Some(*active_tenant_uuid),
                        active_tenant_short_name: None,
                        active_tenant_display_name: None,
                    })
                }
            }
        } else {
            Ok(ContextInfo {
                active_tenant_uuid: None,
                active_tenant_short_name: None,
                active_tenant_display_name: None,
            })
        }
    }
}

impl Formattable for ContextInfo {
    fn format(&self, f: &OutputFormat) -> Result<String, FormattingError> {
        match f {
            OutputFormat::Json(options) => {
                if options.pretty {
                    Ok(serde_json::to_string_pretty(self)?)
                } else {
                    Ok(serde_json::to_string(self)?)
                }
            }
            OutputFormat::Csv(options) => {
                // For CSV format, output with or without header based on options
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.write_record(&["ACTIVE_TENANT_UUID", "ACTIVE_TENANT_SHORT_NAME", "ACTIVE_TENANT_DISPLAY_NAME"])?;
                }

                wtr.serialize((
                    self.active_tenant_uuid.map(|uuid| uuid.to_string()).unwrap_or_default(),
                    self.active_tenant_short_name.as_deref().unwrap_or(""),
                    self.active_tenant_display_name.as_deref().unwrap_or(""),
                ))?;

                let csv_string = String::from_utf8(wtr.into_inner()?)?;
                Ok(csv_string)
            }
            OutputFormat::Tree(_) => {
                // For tree format, just return the same as default text format
                let tenant_info = if let Some(uuid) = self.active_tenant_uuid {
                    if let (Some(short_name), Some(display_name)) = (&self.active_tenant_short_name, &self.active_tenant_display_name) {
                        format!("Active Tenant: {} ({})", short_name, display_name)
                    } else {
                        format!("Active Tenant: <unknown tenant UUID: {}>", uuid)
                    }
                } else {
                    "Active Tenant: <not set>".to_string()
                };
                Ok(tenant_info)
            }
        }
    }
}


pub async fn list_all_tenants(sub_matches: &ArgMatches) -> Result<(), CliActionError> {

    // Get format parameters directly from sub_matches
    let format_str = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .cloned()
        .unwrap_or_else(|| "json".to_string());

    let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);

    // Create format options with metadata set to false since tenants don't have metadata
    let format_options = crate::format::OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    };

    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(|e| CliActionError::FormattingError(e))?;

    let mut api = PhysnaApiClient::try_default()?;

    match api.list_tenants().await {
        Ok(tenant_settings) => {
            // Convert to a format that can be handled by the Formattable trait
            let tenant_list = crate::model::TenantList::from(tenant_settings);

            println!("{}", tenant_list.format(format)?);
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

pub async fn get_tenant_details(sub_matches: &ArgMatches) -> Result<(), CliActionError> {
    let tenant_uuid_param = sub_matches.get_one::<uuid::Uuid>(crate::commands::params::PARAMETER_TENANT_UUID);
    let tenant_name_param = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_TENANT_NAME);

    let mut api = PhysnaApiClient::try_default()?;

    // Get all tenants to search through
    let all_tenants = api.list_tenants().await?;

    // Find the specific tenant based on either UUID or name
    let tenant_setting = if let Some(uuid) = tenant_uuid_param {
        all_tenants.iter().find(|t| &t.tenant_uuid == uuid)
    } else if let Some(name) = tenant_name_param {
        all_tenants.iter().find(|t| &t.tenant_short_name == name)
    } else {
        return Err(CliActionError::MissingRequiredArgument("Either tenant UUID (--id) or tenant name (--tenant-name) must be provided".to_string()));
    };

    match tenant_setting {
        Some(tenant_setting) => {
            // Convert to Tenant for formatting
            let tenant: Tenant = tenant_setting.try_into()?;

            // Get format parameters directly from sub_matches
            let format_str = sub_matches.get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
                .cloned()
                .unwrap_or_else(|| "json".to_string());

            let with_headers = sub_matches.get_flag(crate::commands::params::PARAMETER_HEADERS);
            let pretty = sub_matches.get_flag(crate::commands::params::PARAMETER_PRETTY);
            // Note: We don't use metadata for tenants, so we set it to false

            // Create format options
            let format_options = crate::format::OutputFormatOptions {
                with_metadata: false,  // No metadata for tenants
                with_headers,
                pretty,
            };

            let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
                .map_err(|e| CliActionError::FormattingError(e))?;

            println!("{}", tenant.format(&format)?);
        },
        None => {
            if let Some(uuid) = tenant_uuid_param {
                return Err(CliActionError::TenantNotFound { identifier: uuid.to_string() });
            } else if let Some(name) = tenant_name_param {
                return Err(CliActionError::TenantNotFound { identifier: name.clone() });
            }
            // This shouldn't happen due to the argument group validation, but just in case
            return Err(CliActionError::MissingRequiredArgument("Either tenant UUID (--id) or tenant name (--tenant-name) must be provided".to_string()));
        }
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

pub async fn print_current_context(sub_matches: &ArgMatches) -> Result<(), CliActionError> {
    trace!("Executing 'context get' command");

    let format = crate::param_utils::get_format_parameter_value(sub_matches).await;
    let configuration = Configuration::load_default()?;
    let context_info = ContextInfo::from_configuration(&configuration).await?;

    println!("{}", context_info.format(&format)?);

    Ok(())
}

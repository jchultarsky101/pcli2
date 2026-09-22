use crate::{
    actions::CliActionError,
    commands::params::{PARAMETER_NAME, PARAMETER_REFRESH},
    configuration::Configuration,
    error_utils,
    format::{Formattable, FormattingError, OutputFormat, OutputFormatter},
    model::Tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextInfo {
    pub active_tenant_uuid: Option<uuid::Uuid>,
    pub active_tenant_short_name: Option<String>,
    pub active_tenant_display_name: Option<String>,
}

impl ContextInfo {
    pub async fn from_configuration(
        configuration: &Configuration,
    ) -> Result<ContextInfo, CliActionError> {
        if let Some(active_tenant_uuid) = configuration.active_tenant_uuid() {
            let mut api = PhysnaApiClient::try_default()?;
            let tenants =
                crate::tenant_cache::TenantCache::get_all_tenants(&mut api, false).await?;
            let active_tenant = tenants
                .into_iter()
                .find(|t| t.tenant_uuid.eq(active_tenant_uuid));

            match active_tenant {
                Some(active_tenant) => Ok(ContextInfo {
                    active_tenant_uuid: Some(active_tenant.tenant_uuid),
                    active_tenant_short_name: Some(active_tenant.tenant_short_name),
                    active_tenant_display_name: Some(active_tenant.tenant_display_name),
                }),
                None => Ok(ContextInfo {
                    active_tenant_uuid: Some(*active_tenant_uuid),
                    active_tenant_short_name: None,
                    active_tenant_display_name: None,
                }),
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
                    wtr.write_record([
                        "ACTIVE_TENANT_UUID",
                        "ACTIVE_TENANT_SHORT_NAME",
                        "ACTIVE_TENANT_DISPLAY_NAME",
                    ])?;
                }

                wtr.serialize((
                    self.active_tenant_uuid
                        .map(|uuid| uuid.to_string())
                        .unwrap_or_default(),
                    self.active_tenant_short_name.as_deref().unwrap_or(""),
                    self.active_tenant_display_name.as_deref().unwrap_or(""),
                ))?;

                let csv_string = crate::format::csv_text(wtr.into_inner()?)?;
                Ok(csv_string)
            }
            OutputFormat::Tree(_) => {
                // For tree format, just return the same as default text format
                let tenant_info = if let Some(uuid) = self.active_tenant_uuid {
                    if let (Some(short_name), Some(display_name)) = (
                        &self.active_tenant_short_name,
                        &self.active_tenant_display_name,
                    ) {
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
    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let mut api = PhysnaApiClient::try_default()?;

    let progress = crate::terminal::spinner("Fetching tenants...");
    let tenants_result = crate::tenant_cache::TenantCache::get_all_tenants(&mut api, false).await;
    progress.finish_and_clear();

    // A failure is returned, not just printed: it used to be reported and then
    // exit 0, so a script saw an empty tenant list as success.
    let tenant_settings = tenants_result?;
    let tenant_list = crate::model::TenantList::from(tenant_settings);
    crate::format::print_output(&tenant_list.format(format)?);
    Ok(())
}

pub async fn set_active_tenant(sub_matches: &ArgMatches) -> Result<(), CliActionError> {
    // This command prints nothing structured, so the format flags it accepts
    // for uniformity change nothing.
    for (flag, why) in [
        (
            crate::commands::params::PARAMETER_FORMAT,
            "tenant use prints no data",
        ),
        (
            crate::commands::params::PARAMETER_PRETTY,
            "tenant use prints no data",
        ),
        (
            crate::commands::params::PARAMETER_HEADERS,
            "tenant use prints no data",
        ),
    ] {
        crate::format_utils::warn_if_given(sub_matches, flag, why);
    }
    let name = sub_matches.get_one::<String>(PARAMETER_NAME);
    let refresh = sub_matches.get_flag(PARAMETER_REFRESH);
    let mut api = PhysnaApiClient::try_default()?;

    // Get tenants from cache or API depending on refresh flag
    let tenants = crate::tenant_cache::TenantCache::get_all_tenants(&mut api, refresh)
        .await
        .map_err(CliActionError::ApiError)?;

    // If no name was provided, show interactive selection
    let selected_tenant = if let Some(name) = name {
        // By short name or UUID, as `--tenant` accepts. On a miss the list is
        // fetched once more: a tenant granted since the cache was written is the
        // usual reason, and it used to take `--refresh` to find it.
        match tenants
            .iter()
            .find(|t| crate::param_utils::tenant_matches(t, name))
        {
            Some(tenant) => Some(tenant.clone()),
            None if !refresh => crate::tenant_cache::TenantCache::get_all_tenants(&mut api, true)
                .await
                .map_err(CliActionError::ApiError)?
                .into_iter()
                .find(|t| crate::param_utils::tenant_matches(t, name)),
            None => None,
        }
    } else {
        // Interactive selection using TUI
        if tenants.is_empty() {
            error_utils::report_error_with_remediation(
                &"No tenants available",
                &[
                    "Verify your authentication credentials are valid",
                    "Check that you have access to at least one tenant",
                    "Log in again with 'pcli2 auth login'",
                ],
            );
            return Err(CliActionError::AlreadyReported(
                crate::exit_codes::PcliExitCode::NotFound,
            ));
        }

        // Create options for the select menu
        let options: Vec<String> = tenants
            .iter()
            .map(|tenant| {
                format!(
                    "{}: {} ({})",
                    tenant.tenant_short_name, tenant.tenant_display_name, tenant.tenant_uuid
                )
            })
            .collect();

        // Use inquire to create an interactive selection
        crate::terminal::require_prompt("a tenant name (--name)")?;
        let ans = inquire::Select::new("Select a tenant:", options)
            .with_help_message("Choose the tenant you want to set as active")
            .prompt();

        match ans {
            Ok(choice) => {
                let tenant_name = choice
                    .split_once(':')
                    .map(|(before, _)| before.trim())
                    .unwrap_or(choice.trim());
                trace!("User selected tenant: {}", tenant_name);
                // Find the tenant that matches the selection
                tenants
                    .iter()
                    .find(|t| t.tenant_short_name == tenant_name)
                    .cloned()
            }
            Err(_) => {
                error_utils::report_error_with_remediation(
                    &"No tenant selected",
                    &[
                        "Run 'pcli2 tenant use' again to select a tenant",
                        "Verify you have access to at least one tenant",
                        "Check your authentication credentials",
                    ],
                );
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
        error_utils::report_error_with_remediation(
            &format!(
                "Tenant '{}' not found",
                name.map(|n| n.to_string())
                    .unwrap_or_else(|| "(selected)".to_string())
            ),
            &[
                "Check the tenant name spelling",
                "List available tenants with 'pcli2 tenant list'",
                "Verify you have access to this tenant",
            ],
        ); // Safe to unwrap since we checked above
        return Err(CliActionError::AlreadyReported(
            crate::exit_codes::PcliExitCode::NotFound,
        ));
    }

    Ok(())
}

pub async fn print_active_tenant_name_with_format(
    sub_matches: &ArgMatches,
) -> Result<(), CliActionError> {
    trace!("Executing 'context get tenant' with format options");

    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let configuration = Configuration::load_default()?;

    if let Some(active_tenant_uuid) = configuration.get_active_tenant_uuid() {
        let mut api = PhysnaApiClient::try_default()?;
        let tenants = crate::tenant_cache::TenantCache::get_all_tenants(&mut api, false).await?;
        let active_tenant = tenants
            .into_iter()
            .find(|t| t.tenant_uuid.eq(&active_tenant_uuid));

        match active_tenant {
            Some(tenant_setting) => {
                // Convert to Tenant for formatting
                let tenant = Tenant {
                    uuid: tenant_setting.tenant_uuid,
                    name: tenant_setting.tenant_short_name.clone(),
                    description: tenant_setting.tenant_display_name.clone(),
                };
                crate::format::print_output(&tenant.format(&format)?);
            }
            None => {
                // Create a minimal tenant for formatting when UUID exists but tenant not found
                let tenant = Tenant {
                    uuid: active_tenant_uuid,
                    name: "Unknown Tenant".to_string(),
                    description: "Tenant not found in current user's tenants".to_string(),
                };
                crate::format::print_output(&tenant.format(&format)?);
            }
        }
    } else {
        // Create a minimal tenant for formatting when no active tenant is set
        let tenant = Tenant {
            uuid: uuid::Uuid::nil(), // Use nil UUID for no tenant
            name: "No active tenant".to_string(),
            description: "No tenant selected".to_string(),
        };
        crate::format::print_output(&tenant.format(&format)?);
    }

    Ok(())
}

pub async fn clear_active_tenant() -> Result<(), CliActionError> {
    let mut configuration = Configuration::load_default()?;
    configuration.clear_active_tenant();
    match configuration.save_to_default() {
        Ok(()) => Ok(()),
        Err(e) => {
            error_utils::report_error_with_remediation(
                &format!("Error saving configuration: {}", e),
                &[
                    "Check that you have write permissions to the configuration directory",
                    "Verify the configuration file is not locked by another process",
                    "Ensure you have sufficient disk space",
                ],
            );
            Err(CliActionError::ConfigurationError(e))
        }
    }
}

pub async fn get_tenant_state_counts(sub_matches: &ArgMatches) -> Result<(), CliActionError> {
    trace!("Executing tenant state command...");

    // Check if the --type parameter was provided
    let state_type = sub_matches.get_one::<String>("type");

    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let configuration =
        Configuration::load_default().map_err(CliActionError::ConfigurationError)?;
    let mut api = PhysnaApiClient::try_default().map_err(CliActionError::ApiError)?;
    let tenant = crate::param_utils::get_tenant(&mut api, sub_matches, &configuration)
        .await
        .map_err(|e| match e {
            crate::error::CliError::ConfigurationError(config_error) => {
                CliActionError::ConfigurationError(config_error)
            }
            crate::error::CliError::MissingRequiredArgument(msg) => {
                CliActionError::MissingRequiredArgument(msg)
            }
            crate::error::CliError::TenantNotFound { identifier } => {
                CliActionError::TenantNotFound { identifier }
            }
            crate::error::CliError::FormattingError(fmt_error) => {
                CliActionError::FormattingError(fmt_error)
            }
            crate::error::CliError::ActionError(action_error) => action_error, // Already a CliActionError
            crate::error::CliError::AlreadyReported(code) => CliActionError::AlreadyReported(code),
            crate::error::CliError::PhysnaExtendedApiError(api_error) => {
                CliActionError::ApiError(api_error)
            }
            crate::error::CliError::SecurityError(msg) => {
                CliActionError::MissingRequiredArgument(msg)
            }
            crate::error::CliError::UnsupportedSubcommand(msg) => {
                CliActionError::MissingRequiredArgument(msg)
            }
            crate::error::CliError::JsonError(json_error) => CliActionError::JsonError(json_error),
            crate::error::CliError::FolderNotFound(path, _) => {
                CliActionError::MissingRequiredArgument(format!("Folder not found: {}", path))
            }
            crate::error::CliError::FolderListError(_) => {
                CliActionError::MissingRequiredArgument("Folder list error".to_string())
            }
            crate::error::CliError::XlsxReportError(xlsx_error) => {
                CliActionError::MissingRequiredArgument(format!(
                    "Excel report error: {}",
                    xlsx_error
                ))
            }
            crate::error::CliError::UuidParsingError(uuid_error) => {
                CliActionError::UuidPartsinError(uuid_error)
            }
            crate::error::CliError::FolderRenameFailed(_, _) => {
                CliActionError::MissingRequiredArgument("Folder rename failed".to_string())
            }
            crate::error::CliError::AssetResolutionError(which, msg) => {
                CliActionError::MissingRequiredArgument(format!(
                    "Could not resolve {} asset: {}",
                    which, msg
                ))
            }
            crate::error::CliError::CheckpointError(e) => {
                CliActionError::BusinessLogicError(e.to_string())
            }
            // Tenant resolution never raises this; mapped for exhaustiveness.
            crate::error::CliError::FeatureUnavailable(msg) => {
                CliActionError::BusinessLogicError(msg)
            }
            crate::error::CliError::InputRequired(msg) => CliActionError::InputRequired(msg),
            crate::error::CliError::RemovedArgument(msg) => {
                CliActionError::MissingRequiredArgument(msg)
            }
        })?;

    if let Some(state) = state_type {
        // Call the new function to list assets by state
        let assets = api.list_assets_by_state(&tenant.uuid, state).await?;
        crate::format::print_output(&assets.format(format)?);
    } else {
        // Get the asset state counts from the API (original behavior)
        let state_counts = api.get_asset_state_counts(&tenant.uuid).await?;
        crate::format::print_output(&state_counts.format(&format)?);
    }

    Ok(())
}

/// List the tenant's recent failures (assets, reports, part-finder reports),
/// newest first, for the `tenant failures` command.
///
/// `--kind` narrows the listing and `--limit` stops it early; the JSON output
/// keeps the tenant-wide totals per kind either way.
pub async fn list_recent_failures(sub_matches: &ArgMatches) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant failures command...");

    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let kinds: Vec<crate::model::FailureSource> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_KIND)
        .map(|values| {
            values
                .map(|v| match v.as_str() {
                    "asset" => crate::model::FailureSource::Asset,
                    "report" => crate::model::FailureSource::Report,
                    // clap's value_parser only lets FailureSource::ALL through.
                    _ => crate::model::FailureSource::PartFinderReport,
                })
                .collect()
        })
        .unwrap_or_default();
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied();

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();

    let progress = crate::terminal::spinner("Fetching recent failures...");
    let failures = ctx
        .api()
        .list_recent_failures(&tenant_uuid, &kinds, limit)
        .await;
    progress.finish_and_clear();
    let failures = failures.map_err(crate::error::CliError::PhysnaExtendedApiError)?;

    crate::format::print_output(&failures.format(format)?);

    Ok(())
}

/// The registered metadata field called `name`, with its id.
///
/// Names are matched exactly (the registry is case-sensitive). A miss lists
/// the registered names so the user can copy one; a field the server sent
/// without an id cannot be addressed and is reported as such.
async fn resolve_metadata_field_by_name(
    api: &mut PhysnaApiClient,
    tenant_uuid: &uuid::Uuid,
    name: &str,
) -> Result<(crate::model::MetadataField, uuid::Uuid), crate::error::CliError> {
    let fields = api
        .get_metadata_fields(&tenant_uuid.to_string())
        .await
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    match fields.metadata_fields.iter().find(|f| f.name == name) {
        Some(field) => match field.id {
            Some(id) => Ok((field.clone(), id)),
            None => Err(crate::error::CliError::PhysnaExtendedApiError(
                crate::physna_v3::ApiError::InvalidParameterError(format!(
                    "the server sent no id for metadata field '{}', so it cannot be addressed",
                    name
                )),
            )),
        },
        None => {
            let mut known: Vec<&str> = fields
                .metadata_fields
                .iter()
                .map(|f| f.name.as_str())
                .collect();
            known.sort_unstable();
            Err(crate::error::CliError::PhysnaExtendedApiError(
                crate::physna_v3::ApiError::NotFoundError(format!(
                    "metadata field '{}' not found. Registered fields: {}",
                    name,
                    if known.is_empty() {
                        "(none)".to_string()
                    } else {
                        known.join(", ")
                    }
                )),
            ))
        }
    }
}

/// Format options for a command with no metadata columns of its own.
pub(crate) fn plain_format(
    sub_matches: &ArgMatches,
) -> Result<crate::format::OutputFormat, crate::error::CliError> {
    Ok(crate::format_utils::FormatParams::from_args(sub_matches).format)
}

/// `tenant metadata rename --name OLD --new-name NEW`. Silent on success.
pub async fn rename_tenant_metadata_field(
    sub_matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant metadata rename command...");
    let name = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_NAME)
        .cloned()
        .unwrap_or_default();
    let new_name = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_NEW_NAME)
        .cloned()
        .unwrap_or_default();

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let (field, id) = resolve_metadata_field_by_name(ctx.api(), &tenant_uuid, &name).await?;

    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would rename metadata field '{}' ({}) to '{}'",
            field.name, id, new_name
        );
        return Ok(());
    }

    ctx.api()
        .rename_metadata_field(&tenant_uuid, &id, &new_name)
        .await
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    Ok(())
}

/// `tenant metadata delete --name NAME [--force]`. Confirms unless `--yes`;
/// silent on success.
pub async fn delete_tenant_metadata_field(
    sub_matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant metadata delete command...");
    let name = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_NAME)
        .cloned()
        .unwrap_or_default();
    let force = sub_matches.get_flag("force");
    let yes_flag = sub_matches.get_flag("yes");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let (field, id) = resolve_metadata_field_by_name(ctx.api(), &tenant_uuid, &name).await?;

    let consequence = if force {
        " and remove its values from every asset"
    } else {
        ""
    };
    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would delete metadata field '{}' ({}){}",
            field.name, id, consequence
        );
        return Ok(());
    }
    if !yes_flag
        && !crate::terminal::confirm(
            &format!("Delete metadata field '{}'{}?", field.name, consequence),
            Some("This action cannot be undone"),
        )?
    {
        eprintln!("Deletion cancelled.");
        return Ok(());
    }

    ctx.api()
        .delete_metadata_field(&tenant_uuid, &id, force)
        .await
        .map_err(|e| {
            // Without --force the server refuses a field that is still in use;
            // say what to do about it, keeping the error's class and exit code.
            let hint = " The field is in use by assets; pass --force to delete it and remove its values from every asset.";
            match e {
                crate::physna_v3::ApiError::ConflictError(message) if !force => {
                    crate::physna_v3::ApiError::ConflictError(format!("{}{}", message, hint))
                }
                crate::physna_v3::ApiError::HttpStatus { status, message }
                    if !force && (400..500).contains(&status) =>
                {
                    crate::physna_v3::ApiError::HttpStatus {
                        status,
                        message: format!("{}{}", message, hint),
                    }
                }
                other => other,
            }
        })
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    Ok(())
}

/// `tenant metadata assets --name NAME [--limit N]`: the assets with a value
/// for the field, printed like `asset list`.
pub async fn list_assets_using_tenant_metadata_field(
    sub_matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant metadata assets command...");
    let name = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_NAME)
        .cloned()
        .unwrap_or_default();
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied();
    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let (_, id) = resolve_metadata_field_by_name(ctx.api(), &tenant_uuid, &name).await?;

    let progress = crate::terminal::spinner("Fetching assets...");
    let assets = ctx
        .api()
        .list_assets_using_metadata_field(&tenant_uuid, &id, limit)
        .await;
    progress.finish_and_clear();
    let assets = assets.map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    crate::format::print_output(&assets.format(format)?);
    Ok(())
}

/// `tenant metadata coverage`: how many assets carry any metadata at all.
pub async fn tenant_metadata_coverage(
    sub_matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant metadata coverage command...");
    let format = plain_format(sub_matches)?;
    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let coverage: crate::model::MetadataCoverage = ctx
        .api()
        .get_metadata_coverage(&tenant_uuid)
        .await
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?
        .into();
    crate::format::print_output(&coverage.format(format)?);
    Ok(())
}

/// `tenant metadata missing [--folder-path P]* [--extension E]* [--limit N]`:
/// the assets with no metadata at all, oldest first.
pub async fn list_assets_without_metadata(
    sub_matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant metadata missing command...");
    // The server keys folders by their stored path, without a leading slash
    // and without the `/Home` alias.
    let folders: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
        .map(|values| {
            values
                .map(|p| {
                    crate::model::normalize_path(p)
                        .trim_start_matches('/')
                        .to_string()
                })
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let extensions: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_EXTENSION)
        .map(|values| {
            values
                .map(|e| e.trim_start_matches('.').to_string())
                .collect()
        })
        .unwrap_or_default();
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied();
    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let progress = crate::terminal::spinner("Fetching assets...");
    let assets = ctx
        .api()
        .list_assets_without_metadata(&tenant_uuid, &folders, &extensions, limit)
        .await;
    progress.finish_and_clear();
    let assets = assets.map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    crate::format::print_output(&assets.format(format)?);
    Ok(())
}

/// List all metadata fields registered in the tenant, with their data types.
///
/// This backs the `tenant metadata list` command. The CSV output deliberately
/// mirrors the classic `asset metadata create-batch` input header
/// (`ASSET_PATH,NAME,VALUE,TYPE`) so the listing can be turned into a
/// batch-upload template: NAME and TYPE come from the registry, ASSET_PATH and
/// VALUE are left empty for the user to fill.
pub async fn list_tenant_metadata_fields(
    sub_matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    trace!("Executing tenant metadata list command...");

    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = crate::param_utils::get_tenant(&mut api, sub_matches, &configuration).await?;

    let fields = api
        .get_metadata_fields(&tenant.uuid.to_string())
        .await
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?;

    crate::format::print_output(&fields.format(format)?);

    Ok(())
}

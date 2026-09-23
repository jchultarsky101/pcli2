//! User action implementations.
//!
//! This module contains the core logic for user-related operations in the Physna CLI.
//! It handles listing users, managing user permissions, and other user management tasks.

use crate::format::{Formattable, OutputFormat};
use crate::physna_v3::{PhysnaApiClient, TryDefault};
use clap::ArgMatches;
use serde::{Deserialize, Serialize};

/// Represents a user in the Physna system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// The unique identifier for the user
    #[serde(rename = "id")]
    pub id: String,
    /// The email address of the user
    pub email: String,
    /// The tenant settings for the user
    pub settings: UserSettings,
    /// The creation timestamp of the user account
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// The last update timestamp of the user account
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// Represents the settings for a user in a specific tenant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    /// The ID of the tenant
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    /// The role of the user in the tenant
    #[serde(rename = "tenantRole")]
    pub tenant_role: String,
    /// Whether the user is enabled in this tenant
    #[serde(rename = "userEnabled")]
    pub user_enabled: bool,
}

/// Represents a response containing a list of users
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserListResponse {
    /// List of users
    pub users: Vec<User>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<crate::model::PageData>,
}

/// Represents a response containing a single user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleUserResponse {
    /// The user details
    pub user: User,
}

impl Formattable for User {
    fn format(&self, format: &OutputFormat) -> Result<String, crate::format::FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                json.map_err(crate::format::FormattingError::JsonSerializationError)
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "USER_ID",
                        "EMAIL",
                        "TENANT_ROLE",
                        "USER_ENABLED",
                        "CREATED_AT",
                        "UPDATED_AT",
                    ))?;
                }

                wtr.serialize((
                    &self.id,
                    &self.email,
                    &self.settings.tenant_role,
                    &self.settings.user_enabled,
                    &self.created_at,
                    &self.updated_at,
                ))?;

                let data = wtr.into_inner()?;
                crate::format::csv_text(data).map_err(crate::format::FormattingError::Utf8Error)
            }
            _ => Err(crate::format::FormattingError::UnsupportedOutputFormat(
                format.to_string(),
            )),
        }
    }
}

impl Formattable for UserListResponse {
    fn format(&self, format: &OutputFormat) -> Result<String, crate::format::FormattingError> {
        match format {
            OutputFormat::Json(options) => {
                let json = if options.pretty {
                    serde_json::to_string_pretty(self)
                } else {
                    serde_json::to_string(self)
                };
                json.map_err(crate::format::FormattingError::JsonSerializationError)
            }
            OutputFormat::Csv(options) => {
                let mut wtr = csv::Writer::from_writer(vec![]);

                if options.with_headers {
                    wtr.serialize((
                        "USER_ID",
                        "EMAIL",
                        "TENANT_ROLE",
                        "USER_ENABLED",
                        "CREATED_AT",
                        "UPDATED_AT",
                    ))?;
                }

                for user in &self.users {
                    wtr.serialize((
                        &user.id,
                        &user.email,
                        &user.settings.tenant_role,
                        &user.settings.user_enabled,
                        &user.created_at,
                        &user.updated_at,
                    ))?;
                }

                let data = wtr.into_inner()?;
                crate::format::csv_text(data).map_err(crate::format::FormattingError::Utf8Error)
            }
            _ => Err(crate::format::FormattingError::UnsupportedOutputFormat(
                format.to_string(),
            )),
        }
    }
}

/// List users in the current tenant (or the one named with `--tenant`).
pub async fn list_users(matches: &ArgMatches) -> Result<(), crate::error::CliError> {
    let format = crate::format_utils::FormatParams::from_args(matches).format;

    let mut client = PhysnaApiClient::try_default()?;
    let configuration = crate::configuration::Configuration::load_or_create_default()?;
    // Honours --tenant / PCLI2_TENANT like every other tenant-scoped command; it
    // used to read only the active tenant, and the flag was not accepted.
    let tenant = crate::param_utils::get_tenant(&mut client, matches, &configuration).await?;

    let users_response = client
        .list_tenant_users(&tenant.uuid)
        .await
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    crate::format::print_output(&users_response.format(&format)?);
    Ok(())
}

/// Get details for a specific user.
pub async fn get_user(matches: &ArgMatches) -> Result<(), crate::error::CliError> {
    let user_id = matches
        .get_one::<String>("user_id")
        .ok_or_else(|| crate::error::CliError::MissingRequiredArgument("user_id".to_string()))?;
    let format = crate::format_utils::FormatParams::from_args(matches).format;

    let mut client = PhysnaApiClient::try_default()?;
    let configuration = crate::configuration::Configuration::load_or_create_default()?;
    let tenant = crate::param_utils::get_tenant(&mut client, matches, &configuration).await?;

    let user = client
        .get_user(&tenant.uuid, user_id)
        .await
        .map_err(crate::error::CliError::PhysnaExtendedApiError)?;
    crate::format::print_output(&user.format(&format)?);
    Ok(())
}

//! User action implementations.
//!
//! This module contains the core logic for user-related operations in the Physna CLI.
//! It handles listing users, managing user permissions, and other user management tasks.

use crate::actions::CliActionError;
use crate::format::{Formattable, OutputFormat, OutputFormatOptions};
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
                String::from_utf8(data).map_err(crate::format::FormattingError::Utf8Error)
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
                String::from_utf8(data).map_err(crate::format::FormattingError::Utf8Error)
            }
            _ => Err(crate::format::FormattingError::UnsupportedOutputFormat(
                format.to_string(),
            )),
        }
    }
}

/// List users in the current tenant
pub async fn list_users(matches: &ArgMatches) -> Result<(), CliActionError> {
    // Get format parameters
    let format_str = matches
        .get_one::<String>("format")
        .map(|s| s.as_str())
        .unwrap_or("json");

    let with_headers = matches.get_flag("headers");
    let pretty = matches.get_flag("pretty");

    let format_options = OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    };

    let format = OutputFormat::from_string_with_options(format_str, format_options).unwrap();

    // Create API client
    let mut client = PhysnaApiClient::try_default()?;

    // Get the active tenant from configuration
    let configuration = crate::configuration::Configuration::load_or_create_default()?;
    let active_tenant_uuid = configuration.get_active_tenant_uuid().ok_or_else(|| {
        CliActionError::BusinessLogicError(
            "No active tenant found. Please set an active tenant with 'pcli2 tenant use' first."
                .to_string(),
        )
    })?;

    // Get the current user's tenant settings to find the active one
    let current_user = client.get_current_user().await?;

    let active_tenant = current_user
        .user
        .settings
        .iter()
        .find(|setting| setting.tenant_uuid == active_tenant_uuid)
        .ok_or_else(|| {
            CliActionError::BusinessLogicError(
                "Active tenant UUID not found in user's tenant settings".to_string(),
            )
        })?
        .clone();

    // List users for the tenant
    let users_response = client.list_tenant_users(&active_tenant.tenant_uuid).await?;

    // Format and print the response
    let output = users_response.format(&format)?;
    println!("{}", output);

    Ok(())
}

/// Get details for a specific user
pub async fn get_user(matches: &ArgMatches) -> Result<(), CliActionError> {
    // Get the user ID from the command line
    let user_id = matches.get_one::<String>("user_id").ok_or_else(|| {
        CliActionError::MissingRequiredArgument("user_id is required".to_string())
    })?;

    // Get format parameters
    let format_str = matches
        .get_one::<String>("format")
        .map(|s| s.as_str())
        .unwrap_or("json");

    let with_headers = matches.get_flag("headers");
    let pretty = matches.get_flag("pretty");

    let format_options = OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    };

    let format = OutputFormat::from_string_with_options(format_str, format_options).unwrap();

    // Create API client
    let mut client = PhysnaApiClient::try_default()?;

    // Get the active tenant from configuration
    let configuration = crate::configuration::Configuration::load_or_create_default()?;
    let active_tenant_uuid = configuration.get_active_tenant_uuid().ok_or_else(|| {
        CliActionError::BusinessLogicError(
            "No active tenant found. Please set an active tenant with 'pcli2 tenant use' first."
                .to_string(),
        )
    })?;

    // Get the user details
    let user = client.get_user(&active_tenant_uuid, user_id).await?;

    // Format and print the response
    let output = user.format(&format)?;
    println!("{}", output);

    Ok(())
}

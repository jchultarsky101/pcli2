//! Environment command definitions.
//!
//! This module defines CLI commands related to environment configuration management.
//! These commands were previously under `config environment` but are now top-level
//! for better ergonomics.

use clap::{ArgMatches, Command};

use crate::commands::params::{
    api_url_parameter, auth_url_parameter, format_parameter, format_pretty_parameter,
    format_with_headers_parameter, name_parameter, ui_url_parameter, COMMAND_ADD,
    COMMAND_ENVIRONMENT_GET, COMMAND_ENVIRONMENT_LIST, COMMAND_REMOVE, COMMAND_RESET, COMMAND_USE,
};

/// Create the environment command with all its subcommands.
pub fn environment_command() -> Command {
    Command::new("environment")
        .about("Manage environment configurations")
        .visible_alias("env")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(COMMAND_ADD)
                .about("Add a new environment configuration")
                .arg(
                    name_parameter()
                        .required(true)
                        .help("Name of the environment"),
                )
                .arg(api_url_parameter())
                .arg(ui_url_parameter())
                .arg(auth_url_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_USE)
                .about("Switch to an environment")
                .arg(
                    name_parameter()
                        .required(false)
                        .help("Name of the environment to switch to"),
                ),
        )
        .subcommand(
            Command::new(COMMAND_REMOVE)
                .about("Remove an environment")
                .arg(
                    name_parameter()
                        .required(true)
                        .help("Name of the environment to remove"),
                ),
        )
        .subcommand(
            Command::new(COMMAND_ENVIRONMENT_LIST)
                .about("List all environments")
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
        .subcommand(
            Command::new(COMMAND_RESET)
                .about("Reset all environment configurations to blank state"),
        )
        .subcommand(
            Command::new(COMMAND_ENVIRONMENT_GET)
                .about("Get environment details")
                .arg(name_parameter().required(false).help(
                    "Name of the environment to get details for (defaults to active environment)",
                ))
                .arg(format_parameter().value_parser(["json", "csv"]))
                .arg(format_pretty_parameter())
                .arg(format_with_headers_parameter()),
        )
}

/// Execute environment-related subcommands based on the provided arguments
pub async fn execute_environment_command(
    matches: &ArgMatches,
) -> Result<(), crate::error::CliError> {
    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            use crate::commands::params::{
                PARAMETER_API_URL, PARAMETER_AUTH_URL, PARAMETER_UI_URL,
            };
            use tracing::trace;

            trace!("Executing environment add command");

            let env_name = sub_matches.get_one::<String>("name").ok_or(
                crate::error::CliError::MissingRequiredArgument("name".to_string()),
            )?;

            let api_url = sub_matches.get_one::<String>(PARAMETER_API_URL).cloned();
            let ui_url = sub_matches.get_one::<String>(PARAMETER_UI_URL).cloned();
            let auth_url = sub_matches.get_one::<String>(PARAMETER_AUTH_URL).cloned();

            let mut configuration = crate::configuration::Configuration::load_or_create_default()?;

            let env_config = crate::configuration::EnvironmentConfig {
                api_base_url: api_url.unwrap_or_else(crate::configuration::default_api_base_url),
                ui_base_url: ui_url.unwrap_or_else(crate::configuration::default_ui_base_url),
                auth_base_url: auth_url.unwrap_or_else(crate::configuration::default_auth_base_url),
            };

            configuration.add_environment(env_name.clone(), env_config);
            configuration.save_to_default()?;

            println!("Environment '{}' added successfully", env_name);
            Ok(())
        }
        Some(("use", sub_matches)) => {
            use crate::error_utils;
            use tracing::trace;

            trace!("Executing environment use command");

            let env_name = sub_matches.get_one::<String>("name");

            let mut configuration = crate::configuration::Configuration::load_or_create_default()?;

            let selected_env_name = if let Some(name) = env_name {
                // Use the provided name
                name.clone()
            } else {
                // Interactive selection using TUI
                let available_envs = configuration.list_environments();

                if available_envs.is_empty() {
                    error_utils::report_error_with_remediation(
                        &"No environments available",
                        &[
                            "Add an environment with 'pcli2 env add'",
                            "Check that your configuration file is properly set up",
                        ],
                    );
                    return Ok(());
                }

                // Create options for the select menu
                let options: Vec<String> = available_envs
                    .iter()
                    .map(|env_name| {
                        let is_active =
                            if let Some(ref active) = configuration.get_active_environment() {
                                env_name == active
                            } else {
                                false
                            };
                        let active_status = if is_active { " (active)" } else { "" };
                        format!("{}{}", env_name, active_status)
                    })
                    .collect();

                // Use inquire to create an interactive selection
                let ans = inquire::Select::new("Select an environment:", options)
                    .with_help_message("Choose the environment you want to set as active")
                    .prompt();

                match ans {
                    Ok(choice) => {
                        // Extract the environment name from the choice (removing " (active)" if present)
                        let env_name = choice
                            .split_once(" (active)")
                            .map(|(before, _)| before.trim())
                            .unwrap_or(&choice);
                        env_name.to_string()
                    }
                    Err(_) => {
                        error_utils::report_error_with_remediation(
                            &"No environment selected",
                            &[
                                "Run 'pcli2 env use' again to select an environment",
                                "Add an environment with 'pcli2 env add' if none exist",
                            ],
                        );
                        return Ok(());
                    }
                }
            };

            configuration.set_active_environment(&selected_env_name)?;
            // Clear the active tenant when switching environments to avoid confusion
            configuration.clear_active_tenant();
            configuration.save_to_default()?;

            println!("Switched to environment '{}'. Select a tenant with 'pcli2 tenant use' before running commands.", selected_env_name);
            Ok(())
        }
        Some(("remove", sub_matches)) => {
            use tracing::trace;

            trace!("Executing environment remove command");

            let env_name = sub_matches.get_one::<String>("name").ok_or(
                crate::error::CliError::MissingRequiredArgument("name".to_string()),
            )?;

            let mut configuration = crate::configuration::Configuration::load_or_create_default()?;

            configuration.remove_environment(env_name)?;
            configuration.save_to_default()?;

            println!("Environment '{}' removed successfully", env_name);
            Ok(())
        }
        Some(("list", sub_matches)) => {
            use crate::commands::params::{PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_PRETTY};
            use crate::format::{OutputFormat, OutputFormatOptions};
            use tracing::trace;

            trace!("Executing environment list command");

            // Get format parameters with precedence: 1) explicit --format, 2) PCLI2_FORMAT env var, 3) default "json"
            let format_str =
                if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
                    // User explicitly provided --format argument
                    format_val.clone()
                } else {
                    // Format was not explicitly provided by user, check environment variable first
                    if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
                        env_format
                    } else {
                        // Use default value
                        "json".to_string()
                    }
                };

            let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
            let pretty = sub_matches.get_flag(PARAMETER_PRETTY);

            let format_options = OutputFormatOptions {
                with_metadata: false,
                with_headers,
                pretty,
            };

            let format = OutputFormat::from_string_with_options(&format_str, format_options)
                .map_err(crate::error::CliError::FormattingError)?;

            let configuration = crate::configuration::Configuration::load_or_create_default()?;
            let active_env = configuration.get_active_environment();

            // Create a detailed representation for display
            #[derive(serde::Serialize)]
            struct EnvironmentDetails {
                name: String,
                is_active: bool,
                api_base_url: String,
                ui_base_url: String,
                auth_base_url: String,
            }

            let env_details: Vec<EnvironmentDetails> = configuration
                .list_environments()
                .into_iter()
                .map(|name| {
                    let is_active = if let Some(ref active) = active_env {
                        name == *active
                    } else {
                        false
                    };

                    let env_config = configuration
                        .get_environment_config(&name)
                        .expect("Environment should exist since it came from list_environments");

                    EnvironmentDetails {
                        name: name.clone(),
                        is_active,
                        api_base_url: env_config.api_base_url.clone(),
                        ui_base_url: env_config.ui_base_url.clone(),
                        auth_base_url: env_config.auth_base_url.clone(),
                    }
                })
                .collect();

            // Format the response based on the requested format
            match format {
                OutputFormat::Json(options) => {
                    let json_output = if options.pretty {
                        serde_json::to_string_pretty(&env_details)
                    } else {
                        serde_json::to_string(&env_details)
                    };
                    match json_output {
                        Ok(json) => println!("{}", json),
                        Err(e) => {
                            return Err(crate::error::CliError::FormattingError(
                                crate::format::FormattingError::JsonSerializationError(e),
                            ))
                        }
                    }
                }
                OutputFormat::Csv(options) => {
                    use crate::format::FormattingError;
                    let mut wtr = csv::Writer::from_writer(vec![]);

                    if options.with_headers {
                        if let Err(e) = wtr.serialize((
                            "ENVIRONMENT_NAME",
                            "IS_ACTIVE",
                            "API_BASE_URL",
                            "UI_BASE_URL",
                            "AUTH_BASE_URL",
                        )) {
                            return Err(crate::error::CliError::FormattingError(
                                FormattingError::CsvError(e),
                            ));
                        }
                    }

                    for env_detail in &env_details {
                        if let Err(e) = wtr.serialize((
                            &env_detail.name,
                            &env_detail.is_active,
                            &env_detail.api_base_url,
                            &env_detail.ui_base_url,
                            &env_detail.auth_base_url,
                        )) {
                            return Err(crate::error::CliError::FormattingError(
                                FormattingError::CsvError(e),
                            ));
                        }
                    }

                    let data = match wtr.into_inner() {
                        Ok(data) => data,
                        Err(e) => {
                            return Err(crate::error::CliError::FormattingError(
                                FormattingError::CsvIntoInnerError(e),
                            ))
                        }
                    };
                    let csv_output = match String::from_utf8(data) {
                        Ok(csv_str) => csv_str,
                        Err(e) => {
                            return Err(crate::error::CliError::FormattingError(
                                FormattingError::Utf8Error(e),
                            ))
                        }
                    };
                    print!("{}", csv_output);
                }
                OutputFormat::Tree(_) => {
                    // For tree format, show detailed information
                    for env_detail in &env_details {
                        let active_status = if env_detail.is_active {
                            " (active)"
                        } else {
                            ""
                        };
                        println!("{}{}:", env_detail.name, active_status);
                        println!("  API Base URL: {}", env_detail.api_base_url);
                        println!("  UI Base URL: {}", env_detail.ui_base_url);
                        println!("  Auth Base URL: {}", env_detail.auth_base_url);
                        println!(); // Empty line between environments
                    }
                }
            }
            Ok(())
        }
        Some(("reset", _sub_matches)) => {
            use tracing::trace;

            trace!("Executing environment reset command");

            let mut configuration = crate::configuration::Configuration::load_or_create_default()?;

            configuration.reset_environments();
            configuration.save_to_default()?;

            println!("Environment configurations reset successfully");
            Ok(())
        }
        Some(("get", sub_matches)) => {
            use crate::commands::params::{PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_PRETTY};
            use crate::error_utils;
            use crate::format::{OutputFormat, OutputFormatOptions};
            use tracing::trace;

            trace!("Executing environment get command");

            // Get format parameters with precedence: 1) explicit --format, 2) PCLI2_FORMAT env var, 3) default "json"
            let format_str =
                if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
                    // User explicitly provided --format argument
                    format_val.clone()
                } else {
                    // Format was not explicitly provided by user, check environment variable first
                    if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
                        env_format
                    } else {
                        // Use default value
                        "json".to_string()
                    }
                };

            let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
            let pretty = sub_matches.get_flag(PARAMETER_PRETTY);

            let format_options = OutputFormatOptions {
                with_metadata: false,
                with_headers,
                pretty,
            };

            let format = OutputFormat::from_string_with_options(&format_str, format_options)
                .map_err(crate::error::CliError::FormattingError)?;

            let env_name = sub_matches.get_one::<String>("name");

            let configuration = crate::configuration::Configuration::load_or_create_default()?;

            let target_env_name = if let Some(name) = env_name {
                // Use the provided name
                name.clone()
            } else {
                // Use the active environment
                if let Some(active_env) = configuration.get_active_environment() {
                    active_env
                } else {
                    error_utils::report_error_with_remediation(
                        &"No active environment set",
                        &[
                            "Set an active environment with 'pcli2 env use'",
                            "Add an environment with 'pcli2 env add' if none exist",
                        ],
                    );
                    return Ok(());
                }
            };

            // Get the environment configuration
            if let Some(env_config) = configuration.get_environment_config(&target_env_name) {
                // Create a detailed representation for display
                #[derive(serde::Serialize)]
                struct EnvironmentDetails {
                    name: String,
                    is_active: bool,
                    api_base_url: String,
                    ui_base_url: String,
                    auth_base_url: String,
                }

                let is_active = if let Some(active_env) = configuration.get_active_environment() {
                    active_env == target_env_name
                } else {
                    false
                };

                let env_details = EnvironmentDetails {
                    name: target_env_name,
                    is_active,
                    api_base_url: env_config.api_base_url.clone(),
                    ui_base_url: env_config.ui_base_url.clone(),
                    auth_base_url: env_config.auth_base_url.clone(),
                };

                // Format the response based on the requested format
                match format {
                    OutputFormat::Json(options) => {
                        let json_output = if options.pretty {
                            serde_json::to_string_pretty(&env_details)
                        } else {
                            serde_json::to_string(&env_details)
                        };
                        match json_output {
                            Ok(json) => println!("{}", json),
                            Err(e) => {
                                return Err(crate::error::CliError::FormattingError(
                                    crate::format::FormattingError::JsonSerializationError(e),
                                ))
                            }
                        }
                    }
                    OutputFormat::Csv(options) => {
                        use crate::format::FormattingError;
                        let mut wtr = csv::Writer::from_writer(vec![]);

                        if options.with_headers {
                            if let Err(e) = wtr.serialize((
                                "ENVIRONMENT_NAME",
                                "IS_ACTIVE",
                                "API_BASE_URL",
                                "UI_BASE_URL",
                                "AUTH_BASE_URL",
                            )) {
                                return Err(crate::error::CliError::FormattingError(
                                    FormattingError::CsvError(e),
                                ));
                            }
                        }

                        if let Err(e) = wtr.serialize((
                            &env_details.name,
                            &env_details.is_active,
                            &env_details.api_base_url,
                            &env_details.ui_base_url,
                            &env_details.auth_base_url,
                        )) {
                            return Err(crate::error::CliError::FormattingError(
                                FormattingError::CsvError(e),
                            ));
                        }

                        let data = match wtr.into_inner() {
                            Ok(data) => data,
                            Err(e) => {
                                return Err(crate::error::CliError::FormattingError(
                                    FormattingError::CsvIntoInnerError(e),
                                ))
                            }
                        };
                        let csv_output = match String::from_utf8(data) {
                            Ok(csv_str) => csv_str,
                            Err(e) => {
                                return Err(crate::error::CliError::FormattingError(
                                    FormattingError::Utf8Error(e),
                                ))
                            }
                        };
                        print!("{}", csv_output);
                    }
                    OutputFormat::Tree(_) => {
                        // For tree format, output as JSON (since tree doesn't make sense for single environment)
                        let json_output = serde_json::to_string_pretty(&env_details);
                        match json_output {
                            Ok(json) => println!("{}", json),
                            Err(e) => {
                                return Err(crate::error::CliError::FormattingError(
                                    crate::format::FormattingError::JsonSerializationError(e),
                                ))
                            }
                        }
                    }
                }
            } else {
                error_utils::report_error_with_remediation(
                    &format!("Environment '{}' not found", target_env_name),
                    &[
                        "Check the environment name spelling",
                        "List available environments with 'pcli2 env list'",
                        "Add the environment with 'pcli2 env add'",
                    ],
                );
            }

            Ok(())
        }
        _ => Err(crate::error::CliError::UnsupportedSubcommand(
            matches
                .subcommand()
                .map(|(name, _)| name.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        )),
    }
}

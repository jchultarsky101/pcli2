use crate::format::OutputFormat;
use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;
use url::Url;

pub const COMMAND_CONFIG: &str = "config";
pub const COMMAND_EXPORT: &str = "export";
pub const COMMAND_SHOW: &str = "show";
pub const COMMAND_PATH: &str = "path";
pub const COMMAND_SET: &str = "set";
pub const COMMAND_DELETE: &str = "delete";
pub const COMMAND_TENANT: &str = "tenant";

pub const PARAMETER_FORMAT: &str = "format";
pub const PARAMETER_OUTPUT: &str = "output";
pub const PARAMETER_API_URL: &str = "api_url";
pub const PARAMETER_OIDC_URL: &str = "oidc_url";
pub const PARAMETER_CLIENT_ID: &str = "client_id";
pub const PARAMETER_CLIENT_SECRET: &str = "client_secret";
pub const PARAMETER_ID: &str = "id";
pub const PARAMETER_TENANT_ALIAS: &str = "alias";

pub fn create_cli_commands() -> ArgMatches {
    let format_parameter = Arg::new(PARAMETER_FORMAT)
        .short('f')
        .long(PARAMETER_FORMAT)
        .num_args(1)
        .required(false)
        .default_value("json")
        .help("Output data format")
        .value_parser(OutputFormat::names());

    let output_file_parameter = Arg::new(PARAMETER_OUTPUT)
        .short('o')
        .long(PARAMETER_OUTPUT)
        .num_args(1)
        .required(true)
        .help("output file path")
        .value_parser(clap::value_parser!(PathBuf));

    let id_parameter = Arg::new(PARAMETER_ID)
        .short('i')
        .long(PARAMETER_ID)
        .num_args(1)
        .required(true)
        .help("tenant ID");

    let tenant_alias_parameter = Arg::new(PARAMETER_TENANT_ALIAS)
        .short('a')
        .long(PARAMETER_TENANT_ALIAS)
        .num_args(1)
        .required(false)
        .help("tenant alias");

    let api_url_parameter = Arg::new(PARAMETER_API_URL)
        .long(PARAMETER_API_URL)
        .num_args(1)
        .required(true)
        .help("API URL")
        .value_parser(clap::value_parser!(Url));

    let oidc_url_parameter = Arg::new(PARAMETER_OIDC_URL)
        .long(PARAMETER_OIDC_URL)
        .num_args(1)
        .required(true)
        .help("OpenID Connect identity provider URL")
        .value_parser(clap::value_parser!(Url));

    let client_id_parameter = Arg::new(PARAMETER_CLIENT_ID)
        .long(PARAMETER_CLIENT_ID)
        .num_args(1)
        .required(true)
        .help("OpenID Connect client ID");

    let client_secret_parameter = Arg::new(PARAMETER_CLIENT_SECRET)
        .long(PARAMETER_CLIENT_SECRET)
        .num_args(1)
        .required(true)
        .help("OpenID Connect client secret");

    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            // Configuration
            Command::new(COMMAND_CONFIG)
                .about("working with configuration")
                .subcommand_required(true)
                .subcommand(
                    Command::new(COMMAND_SHOW)
                        .about("displays configuration")
                        .subcommand(Command::new(COMMAND_PATH).about("show the configuration path"))
                        .subcommand(
                            Command::new(COMMAND_TENANT)
                                .about("shows tenant configuration")
                                .arg(format_parameter.clone())
                                .arg(id_parameter.clone()),
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_EXPORT)
                        .about("exports the current configuration as a Yaml file")
                        .arg(output_file_parameter),
                )
                .subcommand(
                    Command::new(COMMAND_SET)
                        .about("sets configuration property")
                        .subcommand_required(true)
                        .subcommand(
                            Command::new(COMMAND_TENANT)
                                .about("sets tenant configuration")
                                .arg(tenant_alias_parameter.clone())
                                .arg(id_parameter.clone())
                                .arg(api_url_parameter)
                                .arg(oidc_url_parameter)
                                .arg(client_id_parameter)
                                .arg(client_secret_parameter),
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_DELETE).about("delete").subcommand(
                        Command::new(COMMAND_TENANT)
                            .about("deletes a tenant")
                            .arg(id_parameter.clone()),
                    ),
                ),
        )
        .get_matches()
}

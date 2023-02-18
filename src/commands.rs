use crate::configuration::OutputFormat;
use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

pub const COMMAND_CONFIG: &str = "config";
pub const COMMAND_EXPORT: &str = "export";
pub const COMMAND_SHOW: &str = "show";
pub const COMMAND_PATH: &str = "path";
pub const COMMAND_SET: &str = "set";
pub const COMMAND_TENANT: &str = "tenant";

pub const PARAMETER_FORMAT: &str = "format";
pub const PARAMETER_OUTPUT: &str = "output";

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
                            Command::new(COMMAND_TENANT).about("shows tenant configuration"),
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
                            Command::new(COMMAND_TENANT).about("sets tenant configuration"),
                        ),
                ),
        )
        .get_matches()
}

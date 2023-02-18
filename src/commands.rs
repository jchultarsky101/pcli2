use crate::configuration::OutputFormat;
use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

pub const COMMAND_NAME_CONFIG: &str = "config";
pub const COMMAND_NAME_EXPORT: &str = "export";
pub const COMMAND_NAME_SHOW: &str = "show";
pub const COMMAND_NAME_PATH: &str = "path";
pub const COMMAND_NAME_ALL: &str = "all";

pub const PARAMETER_NAME_FORMAT: &str = "format";
pub const PARAMETER_NAME_OUTPUT: &str = "output";

pub fn create_cli_commands() -> ArgMatches {
    let format_parameter = Arg::new(PARAMETER_NAME_FORMAT)
        .short('f')
        .long(PARAMETER_NAME_FORMAT)
        .num_args(1)
        .required(false)
        .default_value("json")
        .help("Output data format")
        .value_parser(OutputFormat::names());

    let output_file_parameter = Arg::new(PARAMETER_NAME_OUTPUT)
        .short('o')
        .long(PARAMETER_NAME_OUTPUT)
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
            Command::new(COMMAND_NAME_CONFIG)
                .about("working with configuration")
                .subcommand(
                    Command::new(COMMAND_NAME_SHOW)
                        .about("displays configuration")
                        .subcommand(
                            Command::new(COMMAND_NAME_PATH).about("show the configuration path"),
                        )
                        .subcommand(
                            Command::new(COMMAND_NAME_ALL)
                                .about("shows all valid configuration property names")
                                .arg(format_parameter),
                        ),
                )
                .subcommand(
                    Command::new(COMMAND_NAME_EXPORT)
                        .about("exports the current configuration to a file")
                        .arg(output_file_parameter),
                ),
        )
        .get_matches()
}

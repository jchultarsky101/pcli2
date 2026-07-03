//! Man page command definitions.
//!
//! This module defines the CLI command for generating man pages.

use clap::Command;

/// Create the man page generation command.
pub fn man_command() -> Command {
    Command::new("man")
        .about("Generate man pages for pcli2 and all its subcommands")
        .arg(
            clap::Arg::new("output-dir")
                .long("output-dir")
                .short('o')
                .help("Directory to write the man pages to (created if missing)")
                .default_value(".")
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
}

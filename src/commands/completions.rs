//! Completions command definitions.
//!
//! This module defines the CLI command for generating shell completions.

use clap::Command;

/// Create the completions command.
pub fn completions_command() -> Command {
    Command::new("completions")
        .about("Generate shell completions for various shells")
        .arg(
            clap::Arg::new("shell")
                .help("The shell to generate completions for")
                .required(true)
                .value_parser([
                    "bash",
                    "zsh",
                    "fish",
                    "powershell",
                    "elvish",
                ])
        )
}
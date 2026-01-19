//! Completions action logic.
//!
//! This module contains the logic for generating shell completions.

use crate::error::CliError;
use std::io;

/// Generate completions for the specified shell and write to stdout.
pub fn generate_completions(shell: &str) -> Result<(), CliError> {
    // We need to get the full CLI command structure, not just the completions command
    let mut cmd = crate::commands::create_full_command();

    match shell {
        "bash" => {
            clap_complete::generate(
                clap_complete::Shell::Bash,
                &mut cmd,
                "pcli2",
                &mut io::stdout(),
            );
        }
        "zsh" => {
            clap_complete::generate(
                clap_complete::Shell::Zsh,
                &mut cmd,
                "pcli2",
                &mut io::stdout(),
            );
        }
        "fish" => {
            clap_complete::generate(
                clap_complete::Shell::Fish,
                &mut cmd,
                "pcli2",
                &mut io::stdout(),
            );
        }
        "powershell" => {
            clap_complete::generate(
                clap_complete::Shell::PowerShell,
                &mut cmd,
                "pcli2",
                &mut io::stdout(),
            );
        }
        "elvish" => {
            clap_complete::generate(
                clap_complete::Shell::Elvish,
                &mut cmd,
                "pcli2",
                &mut io::stdout(),
            );
        }
        _ => {
            return Err(CliError::UnsupportedSubcommand(format!("Unsupported shell: {}", shell)));
        }
    }

    Ok(())
}
//! Terminal capability detection for user-facing output.
//!
//! This module centralizes the logic that decides whether decorated
//! (colored) output should be produced. It honors the `NO_COLOR`
//! convention (<https://no-color.org>), the `PCLI2_NO_COLOR` environment
//! variable, the global `--no-color` flag, and whether stdout is attached
//! to a terminal.

use std::env;
use std::io::IsTerminal;

/// Returns true when the given environment variable is set to a non-empty value.
fn env_var_set(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

/// Determine whether colored output should be emitted on stdout.
///
/// Colors are disabled when any of the following holds:
/// - the `NO_COLOR` environment variable is set to a non-empty value
/// - the `PCLI2_NO_COLOR` environment variable is set to a non-empty value
/// - the `--no-color` flag is present on the command line
/// - stdout is not attached to a terminal (e.g. output is piped)
pub fn colors_enabled() -> bool {
    !env_var_set("NO_COLOR")
        && !env_var_set("PCLI2_NO_COLOR")
        && !env::args().any(|arg| arg == "--no-color")
        && std::io::stdout().is_terminal()
}

/// Create a spinner shown on stderr while a quick operation runs.
///
/// The spinner is hidden when stderr is not attached to a terminal, so it
/// never pollutes redirected output or CI logs. Callers should invoke
/// `finish_and_clear()` when the operation completes.
pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let progress_bar = if std::io::stderr().is_terminal() {
        indicatif::ProgressBar::new_spinner()
    } else {
        indicatif::ProgressBar::hidden()
    };
    progress_bar.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("valid spinner template"),
    );
    progress_bar.set_message(message.to_string());
    progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));
    progress_bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_var_set() {
        // An unlikely-to-exist variable is reported as unset
        assert!(!env_var_set("PCLI2_TEST_UNSET_VARIABLE_XYZ"));
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_var_set() {
        // An unlikely-to-exist variable is reported as unset
        assert!(!env_var_set("PCLI2_TEST_UNSET_VARIABLE_XYZ"));
    }
}

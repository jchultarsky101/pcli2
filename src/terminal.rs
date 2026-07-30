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

/// Returns true when the user explicitly disabled colors via the
/// `NO_COLOR`/`PCLI2_NO_COLOR` environment variables or the `--no-color`
/// command-line flag.
fn colors_disabled_by_user() -> bool {
    env_var_set("NO_COLOR")
        || env_var_set("PCLI2_NO_COLOR")
        || env::args().any(|arg| arg == "--no-color")
}

/// Determine whether colored output should be emitted on stdout.
///
/// Colors are disabled when the user opted out (see
/// `colors_disabled_by_user`) or when stdout is not attached to a
/// terminal (e.g. output is piped).
pub fn colors_enabled() -> bool {
    !colors_disabled_by_user() && std::io::stdout().is_terminal()
}

/// Determine whether colored output should be emitted on stderr
/// (diagnostics: tracing logs, warnings).
///
/// Same rules as `colors_enabled`, but checks stderr - it can be
/// redirected independently of stdout (e.g. `pcli2 ... 2>errors.log`).
pub fn stderr_colors_enabled() -> bool {
    !colors_disabled_by_user() && std::io::stderr().is_terminal()
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

/// Template for a phase whose amount of work is not known up front.
const REPORT_SPINNER_TEMPLATE: &str = "{spinner:.green} [{elapsed_precise}] {msg}";

/// Template for a phase with a known item count.
///
/// Both templates carry the elapsed time. The question these phases exist to answer
/// is "is this stuck?", and a number that keeps climbing answers it more
/// convincingly than an animation alone.
const REPORT_COUNTER_TEMPLATE: &str = "{spinner:.green} [{elapsed_precise}] {msg} [{bar:30.cyan/blue}] {human_pos}/{human_len} ({percent}%) ETA {eta}";

/// How often a row-by-row phase refreshes its counter.
///
/// Redrawing on every row would spend more time formatting than doing the work being
/// reported on; a few million rows at this interval is a few dozen redraws.
const REPORT_ROW_INTERVAL: usize = 25_000;

/// Whether a row-loop iteration should refresh the counter.
///
/// Always true for the first row, so a loop shorter than one interval still shows
/// that the phase started rather than leaving the previous phase's message up.
fn should_report_row(index: usize) -> bool {
    index.is_multiple_of(REPORT_ROW_INTERVAL)
}

/// Progress reporting for the phases of a long operation that run after the network
/// work is done.
///
/// Flattening matches into rows, serializing them, and writing them out all happen
/// *after* a match progress bar has already reached 100%, and on a large result set
/// that takes minutes with no output at all - which reads as a hang. This keeps the
/// terminal honest about the fact that work is still happening, and about which
/// phase is responsible for the wait.
///
/// A disabled reporter is a no-op at every method, so callers need no branching. The
/// underlying bar also hides itself when stderr is not a terminal, and everything it
/// writes goes to stderr, so piped `stdout` is unaffected either way.
pub struct ReportProgress {
    bar: Option<indicatif::ProgressBar>,
    started: std::time::Instant,
}

impl ReportProgress {
    /// Create a reporter, active only when `show_progress` is set, showing `message`
    /// as its first phase.
    pub fn new(show_progress: bool, message: &str) -> Self {
        let bar = if show_progress {
            let bar = if std::io::stderr().is_terminal() {
                indicatif::ProgressBar::new_spinner()
            } else {
                indicatif::ProgressBar::hidden()
            };
            bar.enable_steady_tick(std::time::Duration::from_millis(100));
            Some(bar)
        } else {
            None
        };

        let progress = Self {
            bar,
            started: std::time::Instant::now(),
        };
        progress.phase(message.to_string());
        progress
    }

    /// A reporter that never displays anything, for callers with no progress flag.
    pub fn disabled() -> Self {
        Self::new(false, "")
    }

    /// Begin a phase whose amount of work is not known up front.
    pub fn phase(&self, message: impl Into<String>) {
        if let Some(bar) = &self.bar {
            bar.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .template(REPORT_SPINNER_TEMPLATE)
                    .expect("valid spinner template"),
            );
            bar.set_message(message.into());
        }
    }

    /// Begin a phase that will process `total` items, displayed as a percentage bar
    /// with an ETA. An empty phase falls back to a plain message - a bar that can
    /// only ever read 0/0 is noise.
    pub fn start_rows(&self, message: impl Into<String>, total: usize) {
        if total == 0 {
            self.phase(message);
            return;
        }
        if let Some(bar) = &self.bar {
            bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template(REPORT_COUNTER_TEMPLATE)
                    .expect("valid counter template")
                    .progress_chars("#>-"),
            );
            bar.set_message(message.into());
            bar.set_length(total as u64);
            bar.set_position(0);
        }
    }

    /// Advance the counter of the current row phase. `index` is the zero-based index
    /// of the row about to be processed, so the bar reads as items completed.
    pub fn set_row(&self, index: usize) {
        if let Some(bar) = &self.bar {
            if should_report_row(index) {
                bar.set_position(index as u64);
            }
        }
    }

    /// Clear the display. Must happen before anything is written to stdout, so the
    /// bar does not end up sitting in the middle of the report.
    pub fn finish(&self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }

    /// Clear the display and print `summary` with the total elapsed time, so the
    /// user is left with a record of what the wait bought them.
    pub fn finish_with_summary(&self, summary: &str) {
        self.finish();
        if self.bar.is_some() {
            eprintln!(
                "{} in {}",
                summary,
                indicatif::HumanDuration(self.started.elapsed())
            );
        }
    }
}

impl Drop for ReportProgress {
    /// Safety net for the early returns and error paths that do not reach an
    /// explicit `finish()` - a bar left ticking would outlive the command.
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_var_set() {
        // An unlikely-to-exist variable is reported as unset
        assert!(!env_var_set("PCLI2_TEST_UNSET_VARIABLE_XYZ"));
    }

    #[test]
    fn row_progress_reports_first_row_then_throttles() {
        // The first row always reports, so a short loop still shows that the phase
        // started instead of leaving the previous phase's message up.
        assert!(should_report_row(0));

        // Everything up to the next interval boundary stays quiet - redrawing a few
        // million times would cost more than the work being reported on.
        assert!(!should_report_row(1));
        assert!(!should_report_row(REPORT_ROW_INTERVAL - 1));

        // Interval boundaries report.
        assert!(should_report_row(REPORT_ROW_INTERVAL));
        assert!(should_report_row(REPORT_ROW_INTERVAL * 2));
        assert!(!should_report_row(REPORT_ROW_INTERVAL * 2 + 1));
    }

    #[test]
    fn report_progress_is_silent_without_the_progress_flag() {
        // Without --progress nothing is constructed at all, so no bar can leak into
        // output that a caller is piping.
        let quiet = ReportProgress::new(false, "Building report...");
        assert!(quiet.bar.is_none());
        assert!(ReportProgress::disabled().bar.is_none());

        // These must all be no-ops rather than panicking on the absent bar.
        quiet.phase("still nothing");
        quiet.start_rows("still nothing", 10);
        quiet.set_row(0);
        quiet.finish();
        quiet.finish_with_summary("still nothing");
    }

    #[test]
    fn both_phase_styles_parse() {
        // Both templates are applied behind an `expect`, so a typo in either one is a
        // panic at the worst possible moment - minutes into a long report.
        let progress = ReportProgress::new(true, "Building report...");
        progress.start_rows("Building rows", 100);
        progress.set_row(0);
        progress.set_row(50);
        progress.phase("Serializing JSON...");
        progress.finish_with_summary("Built report of 100 row(s)");
    }

    #[test]
    fn empty_row_phase_does_not_become_a_bar() {
        // A 0/0 bar with a meaningless ETA is worse than a plain message; the phase
        // still has to report, because it may be the one the user is waiting on.
        let progress = ReportProgress::new(true, "Building report...");
        let before = progress.bar.as_ref().and_then(|bar| bar.length());
        progress.start_rows("Formatting CSV", 0);
        let after = progress.bar.as_ref().and_then(|bar| bar.length());
        assert_eq!(before, after, "an empty phase must not set a bar length");
        progress.finish();
    }
}

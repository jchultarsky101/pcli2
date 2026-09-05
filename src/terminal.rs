//! Terminal capability detection for user-facing output.
//!
//! This module centralizes the logic that decides whether decorated
//! (colored) output should be produced. It honors the `NO_COLOR`
//! convention (<https://no-color.org>), the `PCLI2_NO_COLOR` environment
//! variable, the global `--no-color` flag, and whether stdout is attached
//! to a terminal.

use std::env;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

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
            // Length and position first, style last. The steady tick redraws on its
            // own thread, so flipping to the counter template before the length is
            // set leaves a window where it renders a meaningless "0/0 (0%)".
            bar.set_length(total as u64);
            bar.set_position(0);
            bar.set_message(message.into());
            bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template(REPORT_COUNTER_TEMPLATE)
                    .expect("valid counter template")
                    .progress_chars("#>-"),
            );
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

    /// Run `f` with the display temporarily cleared.
    ///
    /// Warnings and the bar both go to stderr, so printing one while the other is
    /// drawing leaves the message shredded across a redraw. Use this for anything
    /// that writes to stderr mid-phase.
    pub fn suspend<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        match &self.bar {
            Some(bar) => bar.suspend(f),
            None => f(),
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

static NO_INPUT: AtomicBool = AtomicBool::new(false);

/// Record the global `--no-input` flag.
pub fn set_no_input(no_input: bool) {
    NO_INPUT.store(no_input, Ordering::SeqCst);
}

/// Whether `--no-input` (or `PCLI2_NO_INPUT`) is in effect.
pub fn no_input() -> bool {
    NO_INPUT.load(Ordering::SeqCst)
}

/// Why a prompt cannot be shown right now, if it cannot.
///
/// A prompt needs a terminal on both ends: stdin to read the answer and stderr
/// to show the question. Without one, a prompt either hangs a script or reads
/// end-of-file and takes that as an answer. `--no-input` says so explicitly.
pub fn prompt_unavailable_reason() -> Option<&'static str> {
    if no_input() {
        Some("--no-input was given")
    } else if !std::io::stdin().is_terminal() {
        Some("stdin is not a terminal")
    } else if !std::io::stderr().is_terminal() {
        Some("stderr is not a terminal")
    } else {
        None
    }
}

/// Whether an interactive prompt may be shown.
pub fn prompts_allowed() -> bool {
    prompt_unavailable_reason().is_none()
}

/// A prompt that could not be shown. Converts into either error type, so
/// every command reports it the same way: usage error, exit 64.
#[derive(Debug)]
pub struct PromptUnavailable(pub String);

impl std::fmt::Display for PromptUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<PromptUnavailable> for crate::error::CliError {
    fn from(e: PromptUnavailable) -> Self {
        crate::error::CliError::InputRequired(e.0)
    }
}

impl From<PromptUnavailable> for crate::actions::CliActionError {
    fn from(e: PromptUnavailable) -> Self {
        crate::actions::CliActionError::InputRequired(e.0)
    }
}

/// Fail when the command would have to prompt for `missing`.
///
/// Used before a selection menu: a script that forgot `--name` gets an error
/// naming the flag instead of a menu it cannot answer.
pub fn require_prompt(missing: &str) -> Result<(), PromptUnavailable> {
    match prompt_unavailable_reason() {
        None => Ok(()),
        Some(reason) => Err(PromptUnavailable(format!(
            "{} is required because no prompt can be shown ({})",
            missing, reason
        ))),
    }
}

/// Ask a yes/no question, defaulting to no.
///
/// Refuses, rather than answering for the user, when no prompt can be shown.
/// A confirmation that read end-of-file as "no" used to exit 0 as "cancelled",
/// which scripts took for success.
pub fn confirm(question: &str, help: Option<&str>) -> Result<bool, PromptUnavailable> {
    if let Some(reason) = prompt_unavailable_reason() {
        return Err(PromptUnavailable(format!(
            "confirmation required for \"{}\" but no prompt can be shown ({}); nothing was changed. Pass --yes to proceed without one",
            question, reason
        )));
    }
    let mut prompt = inquire::Confirm::new(question).with_default(false);
    if let Some(help) = help {
        prompt = prompt.with_help_message(help);
    }
    prompt.prompt().map_err(|e| {
        PromptUnavailable(format!(
            "confirmation prompt for \"{}\" failed ({}); nothing was changed. Pass --yes to proceed without one",
            question, e
        ))
    })
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

//! Running one operation over many items: the engine behind `folder download`,
//! `folder thumbnail` and `folder upload`.
//!
//! The three commands had grown three copies of the same loop, and only the
//! download copy had been fixed over time: without `--continue-on-error` the other
//! two returned on the first failure while their remaining tasks carried on
//! uploading in the background, printed no summary, left their progress bars
//! painted over the error, and printed warnings through the bars. The rules now
//! live here:
//!
//! - at most `concurrency` items run at once;
//! - without `--continue-on-error` the first failure stops the run: items that
//!   have not started are cancelled and counted as not attempted;
//! - failures are collected and reported after the progress bars are cleared.

use crate::error::CliError;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// How to run a bulk operation.
#[derive(Debug, Clone)]
pub struct BulkOptions {
    pub concurrency: usize,
    /// Wait this long before each item that does real work (see [`ItemContext::pace`]).
    pub delay: Duration,
    pub continue_on_error: bool,
    pub show_progress: bool,
    /// Shown on a running item's spinner: "Downloading", "Uploading".
    pub verb: &'static str,
}

/// How one item ended, when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemOutcome {
    /// The work was done.
    Done,
    /// Nothing needed doing (already on disk, already uploaded).
    Skipped,
    /// There was nothing to fetch for this item (an asset without a thumbnail).
    Unavailable,
}

/// A failed item: the line for the error list, and the error itself.
pub struct ItemFailure {
    pub message: String,
    pub error: CliError,
}

/// What a bulk run achieved.
#[derive(Default)]
pub struct BulkReport {
    pub total: usize,
    pub done: usize,
    pub skipped: usize,
    pub unavailable: usize,
    /// Cancelled after an earlier failure, without `--continue-on-error`.
    pub not_attempted: usize,
    pub failure_messages: Vec<String>,
    /// The first failure, which is the run's error without `--continue-on-error`.
    pub first_error: Option<CliError>,
}

impl BulkReport {
    pub fn failed(&self) -> usize {
        self.failure_messages.len()
    }

    /// Print the failures (after the summary, so they stay on screen).
    pub fn print_failures(&self) {
        if self.failure_messages.is_empty() {
            return;
        }
        eprintln!();
        eprintln!("📋 Detailed Error List:");
        eprintln!("======================");
        for message in &self.failure_messages {
            eprintln!("{}", message);
        }
    }

    /// The run's result: the first failure without `--continue-on-error`, a
    /// partial failure when some items failed anyway, otherwise success.
    /// `--continue-on-error` decides whether the run went on, not whether the exit
    /// code tells the truth.
    pub fn into_result(self, continue_on_error: bool, what: &str) -> Result<(), CliError> {
        let failed = self.failed();
        if let Some(error) = self.first_error {
            if !continue_on_error {
                return Err(error);
            }
        }
        if failed > 0 {
            return Err(CliError::ActionError(
                crate::actions::CliActionError::PartialFailure {
                    failed,
                    total: self.total,
                    what: what.to_string(),
                },
            ));
        }
        Ok(())
    }
}

/// What a running item may use.
#[derive(Clone)]
pub struct ItemContext {
    delay: Duration,
    progress: Option<MultiProgress>,
}

impl ItemContext {
    /// Wait out `--delay`. Called by an item right before its real work, so an
    /// item that turns out to need nothing (skipped under `--resume`) does not wait.
    pub async fn pace(&self) {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
    }

    /// Print a line on stderr without tearing the progress bars.
    pub fn note(&self, message: &str) {
        match &self.progress {
            Some(progress) => progress.suspend(|| eprintln!("{}", message)),
            None => eprintln!("{}", message),
        }
    }
}

/// Run `work` over `items` (each labelled for its spinner) under `options`.
pub async fn run_bulk<T, F, Fut>(
    items: Vec<(String, T)>,
    options: &BulkOptions,
    work: F,
) -> BulkReport
where
    T: Send + 'static,
    F: Fn(T, ItemContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ItemOutcome, ItemFailure>> + Send + 'static,
{
    let total = items.len();
    let semaphore = Arc::new(Semaphore::new(options.concurrency.max(1)));
    let work = Arc::new(work);

    let (overall, progress) = if options.show_progress {
        let progress = MultiProgress::new();
        let bar = progress.add(ProgressBar::new(total as u64));
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
                .expect("static template")
                .progress_chars("#>-"),
        );
        (Some(bar), Some(progress))
    } else {
        (None, None)
    };
    let context = ItemContext {
        delay: options.delay,
        progress: progress.clone(),
    };

    let mut tasks = Vec::with_capacity(total);
    for (label, item) in items {
        let semaphore = semaphore.clone();
        let work = work.clone();
        let context = context.clone();
        let overall = overall.clone();
        // A spinner per running item only helps when several run at once.
        let spinner_parent = progress.clone().filter(|_| options.concurrency > 1);
        let verb = options.verb;
        tasks.push(tokio::spawn(async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("semaphore is never closed");
            let spinner = spinner_parent.map(|parent| {
                let spinner = parent.add(ProgressBar::new_spinner());
                spinner.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.yellow} [{elapsed_precise}] {msg}")
                        .expect("static template"),
                );
                spinner.set_message(format!("{}: {}", verb, label));
                spinner
            });
            let result = work(item, context).await;
            if let Some(spinner) = spinner {
                spinner.finish_and_clear();
            }
            if let Some(overall) = overall {
                overall.inc(1);
            }
            result
        }));
    }

    // Aborting a task that has already finished is a no-op.
    let abort_handles: Vec<_> = tasks.iter().map(|task| task.abort_handle()).collect();
    let mut report = BulkReport {
        total,
        ..Default::default()
    };
    for task in tasks {
        match task.await {
            Ok(Ok(ItemOutcome::Done)) => report.done += 1,
            Ok(Ok(ItemOutcome::Skipped)) => report.skipped += 1,
            Ok(Ok(ItemOutcome::Unavailable)) => report.unavailable += 1,
            Ok(Err(failure)) => {
                report.failure_messages.push(failure.message);
                if report.first_error.is_none() {
                    report.first_error = Some(failure.error);
                    if !options.continue_on_error {
                        abort_handles.iter().for_each(|handle| handle.abort());
                    }
                }
            }
            Err(join_error) if join_error.is_cancelled() => report.not_attempted += 1,
            Err(join_error) => {
                report
                    .failure_messages
                    .push(format!("⚠️  Task failed to execute: {}", join_error));
                if report.first_error.is_none() {
                    report.first_error = Some(CliError::from(std::io::Error::other(
                        join_error.to_string(),
                    )));
                    if !options.continue_on_error {
                        abort_handles.iter().for_each(|handle| handle.abort());
                    }
                }
            }
        }
    }

    // Cleared before anything else is printed, so the summary is not painted over.
    if let Some(overall) = overall {
        overall.finish_and_clear();
    }
    if let Some(progress) = progress {
        let _ = progress.clear();
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn options(continue_on_error: bool) -> BulkOptions {
        BulkOptions {
            concurrency: 1,
            delay: Duration::ZERO,
            continue_on_error,
            show_progress: false,
            verb: "Testing",
        }
    }

    fn items(n: usize) -> Vec<(String, usize)> {
        (0..n).map(|i| (format!("item {i}"), i)).collect()
    }

    #[tokio::test]
    async fn the_first_failure_stops_the_run_without_continue_on_error() {
        let started = Arc::new(AtomicUsize::new(0));
        let counter = started.clone();
        let report = run_bulk(items(5), &options(false), move |i, _| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                // Real items take time; instantaneous ones would all finish
                // before the failure is collected.
                tokio::time::sleep(Duration::from_millis(30)).await;
                if i == 1 {
                    Err(ItemFailure {
                        message: "item 1 failed".into(),
                        error: CliError::MissingRequiredArgument("x".into()),
                    })
                } else {
                    Ok(ItemOutcome::Done)
                }
            }
        })
        .await;
        assert_eq!(report.done, 1);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.not_attempted, 3);
        assert!(started.load(Ordering::SeqCst) < 5, "later items never ran");
        assert!(report.into_result(false, "tests").is_err());
    }

    #[tokio::test]
    async fn continue_on_error_runs_everything_and_still_fails_the_run() {
        let report = run_bulk(items(4), &options(true), |i, _| async move {
            match i {
                0 => Ok(ItemOutcome::Skipped),
                2 => Err(ItemFailure {
                    message: "item 2 failed".into(),
                    error: CliError::MissingRequiredArgument("x".into()),
                }),
                _ => Ok(ItemOutcome::Done),
            }
        })
        .await;
        assert_eq!(
            (
                report.done,
                report.skipped,
                report.failed(),
                report.not_attempted
            ),
            (2, 1, 1, 0)
        );
        match report.into_result(true, "tests") {
            Err(CliError::ActionError(crate::actions::CliActionError::PartialFailure {
                failed,
                total,
                ..
            })) => assert_eq!((failed, total), (1, 4)),
            _ => panic!("expected a partial failure"),
        }
    }
}

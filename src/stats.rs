//! Per-run request statistics.
//!
//! Counts every HTTP attempt, every transient retry and every token renewal so a
//! run can say what it cost the server. Printed on stderr at exit with `--stats`,
//! and always logged at debug level. The review that motivated this found runs
//! issuing tens of thousands of avoidable requests with nothing visible to the
//! person running them.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

static REQUESTS: AtomicU64 = AtomicU64::new(0);
static RETRIES: AtomicU64 = AtomicU64::new(0);
static RENEWALS: AtomicU64 = AtomicU64::new(0);
static START: OnceLock<Instant> = OnceLock::new();

/// Mark the start of the run; later calls are ignored.
pub fn start() {
    let _ = START.set(Instant::now());
}

/// One HTTP attempt went out (retries count again).
pub fn record_request() {
    REQUESTS.fetch_add(1, Ordering::Relaxed);
}

/// A transient failure is being retried.
pub fn record_retry() {
    RETRIES.fetch_add(1, Ordering::Relaxed);
}

/// The access token was renewed against the auth server.
pub fn record_renewal() {
    RENEWALS.fetch_add(1, Ordering::Relaxed);
}

/// A snapshot of the counters.
pub fn snapshot() -> (u64, u64, u64, f64) {
    let elapsed = START
        .get()
        .map(|s| s.elapsed().as_secs_f64())
        .unwrap_or(0.0);
    (
        REQUESTS.load(Ordering::Relaxed),
        RETRIES.load(Ordering::Relaxed),
        RENEWALS.load(Ordering::Relaxed),
        elapsed,
    )
}

/// One line: `1,284 request(s), 3 retried, 1 token renewal(s) in 4m12s`.
pub fn summary() -> String {
    let (requests, retries, renewals, elapsed) = snapshot();
    format!(
        "{} API request(s), {} retried, {} token renewal(s) in {}",
        group_thousands(requests),
        group_thousands(retries),
        group_thousands(renewals),
        human_duration(elapsed)
    )
}

/// Print the summary on stderr when asked; always leave it in the debug log.
pub fn report(print: bool) {
    let line = summary();
    if print {
        if crate::error_utils::json_errors() {
            let (requests, retries, renewals, elapsed) = snapshot();
            eprintln!(
                "{}",
                serde_json::json!({
                    "level": "INFO",
                    "message": line,
                    "requests": requests,
                    "retries": retries,
                    "renewals": renewals,
                    "elapsed_seconds": (elapsed * 10.0).round() / 10.0,
                })
            );
        } else {
            eprintln!("📊 {}", line);
        }
    } else {
        tracing::debug!("{}", line);
    }
}

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn human_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{:.1}s", seconds)
    } else if seconds < 3600.0 {
        format!(
            "{}m{:02}s",
            (seconds / 60.0) as u64,
            (seconds % 60.0) as u64
        )
    } else {
        format!(
            "{}h{:02}m",
            (seconds / 3600.0) as u64,
            ((seconds % 3600.0) / 60.0) as u64
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1000), "1,000");
        assert_eq!(group_thousands(1234567), "1,234,567");
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(human_duration(4.26), "4.3s");
        assert_eq!(human_duration(252.0), "4m12s");
        assert_eq!(human_duration(3725.0), "1h02m");
    }
}

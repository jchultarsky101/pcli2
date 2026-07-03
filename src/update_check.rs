//! New-version notification.
//!
//! After a successful command, pcli2 may print a one-line hint on stderr
//! when a newer release is available on GitHub. The check is designed to
//! never get in the user's way:
//!
//! - it runs at most once per 24 hours (the result is cached on disk)
//! - it is skipped entirely in non-interactive sessions (stderr not a
//!   terminal), in CI (the `CI` environment variable), or when the user
//!   opts out with `PCLI2_NO_UPDATE_CHECK`
//! - network failures are silently ignored and the request times out
//!   after a few seconds

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::debug;

/// GitHub API endpoint for the latest pcli2 release.
const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/jchultarsky101/pcli2/releases/latest";

/// How long a cached check result stays fresh.
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Timeout for the release lookup; a hint is never worth a slow exit.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// On-disk cache of the last check.
#[derive(Debug, Serialize, Deserialize)]
struct UpdateCheckCache {
    /// Unix timestamp (seconds) of the last successful check
    last_checked: u64,
    /// Latest version reported by GitHub at that time (without leading 'v')
    latest_version: String,
}

/// Relevant subset of the GitHub release response.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

/// Print a one-line hint on stderr when a newer release is available.
///
/// All failures (network, parse, filesystem) are logged at debug level
/// and otherwise ignored: this is a convenience, not a feature the user
/// should ever see fail.
pub async fn maybe_print_update_hint() {
    use std::io::IsTerminal;

    if std::env::var_os("PCLI2_NO_UPDATE_CHECK").is_some_and(|v| !v.is_empty())
        || std::env::var_os("CI").is_some_and(|v| !v.is_empty())
        || !std::io::stderr().is_terminal()
    {
        return;
    }

    let current = env!("CARGO_PKG_VERSION");
    if let Some(latest) = latest_version().await {
        if is_newer(&latest, current) {
            eprintln!(
                "\n💡 A new version of pcli2 is available: v{} → v{} (https://github.com/jchultarsky101/pcli2/releases/latest)",
                current, latest
            );
        }
    }
}

/// Get the latest released version, from the cache when fresh, otherwise
/// from the GitHub API.
///
/// The check timestamp is recorded even when the fetch fails (offline,
/// firewalled, rate-limited), so a failed check is not re-attempted on
/// every command - at most one network attempt is made per cache window.
async fn latest_version() -> Option<String> {
    let cache_path = crate::cache::BaseCache::get_cache_dir().join("update-check.json");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let cached: Option<UpdateCheckCache> = std::fs::read_to_string(&cache_path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok());

    // Serve from cache while fresh
    if let Some(ref cache) = cached {
        if now.saturating_sub(cache.last_checked) < CACHE_TTL.as_secs() {
            return Some(cache.latest_version.clone());
        }
    }

    // Cache is stale or missing: ask GitHub. On failure, fall back to the
    // previously known version (a release that existed still exists), or
    // the current version so the timestamp still advances.
    let latest = fetch_latest_version()
        .await
        .or(cached.map(|cache| cache.latest_version))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let cache = UpdateCheckCache {
        last_checked: now,
        latest_version: latest.clone(),
    };
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(serialized) = serde_json::to_string(&cache) {
        if let Err(e) = std::fs::write(&cache_path, serialized) {
            debug!("Failed to write update-check cache: {}", e);
        }
    }

    Some(latest)
}

/// Query the GitHub API for the latest release tag.
async fn fetch_latest_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()?;

    let response = client
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", "PCLI2")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| debug!("Update check request failed: {}", e))
        .ok()?;

    if !response.status().is_success() {
        debug!("Update check returned status {}", response.status());
        return None;
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| debug!("Update check parse failed: {}", e))
        .ok()?;

    Some(release.tag_name.trim_start_matches('v').trim().to_string())
}

/// Parse a semantic version string ("1.8.2") into a numeric triple.
///
/// Missing components default to 0; pre-release/build suffixes on the
/// last component are ignored (e.g. "1.9.0-rc1" parses as (1, 9, 0)).
fn parse_version(version: &str) -> (u64, u64, u64) {
    let mut parts = version.split('.').map(|part| {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Returns true when `candidate` is a strictly newer version than `current`.
fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.8.2"), (1, 8, 2));
        assert_eq!(parse_version("2.0"), (2, 0, 0));
        assert_eq!(parse_version("1.9.0-rc1"), (1, 9, 0));
        assert_eq!(parse_version("garbage"), (0, 0, 0));
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.9.0", "1.8.2"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.8.2", "1.8.2"));
        assert!(!is_newer("1.8.1", "1.8.2"));
    }
}

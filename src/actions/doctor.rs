//! `pcli2 doctor`: the local setup in one screen.
//!
//! Where the binary is (and whether there are others on the PATH), whether the
//! configuration loads, which environment and tenant are active, where the
//! credentials live and whether a usable token is stored, how old the caches are,
//! whether the API and the auth server answer, and whether a newer release exists.
//! Half of the support round trips in the changelog would have ended here.

use crate::configuration::Configuration;
use crate::error::CliError;
use crate::exit_codes::PcliExitCode;
use crate::keyring::Keyring;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Serialize)]
struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

#[derive(Serialize)]
struct Report {
    version: String,
    checks: Vec<Check>,
}

fn check(name: &'static str, status: Status, detail: impl Into<String>) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
    }
}

pub async fn run(sub_matches: &clap::ArgMatches) -> Result<(), CliError> {
    let as_json = sub_matches
        .get_one::<String>("format")
        .map(|f| f.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let mut checks: Vec<Check> = Vec::new();

    // Binary and PATH
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let on_path = pcli2_binaries_on_path();
    if on_path.len() > 1 {
        checks.push(check(
            "binary",
            Status::Warn,
            format!(
                "running {} v{}; {} pcli2 executables on PATH ({}) - \"pcli2\" may not be the one you expect",
                exe,
                env!("CARGO_PKG_VERSION"),
                on_path.len(),
                on_path
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    } else {
        checks.push(check(
            "binary",
            Status::Ok,
            format!("{} v{}", exe, env!("CARGO_PKG_VERSION")),
        ));
    }

    // Configuration
    let config_path = Configuration::get_default_configuration_file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| format!("(unresolvable: {})", e));
    let configuration = match Configuration::load_default() {
        Ok(c) => {
            checks.push(check("configuration", Status::Ok, config_path.clone()));
            Some(c)
        }
        Err(e) => {
            checks.push(check(
                "configuration",
                Status::Fail,
                format!("{}: {}", config_path, e),
            ));
            None
        }
    };

    // Environment
    let environment_name = configuration
        .as_ref()
        .and_then(|c| c.get_active_environment());
    if let Some(cfg) = &configuration {
        match &environment_name {
            Some(name) => checks.push(check(
                "environment",
                Status::Ok,
                format!(
                    "{} (API {}, auth {})",
                    name,
                    cfg.get_api_base_url(),
                    cfg.get_auth_base_url()
                ),
            )),
            None => checks.push(check(
                "environment",
                Status::Warn,
                format!(
                    "none selected; using defaults (API {}) - 'pcli2 env use' to pick one",
                    cfg.get_api_base_url()
                ),
            )),
        }
    }
    let env_key = environment_name
        .clone()
        .unwrap_or_else(|| "default".to_string());

    // Credentials store
    #[allow(unused_mut)]
    let mut keyring = Keyring::default();
    let (token, client_id, client_secret) = keyring
        .get_environment_credentials(&env_key)
        .unwrap_or((None, None, None));
    checks.push(credentials_check(
        &env_key,
        client_id.is_some(),
        client_secret.is_some(),
    ));

    // Token
    let mut token_usable = false;
    match token.as_deref() {
        Some(t) => match crate::physna_v3::PhysnaApiClient::decode_token_expiration(t) {
            Ok(exp) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let remaining = exp - now;
                if remaining > 0 {
                    token_usable = true;
                    checks.push(check(
                        "token",
                        Status::Ok,
                        format!("valid for {}", human_seconds(remaining as u64)),
                    ));
                } else {
                    let renewable = client_id.is_some() && client_secret.is_some();
                    checks.push(check(
                        "token",
                        if renewable { Status::Ok } else { Status::Fail },
                        if renewable {
                            format!(
                                "expired {} ago; renewed automatically on the next command",
                                human_seconds((-remaining) as u64)
                            )
                        } else {
                            "expired and no client credentials are stored - run 'pcli2 auth login'"
                                .to_string()
                        },
                    ));
                    token_usable = renewable;
                }
            }
            Err(e) => checks.push(check(
                "token",
                Status::Warn,
                format!(
                    "stored but not decodable ({}); a renewal will replace it",
                    e
                ),
            )),
        },
        None => {
            let renewable = client_id.is_some() && client_secret.is_some();
            token_usable = renewable;
            checks.push(check(
                "token",
                if renewable { Status::Ok } else { Status::Fail },
                if renewable {
                    "none stored; obtained automatically on the next command".to_string()
                } else {
                    "none stored - run 'pcli2 auth login'".to_string()
                },
            ));
        }
    }

    // Tenant
    let active_tenant = configuration
        .as_ref()
        .and_then(|c| c.get_active_tenant_uuid());
    match active_tenant {
        Some(uuid) => {
            let cached = crate::tenant_cache::TenantCache::load().ok();
            let name = cached.as_ref().and_then(|cache| {
                cache
                    .tenants
                    .iter()
                    .find(|t| t.tenant_uuid == uuid)
                    .map(|t| format!("{} ({})", t.tenant_short_name, t.tenant_display_name))
            });
            match name {
                Some(name) => checks.push(check("tenant", Status::Ok, format!("{} {}", name, uuid))),
                None => checks.push(check(
                    "tenant",
                    Status::Warn,
                    format!(
                        "{} is active but not in the cached tenant list (stale cache, or the tenant belongs to another environment)",
                        uuid
                    ),
                )),
            }
        }
        None => checks.push(check(
            "tenant",
            Status::Warn,
            "none selected - 'pcli2 tenant use --name <tenant>'",
        )),
    }

    // Caches
    checks.push(cache_check());

    // API connectivity (only meaningful with something to authenticate with)
    let mut connectivity_failed = false;
    if token_usable {
        match crate::physna_v3::PhysnaApiClient::try_default_quiet() {
            Ok(mut api) => {
                let started = Instant::now();
                match crate::tenant_cache::TenantCache::get_all_tenants(&mut api, true).await {
                    Ok(tenants) => checks.push(check(
                        "api",
                        Status::Ok,
                        format!(
                            "{} answered in {} ms; {} tenant(s) visible",
                            api.base_url(),
                            started.elapsed().as_millis(),
                            tenants.len()
                        ),
                    )),
                    Err(e) => {
                        connectivity_failed = true;
                        checks.push(check(
                            "api",
                            Status::Fail,
                            format!("{}: {}", api.base_url(), e),
                        ));
                    }
                }
            }
            Err(e) => {
                connectivity_failed = true;
                checks.push(check("api", Status::Fail, e.to_string()));
            }
        }
    } else {
        checks.push(check(
            "api",
            Status::Warn,
            "not checked: no usable credentials",
        ));
    }

    // Auth server reachability
    if let Some(cfg) = &configuration {
        let auth_url = cfg.get_auth_base_url();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build();
        match client {
            Ok(client) => {
                let started = Instant::now();
                match client.head(&auth_url).send().await {
                    Ok(response) => checks.push(check(
                        "auth server",
                        Status::Ok,
                        format!(
                            "{} reachable (HTTP {} in {} ms)",
                            auth_url,
                            response.status().as_u16(),
                            started.elapsed().as_millis()
                        ),
                    )),
                    Err(e) => {
                        connectivity_failed = true;
                        checks.push(check(
                            "auth server",
                            Status::Fail,
                            format!("{}: {}", auth_url, e),
                        ));
                    }
                }
            }
            Err(e) => checks.push(check("auth server", Status::Warn, e.to_string())),
        }
    }

    // Update state
    checks.push(update_check());

    let report = Report {
        version: env!("CARGO_PKG_VERSION").to_string(),
        checks,
    };
    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("pcli2 doctor (v{})", report.version);
        for c in &report.checks {
            let mark = match c.status {
                Status::Ok => "✓",
                Status::Warn => "!",
                Status::Fail => "✗",
            };
            println!("  {} {:<14} {}", mark, c.name, c.detail);
        }
    }

    let failed = report.checks.iter().any(|c| c.status == Status::Fail);
    if failed {
        Err(CliError::AlreadyReported(if connectivity_failed {
            PcliExitCode::Unavailable
        } else {
            PcliExitCode::ConfigError
        }))
    } else {
        Ok(())
    }
}

fn credentials_check(env_key: &str, has_id: bool, has_secret: bool) -> Check {
    #[cfg(feature = "dev-keyring")]
    let (backend, location) = {
        let path = crate::dev_keyring::DevKeyring::default()
            .path()
            .to_path_buf();
        let mut note = format!("plaintext file {}", path.display());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mode = meta.permissions().mode() & 0o777;
                if mode != 0o600 {
                    note.push_str(&format!(" (mode {:o}; expected 600)", mode));
                }
            } else {
                note.push_str(" (not created yet)");
            }
        }
        ("file", note)
    };
    #[cfg(not(feature = "dev-keyring"))]
    let (backend, location) = ("os-keychain", "system keychain".to_string());

    let stored = match (has_id, has_secret) {
        (true, true) => "client ID and secret stored",
        (true, false) => "client ID stored, secret missing",
        (false, true) => "secret stored, client ID missing",
        (false, false) => "no client credentials stored",
    };
    check(
        "credentials",
        if has_id && has_secret {
            Status::Ok
        } else {
            Status::Warn
        },
        format!(
            "{} backend, {}; environment '{}': {}",
            backend, location, env_key, stored
        ),
    )
}

fn cache_check() -> Check {
    let dir = crate::cache::BaseCache::get_cache_dir();
    let mut entries: Vec<String> = Vec::new();
    let mut newest: Option<u64> = None;
    for sub in [dir.clone(), dir.join("folder_cache")] {
        if let Ok(read) = std::fs::read_dir(&sub) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    let age = meta
                        .modified()
                        .ok()
                        .and_then(|m| SystemTime::now().duration_since(m).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    newest = Some(newest.map_or(age, |n: u64| n.min(age)));
                    entries.push(format!(
                        "{} ({} old)",
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        human_seconds(age)
                    ));
                }
            }
        }
    }
    if entries.is_empty() {
        check(
            "caches",
            Status::Ok,
            format!("{} (empty; filled on first use)", dir.display()),
        )
    } else {
        entries.sort();
        check(
            "caches",
            Status::Ok,
            format!("{}: {}", dir.display(), entries.join(", ")),
        )
    }
}

fn update_check() -> Check {
    #[derive(serde::Deserialize)]
    struct Cache {
        last_checked: u64,
        latest_version: String,
    }
    let path = crate::cache::BaseCache::get_cache_dir().join("update-check.json");
    let current = env!("CARGO_PKG_VERSION");
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Cache>(&s).ok())
    {
        Some(cache) => {
            let age = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs().saturating_sub(cache.last_checked))
                .unwrap_or(0);
            if version_tuple(&cache.latest_version) > version_tuple(current) {
                check(
                    "update",
                    Status::Warn,
                    format!(
                        "v{} is available (you have v{}; checked {} ago)",
                        cache.latest_version,
                        current,
                        human_seconds(age)
                    ),
                )
            } else {
                check(
                    "update",
                    Status::Ok,
                    format!(
                        "up to date (latest known v{}, checked {} ago)",
                        cache.latest_version,
                        human_seconds(age)
                    ),
                )
            }
        }
        None => check(
            "update",
            Status::Ok,
            "no check recorded yet (runs once a day in terminal sessions)",
        ),
    }
}

fn version_tuple(v: &str) -> (u64, u64, u64) {
    let mut parts = v.trim_start_matches('v').split('.').map(|p| {
        p.chars()
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

fn pcli2_binaries_on_path() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let name = if cfg!(windows) { "pcli2.exe" } else { "pcli2" };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                let canonical = std::fs::canonicalize(&candidate).unwrap_or(candidate.clone());
                if !found.iter().any(|p| Path::new(p) == canonical) {
                    found.push(canonical);
                }
            }
        }
    }
    found
}

fn human_seconds(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

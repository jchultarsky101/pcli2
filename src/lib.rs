//! The Physna CLI client library behind the `pcli2` binary.
//!
//! # Layers
//!
//! - `commands`: the clap command tree (flags, help, examples). `cli.rs` in the
//!   binary dispatches a parsed command to `actions`.
//! - `actions`: one function per command; `actions::bulk` runs an operation over
//!   many items (folder download, thumbnail, upload).
//! - `physna_v3`: the API client. Every request goes through
//!   `http_utils::HttpClient` (timeouts, retries) and renews the token on a 401;
//!   `paging` decides when a paginated listing is complete.
//! - `model`: API and output types; `format` and `xlsx_report` turn them into
//!   JSON, CSV, tree and Excel output.
//!
//! # Local state
//!
//! - `configuration`: `config.yml` (environments, the active tenant).
//! - `keyring` / `dev_keyring`: credentials and tokens (a file next to the
//!   configuration by default, the OS keychain with the `os-keyring` feature).
//! - `cache`, `folder_cache`, `tenant_cache`: per-environment caches.
//! - `checkpoint`: resumable folder matches.
//! - `fs_utils`: atomic writes and the lock file every state file uses.
//!
//! # Contracts
//!
//! - `exit_codes`: the documented exit codes; `error` and `error_utils` map
//!   failures to them and report them (text or `--error-format json`).
//! - `ui_url`: links into the Physna web application.

#[cfg(not(any(feature = "dev-keyring", feature = "os-keyring")))]
compile_error!(
    "choose a credential store: the default `dev-keyring` feature, or `--no-default-features --features os-keyring`"
);

pub mod actions;
pub mod auth;
pub mod cache;
pub mod checkpoint;
pub mod commands;
pub mod configuration;
pub mod context;
pub mod dependency_diff;
pub mod dev_keyring;
pub mod error;
pub mod error_utils;
pub mod exit_codes;
pub mod folder_cache;
pub mod folder_hierarchy;
pub mod format;
pub mod format_utils;
pub mod fs_utils;
pub mod http_utils;
pub mod keyring;
pub mod match_groups;
pub mod metadata;
pub mod model;
pub mod paging;
pub mod param_utils;
pub mod path_utils;
pub mod physna_v3;
pub mod stats;
pub mod tenant_cache;
pub mod terminal;
pub mod ui_url;
pub mod update_check;
pub mod xlsx_report;

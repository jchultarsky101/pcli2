//! The Physna CLI client library.
//!
//! This crate provides the core functionality for the Physna CLI client,
//! including API interactions, data models, caching, authentication, and
//! command execution.
//!
//! # Modules
//!
//! - `api`: Legacy API client implementations
//! - `asset_cache`: Caching functionality for assets
//! - `auth`: Authentication mechanisms
//! - `commands`: CLI command parsing and execution
//! - `configuration`: Configuration management
//! - `folder_cache`: Caching functionality for folders
//! - `folder_hierarchy`: Folder hierarchy management and traversal
//! - `format`: Data formatting utilities for various output formats
//! - `model`: Data models for Physna entities (folders, assets, tenants, etc.)
//! - `physna_v3`: Physna V3 API client implementation

pub mod actions;
pub mod auth;
pub mod cache;
pub mod commands;
pub mod context;
pub mod configuration;
pub mod dev_keyring;
pub mod error;
pub mod error_utils;
pub mod exit_codes;
pub mod folder_hierarchy;
pub mod format;
pub mod format_utils;
pub mod http_utils;
pub mod keyring;
pub mod metadata;
pub mod metadata_cache;
pub mod model;
pub mod param_utils;
pub mod physna_v3;
pub mod tenant_cache;

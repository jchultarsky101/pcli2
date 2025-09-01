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

pub mod api;
pub mod asset_cache;
pub mod auth;
pub mod commands;
pub mod configuration;
pub mod dev_keyring;
pub mod exit_codes;
pub mod folder_cache;
pub mod folder_hierarchy;
pub mod format;
pub mod keyring;
pub mod metadata;
pub mod model;
pub mod physna_v3;

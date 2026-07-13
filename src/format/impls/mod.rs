//! Formatting trait implementations for Physna data models.
//!
//! This module contains the implementations of formatting traits (`OutputFormatter`,
//! `CsvRecordProducer`, and `Formattable`) for all data models defined in `crate::model`.
//!
//! The implementations are organized by domain:
//! - `folder`: Folder and FolderList formatting
//! - `tenant`: Tenant and TenantList formatting
//! - `asset`: Asset, AssetList, and thumbnail variants formatting
//! - `metadata`: AssetMetadata formatting
//! - `metadata_field`: tenant metadata-field registry (`tenant metadata list`)
//! - `match_ops`: Search and match response formatting (part search, geometric search, etc.)
//! - `dependencies`: Asset dependency and assembly tree formatting
//! - `state`: Asset state counts formatting

mod asset;
mod dependencies;
mod dependency_diff;
mod folder;
mod health;
mod match_ops;
mod metadata;
mod metadata_field;
mod state;
mod tenant;

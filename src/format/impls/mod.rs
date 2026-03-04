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
//! - `match_ops`: Search and match response formatting (part search, geometric search, etc.)
//! - `dependencies`: Asset dependency and assembly tree formatting
//! - `state`: Asset state counts formatting

mod asset;
mod dependencies;
mod folder;
mod match_ops;
mod metadata;
mod state;
mod tenant;

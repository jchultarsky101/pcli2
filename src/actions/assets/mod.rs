//! Asset management actions for the Physna CLI.
//!
//! This module provides functionality for managing assets including:
//! - Listing and printing asset information
//! - Creating and uploading assets
//! - Downloading assets and thumbnails
//! - Managing asset metadata
//! - Deleting assets
//! - Finding matching assets (geometric, visual, part, text)
//! - Reprocessing assets

pub mod create;
pub mod delete;
pub mod download;
pub mod list;
pub mod match_ops;
pub mod metadata;
pub mod print;
pub mod reprocess;

// Re-export all public functions for backward compatibility
pub use create::{create_asset, create_asset_batch, create_asset_metadata_batch, update_asset_metadata};
pub use delete::{delete_asset, delete_asset_metadata};
pub use download::{download_asset, download_asset_thumbnail, download_folder};
pub use list::list_assets;
pub use match_ops::{
    geometric_match_asset, geometric_match_folder, part_match_asset, part_match_folder,
    text_match, visual_match_asset, visual_match_folder,
};
pub use metadata::metadata_inference;
pub use print::{
    print_asset, print_asset_dependencies, print_asset_metadata, print_folder_dependencies,
};
pub use reprocess::reprocess_asset;

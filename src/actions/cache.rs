//! Cache management actions.
//!
//! This module provides functionality for clearing various caches.

use crate::error::CliError;
use clap::ArgMatches;
use tracing::trace;

/// Clear all cached data or specific cache types.
///
/// This function handles the "cache clear" command, removing cached data
/// from the file system.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the cache was cleared successfully
/// * `Err(CliError)` - If an error occurred during clearing
pub async fn clear_cache(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Clearing cache...");

    let clear_folder = sub_matches.get_flag("folder");
    let clear_metadata = sub_matches.get_flag("metadata");
    let clear_tenant = sub_matches.get_flag("tenant");
    let skip_confirm = sub_matches.get_flag("yes");

    // Determine what to clear
    let clear_all = !clear_folder && !clear_metadata && !clear_tenant;

    // Show what will be cleared
    if clear_all {
        eprintln!("This will clear:");
        eprintln!("  • Folder hierarchy cache");
        eprintln!("  • Metadata field cache");
        eprintln!("  • Tenant list cache");
        eprintln!();
    } else {
        eprintln!("This will clear:");
        if clear_folder {
            eprintln!("  • Folder hierarchy cache");
        }
        if clear_metadata {
            eprintln!("  • Metadata field cache");
        }
        if clear_tenant {
            eprintln!("  • Tenant list cache");
        }
        eprintln!();
    }

    // Confirm unless --yes flag is provided
    if !skip_confirm {
        eprint!("Continue? [y/N] ");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

        let response = input.trim().to_lowercase();
        if response != "y" && response != "yes" {
            eprintln!("Cache clear cancelled.");
            return Ok(());
        }
    }

    // Track failures so the command exits non-zero when a purge fails,
    // instead of printing a success message that scripts would trust.
    let mut failures: Vec<String> = Vec::new();

    // Clear folder cache
    if clear_all || clear_folder {
        match crate::folder_cache::FolderCache::purge_all() {
            Ok(()) => eprintln!("✓ Folder cache cleared"),
            Err(e) => {
                eprintln!("✗ Failed to clear folder cache: {}", e);
                failures.push(format!("folder cache: {}", e));
            }
        }
    }

    // Clear metadata cache
    if clear_all || clear_metadata {
        // The metadata cache lives in two historical locations: the
        // metadata_cache/ directory and the metadata_cache.json file that
        // MetadataCache actually writes. Purge both.
        let base_cache_dir = crate::cache::BaseCache::get_cache_dir();
        let mut metadata_ok = true;

        let metadata_cache_dir = base_cache_dir.join("metadata_cache");
        if metadata_cache_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&metadata_cache_dir) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("✗ Failed to clear metadata cache directory: {}", e);
                    failures.push(format!("metadata cache: {}", e));
                    metadata_ok = false;
                }
            }
        }

        let metadata_cache_file = base_cache_dir.join("metadata_cache.json");
        if metadata_cache_file.exists() {
            if let Err(e) = std::fs::remove_file(&metadata_cache_file) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("✗ Failed to clear metadata cache file: {}", e);
                    failures.push(format!("metadata cache file: {}", e));
                    metadata_ok = false;
                }
            }
        }

        if metadata_ok {
            eprintln!("✓ Metadata cache cleared");
        }
    }

    // Clear tenant cache
    if clear_all || clear_tenant {
        match crate::tenant_cache::TenantCache::invalidate_all() {
            Ok(()) => eprintln!("✓ Tenant cache cleared"),
            Err(e) => {
                eprintln!("✗ Failed to clear tenant cache: {}", e);
                failures.push(format!("tenant cache: {}", e));
            }
        }
    }

    eprintln!();
    if failures.is_empty() {
        eprintln!(
            "Cache cleared successfully. Fresh data will be fetched from the API on next use."
        );
        Ok(())
    } else {
        Err(CliError::ActionError(
            crate::actions::CliActionError::BusinessLogicError(format!(
                "Cache clear failed for: {}",
                failures.join("; ")
            )),
        ))
    }
}

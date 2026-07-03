//! Create asset functionality.
//!
//! This module provides functionality for creating and uploading assets,
//! including batch operations and metadata management.

use crate::{
    actions::assets::metadata_batch_csv::{parse_batch_csv, BatchAssetRef, BatchCsvFormat},
    actions::folders::resolve_folder_uuid_by_path,
    actions::CliActionError,
    commands::params::{
        PARAMETER_CONTINUE_ON_ERROR, PARAMETER_DELETE_IF_EMPTY, PARAMETER_FILE, PARAMETER_FILES,
        PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_OVERRIDE, PARAMETER_PATH,
        PARAMETER_RESTORE_METADATA, PARAMETER_UUID,
    },
    configuration::Configuration,
    error::CliError,
    error_utils,
    folder_hierarchy::FolderHierarchy,
    format::OutputFormatter,
    metadata::convert_single_metadata_to_json_value,
    model::{Asset, AssetList},
    param_utils::get_format_parameter_value,
    param_utils::get_tenant,
    physna_v3::{ApiError, PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, trace, warn};

/// Cache for asset metadata during batch processing to avoid repeated API calls
struct AssetMetadataCache {
    cache: HashMap<String, Asset>,
}

impl AssetMetadataCache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<&Asset> {
        self.cache.get(key)
    }

    fn insert(&mut self, key: String, asset: Asset) {
        self.cache.insert(key, asset);
    }
}

/// Convert a string value to JSON type
/// For batch operations, we keep all values as strings to avoid type conflicts
/// The API will validate and return clear errors if there's a type mismatch
///
/// # Arguments
/// * `value` - The string value to convert
/// * `_existing_type` - Optional existing field type (currently ignored)
///
/// # Returns
/// JSON value (always String for safety)
fn convert_string_to_json_type(value: &str, _existing_type: Option<&str>) -> Value {
    // Handle empty string - represents "delete" operation
    if value.is_empty() {
        return Value::Null;
    }

    // Keep as string to avoid type conflicts
    // The API handles type validation and provides clear error messages
    Value::String(value.to_string())
}

/// Create a single asset by uploading a file.
///
/// This function handles the "asset create" command, uploading a file
/// to the Physna API and creating a new asset.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was created successfully
/// * `Err(CliError)` - If an error occurred during the creation
pub async fn create_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing file upload...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path - handle specially if needed, but for asset creation we need the actual folder
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Check if the folder exists and set its path
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder = folder;
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned());
    } else {
        // When using --folder-uuid, try to build the folder hierarchy to get the full path
        // This is optional - if it fails, we'll just use the folder name without the full path
        match FolderHierarchy::build_from_api(&mut api, &tenant.uuid).await {
            Ok(hierarchy) => {
                if let Some(path) = hierarchy.get_path_for_folder(&folder_uuid) {
                    folder.set_path(path);
                }
            }
            Err(_) => {
                // If we can't build the hierarchy, just use the folder name as the path
                // This allows the create operation to proceed even if hierarchy fetch fails
                folder.set_path(folder.name());
            }
        }
    }

    let file_path = sub_matches
        .get_one::<PathBuf>(PARAMETER_FILE)
        .ok_or(CliError::MissingRequiredArgument("file".to_string()))?;

    // Extract filename from path for use in asset path construction
    let file_name = file_path
        .file_name()
        .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file path".to_string()))?
        .to_str()
        .ok_or_else(|| CliError::MissingRequiredArgument("Invalid file name".to_string()))?
        .to_string();

    // Construct the full asset path by combining folder path with filename
    let folder_path = folder.path();
    let asset_path = if folder_path.is_empty() || folder_path == "/" {
        file_name.clone()
    } else {
        format!("{}/{}", folder_path, file_name)
    };

    debug!("Creating asset with path: {}", asset_path);

    // Report and stop without uploading anything when --dry-run is given
    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        println!(
            "Dry run: would upload '{}' as asset '{}'",
            file_path.display(),
            asset_path
        );
        return Ok(());
    }

    let override_flag = sub_matches.get_flag(PARAMETER_OVERRIDE);
    let restore_metadata = sub_matches.get_flag(PARAMETER_RESTORE_METADATA);

    let asset = if override_flag {
        let existing = api.get_asset_by_path(&tenant.uuid, &asset_path).await.ok();

        if let Some(existing) = existing {
            debug!(
                "Asset already exists at path '{}', --override specified, deleting and re-uploading",
                asset_path
            );

            let saved_metadata = if restore_metadata {
                let metadata: HashMap<String, serde_json::Value> = existing
                    .metadata()
                    .map(|m| {
                        m.keys()
                            .filter_map(|k| {
                                m.get(k)
                                    .map(|v| (k.clone(), serde_json::Value::String(v.clone())))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if metadata.is_empty() {
                    debug!("No metadata found on existing asset to restore");
                    None
                } else {
                    debug!("Saved {} metadata fields for restoration", metadata.len());
                    Some(metadata)
                }
            } else {
                None
            };

            api.delete_asset(&tenant.uuid.to_string(), &existing.uuid().to_string())
                .await?;

            const MAX_RETRIES: u32 = 5;
            const INITIAL_DELAY_MS: u64 = 500;

            let mut created_asset = None;
            for attempt in 0..=MAX_RETRIES {
                match api
                    .create_asset_with_metadata(
                        &tenant.uuid,
                        file_path,
                        &asset_path,
                        &folder_uuid,
                        saved_metadata.as_ref(),
                    )
                    .await
                {
                    Ok(asset) => {
                        if attempt > 0 {
                            debug!("Asset creation succeeded on retry attempt {}", attempt);
                        }
                        created_asset = Some(asset);
                        break;
                    }
                    Err(ApiError::ConflictError(_)) if attempt < MAX_RETRIES => {
                        let delay = INITIAL_DELAY_MS * 2u64.pow(attempt);
                        warn!(
                            "Conflict on attempt {} of {} — the previous asset may still be deleting. Retrying in {}ms...",
                            attempt + 1,
                            MAX_RETRIES + 1,
                            delay
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            match created_asset {
                Some(asset) => asset,
                None => {
                    return Err(ApiError::ConflictError(
                        "Asset conflict persisted after delete during --override. The server may still be processing the deletion.".to_string(),
                    )
                    .into())
                }
            }
        } else {
            api.create_asset(&tenant.uuid, file_path, &asset_path, &folder_uuid)
                .await?
        }
    } else {
        api.create_asset(&tenant.uuid, file_path, &asset_path, &folder_uuid)
            .await?
    };

    println!("{}", asset.format(format)?);

    Ok(())
}

/// Create multiple assets in batch from a glob pattern.
///
/// This function handles the "asset create-batch" command, uploading multiple files
/// matching a glob pattern to the Physna API.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the assets were created successfully
/// * `Err(CliError)` - If an error occurred during the creation
pub async fn create_asset_batch(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"create asset batch\" command...");

    let glob_pattern = sub_matches
        .get_one::<String>(PARAMETER_FILES)
        .ok_or(CliError::MissingRequiredArgument("files".to_string()))?
        .clone();
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").unwrap_or(&5);
    let concurrent = *concurrent_param;
    let show_progress = sub_matches.get_flag("progress");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let format = get_format_parameter_value(sub_matches).await;
    let folder_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        let normalized_path = crate::model::normalize_path(path);
        if normalized_path == "/" {
            // Root path - handle specially if needed, but for asset creation we need the actual folder
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        } else {
            resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
        }
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Check if the folder exists
    let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;
    let mut folder = folder;
    if let Some(path) = folder_path_param {
        folder.set_path(path.to_owned())
    } else {
        // When using --folder-uuid, we need to build the folder hierarchy to get the full path
        let hierarchy = FolderHierarchy::build_from_api(&mut api, &tenant.uuid).await?;
        if let Some(path) = hierarchy.get_path_for_folder(&folder_uuid) {
            folder.set_path(path);
        }
    }

    // Report and stop without uploading anything when --dry-run is given
    if sub_matches.get_flag(crate::commands::params::PARAMETER_DRY_RUN) {
        let mut paths = crate::physna_v3::expand_upload_paths(&glob_pattern)
            .map_err(CliError::PhysnaExtendedApiError)?;
        paths.sort();
        if paths.is_empty() {
            println!("Dry run: no files match '{}'", glob_pattern);
            return Ok(());
        }
        println!(
            "Dry run: would upload {} file(s) to folder '{}':",
            paths.len(),
            folder.path()
        );
        for path in &paths {
            println!("  {}", path.display());
        }
        return Ok(());
    }

    let assets = api
        .create_assets_batch(
            &tenant.uuid,
            &glob_pattern,
            Some(folder.path().as_str()),
            Some(&folder_uuid),
            concurrent,
            show_progress,
        )
        .await?;
    println!("{}", AssetList::from(assets).format(format)?);

    Ok(())
}

/// Create metadata for multiple assets from a CSV file.
///
/// This function handles the "asset metadata create-batch" command, which creates or updates
/// metadata for multiple assets from a CSV file.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the metadata was created successfully
/// * `Err(CliError)` - If an error occurred during the creation
pub async fn create_asset_metadata_batch(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"create asset metadata batch\" command...");

    let csv_file_path = sub_matches
        .get_one::<std::path::PathBuf>("csv-file")
        .ok_or(CliError::MissingRequiredArgument("csv-file".to_string()))?;

    let show_progress = sub_matches.get_flag("progress");
    let continue_on_error = sub_matches.get_flag(PARAMETER_CONTINUE_ON_ERROR);
    let delete_if_empty = sub_matches.get_flag(PARAMETER_DELETE_IF_EMPTY);
    let requested_format = sub_matches
        .get_one::<String>("csv-format")
        .map(|s| BatchCsvFormat::from_arg(s))
        .unwrap_or(BatchCsvFormat::Auto);

    // Parse and validate the whole CSV file (classic vertical or UI
    // horizontal layout) before authenticating or making any API calls, so a
    // malformed file fails fast instead of half-applying.
    let file = std::fs::File::open(csv_file_path)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
    let parsed =
        parse_batch_csv(file, requested_format, delete_if_empty).map_err(CliError::ActionError)?;

    debug!("Parsed batch CSV as {:?} format", parsed.format);
    for warning in &parsed.warnings {
        error_utils::report_warning(warning);
    }
    let entries = parsed.entries;

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Pre-flight token expiration check
    const TIME_PER_ASSET_SECONDS: u64 = 5;
    const SAFETY_MARGIN_SECONDS: u64 = 300;

    let time_remaining = api.get_token_time_remaining().unwrap_or(0);
    let estimated_time_needed = (entries.len() as u64) * TIME_PER_ASSET_SECONDS;

    if time_remaining > 0 && (time_remaining as u64) < estimated_time_needed + SAFETY_MARGIN_SECONDS
    {
        let time_remaining_min = time_remaining / 60;
        error_utils::report_warning(&format!(
            "Token expires in approximately {} minutes, but batch operation may take {} minutes. Token will be refreshed automatically if needed during processing.",
            time_remaining_min,
            (estimated_time_needed / 60).max(1)
        ));
    }

    // Create in-memory cache for asset metadata to avoid repeated API calls
    let mut asset_cache = AssetMetadataCache::new();

    // Process each asset
    let total_assets = entries.len();
    let mut success_count = 0;
    let mut failure_count = 0;
    let mut skipped_missing = 0;
    let mut auth_failure_occurred = false;

    // Progress bar drawn on stderr. Unlike a hand-rolled "\r"-prefixed line, it
    // redraws cleanly instead of leaving stale characters behind when the next
    // asset path is shorter than the previous one.
    let progress_bar = if show_progress {
        let pb = ProgressBar::new(total_assets as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec} {wide_msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Emit a diagnostic without corrupting the progress bar: suspend() clears
    // the bar, runs the print, then redraws the bar on a fresh line.
    let report = |msg: String, steps: &[&str]| match progress_bar.as_ref() {
        Some(pb) => pb.suspend(|| error_utils::report_error_with_remediation(&msg, steps)),
        None => error_utils::report_error_with_remediation(&msg, steps),
    };

    // Concise single-line warning (used for skipped rows in --continue-on-error
    // mode, where repeating the full remediation block per asset is noise).
    let warn = |msg: String| match progress_bar.as_ref() {
        Some(pb) => pb.suspend(|| error_utils::report_warning(&msg)),
        None => error_utils::report_warning(&msg),
    };

    for entry in &entries {
        // Display key for progress, caching, and error messages: the asset
        // path, or the UUID when the row identified the asset by UUID.
        let asset_display = entry.asset.display();
        let raw_metadata = &entry.metadata;

        if let Some(pb) = progress_bar.as_ref() {
            pb.set_message(asset_display.clone());
            pb.inc(1);
        }

        // Proactively refresh token if expiring soon
        const TOKEN_REFRESH_THRESHOLD_SECONDS: u64 = 120;
        if let Err(e) = api
            .refresh_token_if_expiring_soon(TOKEN_REFRESH_THRESHOLD_SECONDS)
            .await
        {
            debug!("Proactive token refresh failed: {}", e);
        }

        // Get the asset to read existing metadata types (use cache if available)
        let asset = match asset_cache.get(&asset_display) {
            Some(cached) => cached.clone(),
            None => {
                let lookup_result = match &entry.asset {
                    BatchAssetRef::Uuid(uuid) => api.get_asset_by_uuid(&tenant.uuid, uuid).await,
                    BatchAssetRef::Path(path) => api.get_asset_by_path(&tenant.uuid, path).await,
                };
                match lookup_result {
                    Ok(asset) => {
                        // Cache the asset for potential reuse
                        asset_cache.insert(asset_display.clone(), asset.clone());
                        asset
                    }
                    Err(e) => {
                        if e.is_authentication_failure() {
                            auth_failure_occurred = true;
                            report(
                                format!("Authentication failed while looking up asset '{}': {}", asset_display, e),
                                &[
                                    "Your access token may have expired",
                                    "Try running 'pcli2 auth expiration' to check token status",
                                    "Re-authenticate with 'pcli2 auth login' and retry the batch operation",
                                ],
                            );
                            failure_count += 1;
                            break;
                        }

                        failure_count += 1;

                        // In continue-on-error mode a missing asset is expected and
                        // common, so emit a single concise line here and defer the
                        // detailed guidance to one summary at the end of the run.
                        if continue_on_error {
                            skipped_missing += 1;
                            warn(format!("Skipped '{}' — asset not found", asset_display));
                            continue;
                        }

                        let remediation_steps: &[&str] = match &entry.asset {
                            BatchAssetRef::Uuid(_) => &[
                                "Verify the UUID in the 'id' column matches an existing asset in Physna",
                                "Check that the asset hasn't been deleted or re-uploaded (re-uploading changes the UUID)",
                                "Verify you're using the correct tenant for this asset",
                                "Or re-run with --continue-on-error to skip unresolvable assets",
                            ],
                            BatchAssetRef::Path(_) => &[
                                "Verify the asset path in your CSV file matches the actual asset path in Physna",
                                "Check that the asset hasn't been deleted from the system",
                                "Verify you're using the correct tenant for this asset",
                                "Check for path format mismatches (e.g., leading slash differences)",
                                "Verify the asset exists using 'pcli2 asset list --folder-path /' or similar command",
                                "Or re-run with --continue-on-error to skip unresolved paths",
                            ],
                        };
                        report(
                            format!("Asset not found: '{}'", asset_display),
                            remediation_steps,
                        );

                        if let Some(pb) = progress_bar.as_ref() {
                            pb.finish_and_clear();
                        }
                        eprintln!(
                            "Batch operation stopped: {} successful, {} failed (use --continue-on-error to skip unresolvable asset paths)",
                            success_count, failure_count
                        );
                        return Err(CliError::PhysnaExtendedApiError(e));
                    }
                }
            }
        };

        // Split into fields to delete (empty value, only present when the file
        // was parsed with --delete-if-empty) and fields to update (non-empty value)
        let mut fields_to_delete: Vec<String> = Vec::new();
        let mut typed_metadata: HashMap<String, serde_json::Value> = HashMap::new();

        for (field_name, raw_value) in raw_metadata {
            let json_value = convert_string_to_json_type(raw_value, None);
            if json_value.is_null() {
                fields_to_delete.push(field_name.clone());
            } else {
                typed_metadata.insert(field_name.clone(), json_value);
            }
        }

        // Delete fields with empty values
        if !fields_to_delete.is_empty() {
            let keys: Vec<&str> = fields_to_delete.iter().map(|s| s.as_str()).collect();
            if let Err(e) = api
                .delete_asset_metadata(&tenant.uuid.to_string(), &asset.uuid().to_string(), keys)
                .await
            {
                if e.is_authentication_failure() {
                    auth_failure_occurred = true;
                    report(
                        format!(
                            "Authentication failed while deleting metadata for asset '{}': {}",
                            asset_display, e
                        ),
                        &[
                            "Your access token may have expired",
                            "Try running 'pcli2 auth expiration' to check token status",
                            "Re-authenticate with 'pcli2 auth login' and retry the batch operation",
                        ],
                    );
                    failure_count += 1;
                    break;
                }
                report(
                    format!(
                        "Failed to delete metadata fields for asset '{}': {}",
                        asset_display, e
                    ),
                    &[
                        "Verify the metadata field names exist on this asset",
                        "Check that you have sufficient permissions to modify this asset",
                    ],
                );
                failure_count += 1;
                if let Some(pb) = progress_bar.as_ref() {
                    pb.finish_and_clear();
                }
                eprintln!(
                    "Batch operation stopped: {} successful, {} failed",
                    success_count, failure_count
                );
                return Err(CliError::PhysnaExtendedApiError(e));
            }
        }

        // Update fields with non-empty values
        if !typed_metadata.is_empty() {
            if let Err(e) = api
                .update_asset_metadata_with_registration(
                    &tenant.uuid,
                    &asset.uuid(),
                    &typed_metadata,
                )
                .await
            {
                if e.is_authentication_failure() {
                    auth_failure_occurred = true;
                    report(
                        format!(
                            "Authentication failed while updating metadata for asset '{}': {}",
                            asset_display, e
                        ),
                        &[
                            "Your access token may have expired",
                            "Try running 'pcli2 auth expiration' to check token status",
                            "Re-authenticate with 'pcli2 auth login' and retry the batch operation",
                        ],
                    );
                    failure_count += 1;
                    break;
                }

                let error_str = format!("{}", e);
                if error_str.contains("must be a") || error_str.contains("Metadata type mismatch") {
                    report(
                        format!("Type conflict for asset '{}': {}", asset_display, e),
                        &[
                            "The metadata field already exists with a different type",
                            "Delete the existing field first if you need to change its type",
                            "Or provide a value that matches the existing field type",
                        ],
                    );
                } else {
                    report(
                        format!(
                            "Failed to update metadata for asset '{}': {}",
                            asset_display, e
                        ),
                        &[
                            "Verify metadata field names and values are valid",
                            "Check that you have sufficient permissions to modify this asset",
                            "Verify your network connectivity",
                            "Confirm the asset hasn't been deleted or modified recently",
                        ],
                    );
                }
                failure_count += 1;
                if let Some(pb) = progress_bar.as_ref() {
                    pb.finish_and_clear();
                }
                eprintln!(
                    "Batch operation stopped: {} successful, {} failed",
                    success_count, failure_count
                );
                return Err(CliError::PhysnaExtendedApiError(e));
            }
        }

        success_count += 1;
    }

    if let Some(pb) = progress_bar.as_ref() {
        pb.finish_and_clear();
    }

    if show_progress || failure_count > 0 {
        eprintln!(
            "Batch operation completed: {} successful, {} failed",
            success_count, failure_count
        );
    }

    // Detailed guidance for skipped rows is shown once here, rather than after
    // every skipped asset, to keep the per-asset output concise.
    if skipped_missing > 0 {
        eprintln!(
            "\n⚠️  {} asset(s) were skipped because they could not be resolved in this tenant.",
            skipped_missing
        );
        eprintln!("🔧 To resolve, verify that:");
        eprintln!("  1. The asset paths (and UUIDs, if present) in your CSV match actual assets in Physna");
        eprintln!("  2. You are targeting the correct tenant");
        eprintln!("  3. The assets have not been deleted, and paths match (e.g., leading slash)");
        eprintln!("  List existing paths with 'pcli2 asset list --folder-path /' to compare.");
    }

    if auth_failure_occurred {
        return Err(CliError::SecurityError(
            "Batch operation stopped due to authentication failure. Please re-authenticate and retry.".to_string(),
        ));
    }

    Ok(())
}

/// Update an asset's metadata with the specified key-value pair.
///
/// This function handles the "asset metadata create" command, which adds or updates
/// metadata for a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the metadata was updated successfully
/// * `Err(CliError)` - If an error occurred during the update
pub async fn update_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Execute \"asset metadata create\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Get metadata parameters from command line
    let metadata_name = sub_matches
        .get_one::<String>("name")
        .ok_or(CliError::MissingRequiredArgument("name".to_string()))?;
    let metadata_value = sub_matches
        .get_one::<String>("value")
        .ok_or(CliError::MissingRequiredArgument("value".to_string()))?;
    let metadata_type = sub_matches
        .get_one::<String>("type")
        .map(|s| s.as_str())
        .unwrap_or("text");

    // Convert the single metadata entry to JSON value using shared function
    let json_value =
        convert_single_metadata_to_json_value(metadata_name, metadata_value, metadata_type);

    // Create a HashMap with the single metadata entry
    // This hashmap represents the desired metadata fields to update
    let mut metadata: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    metadata.insert(metadata_name.clone(), json_value);

    // Resolve asset ID from either UUID parameter or path
    let asset = if let Some(uuid) = asset_uuid_param {
        api.get_asset_by_uuid(&tenant.uuid, uuid).await?
    } else if let Some(asset_path) = asset_path_param {
        // Get asset by path
        api.get_asset_by_path(&tenant.uuid, asset_path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either asset UUID or path must be provided".to_string(),
        ));
    };

    // Update the asset's metadata with automatic registration of new keys
    api.update_asset_metadata_with_registration(&tenant.uuid, &asset.uuid(), &metadata)
        .await?;

    // No output on success (per requirements)

    Ok(())
}

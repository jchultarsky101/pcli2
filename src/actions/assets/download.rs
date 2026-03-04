//! Download asset functionality.
//!
//! This module provides functionality for downloading assets and thumbnails,
//! including folder downloads as ZIP archives.

use crate::actions::CliActionError;
use crate::{
    actions::folders::resolve_folder_uuid_by_path,
    commands::params::{PARAMETER_FILE, PARAMETER_FOLDER_PATH, PARAMETER_FOLDER_UUID, PARAMETER_PATH, PARAMETER_UUID},
    configuration::Configuration,
    error::CliError,
    error_utils,
    param_utils::get_tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tracing::trace;
use zip::write::FileOptions;
use zip::ZipWriter;

/// Download an asset by UUID or path to a local file.
///
/// This function handles the "asset download" command, retrieving a specific asset
/// identified by either its UUID or path from the Physna API and saving it to a local file.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset was downloaded successfully
/// * `Err(CliError)` - If an error occurred during download
pub async fn download_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset download\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(PARAMETER_FILE)
    {
        output_path.clone()
    } else {
        // Use the asset name as the default output file name
        let asset_name = asset.name();

        let mut path = std::path::PathBuf::new();
        path.push(asset_name);
        path
    };

    // Download the asset file with retry logic
    let file_content = download_asset_with_retry(
        ctx.api(),
        &tenant_uuid.to_string(),
        &asset.uuid().to_string(),
    )
    .await?;

    // If the asset is an assembly, the downloaded file is a ZIP file
    // Add .zip extension to avoid conflict with extracted assembly file
    let zip_file_path = if asset.is_assembly() {
        let mut zip_path = output_file_path.clone();
        // Add .zip extension to the existing filename (e.g., sample17.asm -> sample17.asm.zip)
        let zip_extension = if let Some(ext) = output_file_path.extension() {
            format!("{}.zip", ext.to_string_lossy())
        } else {
            "zip".to_string()
        };
        zip_path.set_extension(zip_extension);
        zip_path
    } else {
        output_file_path.clone()
    };

    // Write the file content to the output file
    std::fs::write(&zip_file_path, file_content).map_err(CliActionError::IoError)?;

    // If the asset is an assembly, extract the ZIP file and cleanup
    if asset.is_assembly() {
        // DEBUG: Log the ZIP file path
        tracing::debug!("Downloaded ZIP file to: {:?}", zip_file_path);
        extract_zip_and_cleanup(&zip_file_path)?;
    }

    Ok(())
}

/// Download a thumbnail for an asset by UUID or path.
///
/// This function handles the "asset thumbnail" command, retrieving a thumbnail
/// for a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the thumbnail was downloaded successfully
/// * `Err(CliError)` - If an error occurred during download
pub async fn download_asset_thumbnail(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset thumbnail\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(PARAMETER_FILE)
    {
        // Validate the output file path
        if output_path.as_os_str().is_empty() {
            return Err(CliError::MissingRequiredArgument(
                "Output file path cannot be empty".to_string(),
            ));
        }

        // Check if the parent directory exists
        if let Some(parent) = output_path.parent() {
            if !parent.exists() {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Parent directory does not exist: {}",
                    parent.display()
                )));
            }
        }

        // Check if the file extension is .png (recommended for thumbnails)
        if let Some(ext) = output_path.extension() {
            if ext.to_string_lossy().to_lowercase() != "png" {
                // Log a warning but allow the operation to continue
                use tracing::warn;
                warn!("Thumbnail file extension is not PNG. Recommended extension is .png");
            }
        }

        output_path.clone()
    } else {
        // Use the asset name as the default output file name with .png extension
        let asset_name = asset.name();

        // Get the stem of the asset name (without extension) and add .png
        let path_stem = std::path::Path::new(&asset_name)
            .file_stem()
            .unwrap_or(std::ffi::OsStr::new(&asset_name))
            .to_string_lossy()
            .to_string();

        let mut path = std::path::PathBuf::new();
        path.push(format!("{}.png", path_stem));
        path
    };

    // Download the asset thumbnail
    let thumbnail_content = ctx
        .api()
        .download_asset_thumbnail(&tenant_uuid.to_string(), &asset.uuid().to_string())
        .await
        .map_err(CliActionError::ApiError)?;

    // Write the thumbnail content to the output file
    std::fs::write(&output_file_path, thumbnail_content).map_err(CliActionError::IoError)?;

    Ok(())
}

/// Download all assets in a folder as a ZIP archive.
///
/// This function handles the "asset download-folder" command, retrieving all assets
/// in a specified folder from the Physna API and packaging them into a ZIP file.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the folder was downloaded successfully
/// * `Err(CliError)` - If an error occurred during download
pub async fn download_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset download-folder\" command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder UUID or path from command line
    let folder_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_FOLDER_UUID);
    let folder_path_param = sub_matches.get_one::<String>(PARAMETER_FOLDER_PATH);

    // Resolve folder UUID from either UUID parameter or path
    let folder_uuid = if let Some(uuid) = folder_uuid_param {
        *uuid
    } else if let Some(path) = folder_path_param {
        // Resolve folder UUID by path
        resolve_folder_uuid_by_path(&mut api, &tenant, path).await?
    } else {
        // This shouldn't happen due to our earlier check, but just in case
        return Err(CliError::MissingRequiredArgument(
            "Either folder UUID or path must be provided".to_string(),
        ));
    };

    // Get the output file path
    let output_file_path = if let Some(output_path) =
        sub_matches.get_one::<PathBuf>(PARAMETER_FILE)
    {
        output_path.clone()
    } else {
        // Use the folder name as the default output file name
        // Determine the folder name from the provided path or get it from the folder details
        let folder_name = if let Some(path) = folder_path_param {
            // If the folder was specified by path, extract the folder name from the path
            let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if path_segments.is_empty() {
                "untitled".to_string()
            } else {
                path_segments.last().unwrap().to_string()
            }
        } else {
            // If the folder was specified by UUID, get the folder details to determine the name
            let folder = api.get_folder(&tenant.uuid, &folder_uuid).await?;

            let folder_path = folder.path();
            let path_segments: Vec<&str> =
                folder_path.split('/').filter(|s| !s.is_empty()).collect();
            if path_segments.is_empty() {
                "untitled".to_string()
            } else {
                path_segments.last().unwrap().to_string()
            }
        };

        let mut path = std::path::PathBuf::new();
        path.push(format!("{}.zip", folder_name));
        path
    };

    // Get all assets in the folder
    let assets_response = api
        .list_assets_by_parent_folder_uuid(&tenant.uuid, Some(&folder_uuid))
        .await?;
    let assets: Vec<_> = assets_response.get_all_assets().to_vec();

    if assets.is_empty() {
        error_utils::report_error_with_remediation(
            &format!("No assets found in folder with UUID: {}", folder_uuid),
            &[
                "Verify the folder UUID or path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the folder",
            ],
        );
        return Ok(());
    }

    // Check if progress should be displayed
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);

    // Create a temporary directory to store downloaded files
    let temp_dir = std::env::temp_dir().join(format!("pcli2_temp_{}", folder_uuid));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;

    // Create progress bar if requested
    let progress_bar = if show_progress {
        let pb = ProgressBar::new(assets.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Downloading assets")
            .unwrap()
            .progress_chars("#>-"));
        Some(pb)
    } else {
        None
    };

    // Download each asset to the temporary directory
    for asset in &assets {
        let file_content = api
            .download_asset(
                &tenant.uuid.to_string(),
                &asset.uuid().to_string(),
                Some(&asset.name()),
            )
            .await?;

        let asset_file_path = temp_dir.join(asset.name());
        let mut file = File::create(&asset_file_path).map_err(CliActionError::IoError)?;
        file.write_all(&file_content)
            .map_err(CliActionError::IoError)?;

        // Update progress bar if present
        if let Some(ref pb) = progress_bar {
            pb.inc(1);
        }
    }

    // Finish progress bar if present
    if let Some(pb) = progress_bar {
        pb.finish_with_message("Assets downloaded, creating ZIP...");
    }

    // Create ZIP file with all downloaded assets
    let zip_file = File::create(&output_file_path).map_err(CliActionError::IoError)?;
    let mut zip_writer = ZipWriter::new(zip_file);

    // Walk through the temp directory and add files to the ZIP
    for entry in std::fs::read_dir(&temp_dir)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?
    {
        let entry = entry.map_err(CliActionError::IoError)?;
        let path = entry.path();

        if path.is_file() {
            let file_name = path
                .file_name()
                .ok_or_else(|| {
                    CliError::ActionError(CliActionError::IoError(std::io::Error::other(
                        "Could not get file name",
                    )))
                })?
                .to_str()
                .ok_or_else(|| {
                    CliError::ActionError(CliActionError::IoError(std::io::Error::other(
                        "Invalid file name",
                    )))
                })?;

            let options: FileOptions<()> = FileOptions::default();
            zip_writer
                .start_file(file_name, options)
                .map_err(|e| CliError::ActionError(CliActionError::ZipError(e)))?;
            let file_content = std::fs::read(&path)
                .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
            zip_writer
                .write_all(&file_content)
                .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| CliError::ActionError(CliActionError::ZipError(e)))?;

    // Clean up temporary directory
    std::fs::remove_dir_all(&temp_dir)
        .map_err(|e| CliError::ActionError(CliActionError::IoError(e)))?;

    Ok(())
}

/// Extract a ZIP file and clean up the archive.
///
/// # Arguments
///
/// * `zip_path` - Path to the ZIP file to extract
///
/// # Returns
///
/// * `Ok(())` - If the extraction was successful
/// * `Err(CliError)` - If an error occurred during extraction
#[allow(clippy::result_large_err)]
fn extract_zip_and_cleanup(zip_path: &std::path::PathBuf) -> Result<(), CliError> {
    use std::io::Cursor;

    // Read the ZIP file content
    let zip_content = std::fs::read(zip_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    // Create a cursor from the content
    let cursor = Cursor::new(zip_content);

    // Create a ZipArchive from the cursor
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;

    // DEBUG: Log archive contents
    tracing::debug!("ZIP archive contains {} files:", archive.len());
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;
        tracing::debug!("  [{}] {} ({} bytes)", i, file.name(), file.size());
    }

    // Extract all files to the same directory as the ZIP file
    let parent_dir = zip_path.parent().ok_or_else(|| {
        CliError::ActionError(crate::actions::CliActionError::IoError(
            std::io::Error::other("Could not get parent directory"),
        ))
    })?;

    tracing::debug!("Extracting to: {:?}", parent_dir);

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| CliError::ActionError(crate::actions::CliActionError::ZipError(e)))?;

        let file_name = file.name().to_string();
        let file_path = parent_dir.join(file.mangled_name());

        tracing::debug!("Extracting [{}]: {} -> {:?}", i, file_name, file_path);

        if file.is_dir() {
            tracing::debug!("  Creating directory: {:?}", file_path);
            std::fs::create_dir_all(&file_path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;
        } else {
            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    CliError::ActionError(crate::actions::CliActionError::IoError(e))
                })?;
            }

            tracing::debug!("  Creating file: {:?} ({} bytes)", file_path, file.size());
            let mut output_file = std::fs::File::create(&file_path)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            std::io::copy(&mut file, &mut output_file)
                .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

            tracing::debug!(
                "  Extracted: {:?} ({} bytes)",
                file_path,
                output_file.metadata().map(|m| m.len()).unwrap_or(0)
            );
        }
    }

    // Remove the original ZIP file after successful extraction
    std::fs::remove_file(zip_path)
        .map_err(|e| CliError::ActionError(crate::actions::CliActionError::IoError(e)))?;

    Ok(())
}

/// Download an asset with retry logic.
///
/// This function attempts to download an asset and retries once if the first attempt fails.
///
/// # Arguments
///
/// * `api` - The Physna API client
/// * `tenant_id` - The tenant ID
/// * `asset_id` - The asset ID
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - The downloaded file content
/// * `Err(CliError)` - If an error occurred during download
async fn download_asset_with_retry(
    api: &mut crate::physna_v3::PhysnaApiClient,
    tenant_id: &str,
    asset_id: &str,
) -> Result<Vec<u8>, CliError> {
    use rand::Rng;

    // First attempt
    match api.download_asset(tenant_id, asset_id, None).await {
        Ok(content) => Ok(content),
        Err(e) => {
            // If the first attempt fails, wait for a random delay between 3-5 seconds and retry once
            tracing::warn!(
                "Asset download failed (attempt 1), retrying after delay: {}",
                e
            );

            // Generate random delay between 3 and 5 seconds
            let mut rng = rand::thread_rng();
            let delay_seconds = rng.gen_range(3..=5);

            tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

            // Second and final attempt
            match api.download_asset(tenant_id, asset_id, None).await {
                Ok(content) => Ok(content),
                Err(final_e) => {
                    tracing::error!("Asset download failed after retry: {}", final_e);
                    Err(CliError::PhysnaExtendedApiError(final_e))
                }
            }
        }
    }
}

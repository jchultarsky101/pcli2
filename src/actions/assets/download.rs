//! Download asset functionality.
//!
//! This module provides functionality for downloading assets and thumbnails.
//! Folder downloads live in `actions::folders`.

use crate::actions::CliActionError;
use crate::{
    commands::params::{PARAMETER_OUTPUT, PARAMETER_PATH, PARAMETER_UUID},
    error::CliError,
    physna_v3::PhysnaApiClient,
};
use clap::ArgMatches;
use std::fs::File;
use std::path::PathBuf;
use tracing::trace;

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
    let output_file_path = if let Some(output_path) = requested_output(sub_matches).as_ref() {
        output_path.clone()
    } else {
        // Use the asset name as the default output file name, provided it is a plain
        // name: it comes from the server and is joined to the working directory.
        let asset_name = asset.name();
        crate::actions::utils::safe_file_name(&asset_name).ok_or_else(|| {
            CliError::from(CliActionError::BusinessLogicError(format!(
                "The asset's name '{}' is not a safe local file name; choose one with -o/--output",
                asset_name
            )))
        })?
    };

    let tenant_id = tenant_uuid.to_string();
    let asset_id = asset.uuid().to_string();

    if asset.is_assembly() {
        download_assembly(
            ctx.api(),
            &tenant_id,
            &asset_id,
            asset.name().as_str(),
            &output_file_path,
        )
        .await?;
    } else {
        // Streamed straight to disk through a temporary file, so a multi-gigabyte
        // model is never held in memory and an interrupted transfer never leaves a
        // truncated file under the final name.
        ctx.api()
            .download_asset_to_file(
                &tenant_id,
                &asset_id,
                Some(asset.name().as_str()),
                &output_file_path,
            )
            .await?;
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
    let output_file_path = if let Some(output_path) = requested_output(sub_matches).as_ref() {
        // Validate the output file path
        if output_path.as_os_str().is_empty() {
            return Err(CliError::MissingRequiredArgument(
                "Output file path cannot be empty".to_string(),
            ));
        }

        // Check if the parent directory exists. A bare filename like
        // "thumb.png" has `parent() == Some("")`, which means the current
        // directory and is always valid.
        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
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

/// What `download_assembly` left on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyDownload {
    /// The server sent the dependency bundle: it was extracted next to
    /// `archive` and the archive itself was removed.
    Extracted { archive: PathBuf },
    /// The server had no bundle for the assembly and sent the raw source file
    /// instead. It is at `file`, under the asset's own name.
    RawFile { file: PathBuf },
}

/// Download an assembly to `output_file_path`, coping with either body the API
/// may send.
///
/// Physna serves an assembly as its dependency bundle (a ZIP holding the
/// assembly and every part it references) when it has one, or as the raw
/// source file when it does not, for example while the assembly is in the
/// `missing-dependencies` state. The body is streamed to `<output>.zip`; a
/// real archive is extracted next to it and removed, while anything else is
/// renamed to `output_file_path` unchanged.
pub async fn download_assembly(
    api: &mut PhysnaApiClient,
    tenant_id: &str,
    asset_id: &str,
    asset_name: &str,
    output_file_path: &std::path::Path,
) -> Result<AssemblyDownload, CliError> {
    // Download under a .zip name so a bundle never collides with the assembly
    // file it contains (sample17.asm -> sample17.asm.zip).
    let mut zip_file_path = output_file_path.to_path_buf();
    let zip_extension = if let Some(ext) = output_file_path.extension() {
        format!("{}.zip", ext.to_string_lossy())
    } else {
        "zip".to_string()
    };
    zip_file_path.set_extension(zip_extension);

    // Streamed straight to disk through a temporary file, so a multi-gigabyte
    // assembly is never held in memory and an interrupted transfer never leaves a
    // truncated file under the final name.
    api.download_asset_to_file(tenant_id, asset_id, Some(asset_name), &zip_file_path)
        .await?;

    if is_zip_file(&zip_file_path)? {
        tracing::debug!("Downloaded ZIP file to: {:?}", zip_file_path);
        // Extraction is blocking file I/O; kept off the async worker threads so a
        // `folder download --concurrent 8` does not stall every other download.
        let archive = zip_file_path.clone();
        let last = output_file_path
            .file_name()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(asset_name));
        tokio::task::spawn_blocking(move || extract_bundle(&archive, &last))
            .await
            .map_err(|e| {
                CliError::ActionError(CliActionError::IoError(std::io::Error::other(
                    e.to_string(),
                )))
            })??;
        Ok(AssemblyDownload::Extracted {
            archive: zip_file_path,
        })
    } else {
        tracing::debug!(
            "No dependency bundle for assembly {}: keeping the raw source file as {:?}",
            asset_id,
            output_file_path
        );
        std::fs::rename(&zip_file_path, output_file_path)?;
        Ok(AssemblyDownload::RawFile {
            file: output_file_path.to_path_buf(),
        })
    }
}

/// Whether the file starts with a ZIP signature: a local file header, or the
/// end-of-central-directory record of an empty archive.
fn is_zip_file(path: &std::path::Path) -> std::io::Result<bool> {
    use std::io::Read;

    let mut magic = Vec::with_capacity(4);
    File::open(path)?.take(4).read_to_end(&mut magic)?;
    Ok(magic == b"PK\x03\x04" || magic == b"PK\x05\x06")
}

/// Extract an assembly's dependency bundle next to the archive, then remove it.
///
/// - The archive is read from disk as it is extracted, never loaded whole: a
///   bundle can be gigabytes, and a folder download extracts several at once.
/// - An entry whose name would land outside the directory (`../x`, an absolute
///   path) is skipped, not rewritten into some other place.
/// - Everything is extracted into a staging directory first and then moved into
///   place, with `last` (the assembly file itself) moved last. `folder download
///   --resume` takes that file as proof the assembly is complete, so it must not
///   appear before its parts do; an interrupted run leaves no half-extracted
///   bundle behind that looks finished.
#[allow(clippy::result_large_err)]
pub(crate) fn extract_bundle(
    zip_path: &std::path::Path,
    last: &std::path::Path,
) -> Result<(), CliError> {
    let io = |e: std::io::Error| CliError::ActionError(CliActionError::IoError(e));
    let parent_dir = zip_path
        .parent()
        .ok_or_else(|| io(std::io::Error::other("Could not get parent directory")))?;
    let staging = parent_dir.join(format!(".pcli2-extract-{}", uuid::Uuid::new_v4()));
    let result: Result<(), CliError> = (|| {
        let file = File::open(zip_path).map_err(io)?;
        let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file))
            .map_err(|e| CliError::ActionError(CliActionError::ZipError(e)))?;
        tracing::debug!(
            "Extracting {} entries from {:?} to {:?}",
            archive.len(),
            zip_path,
            parent_dir
        );

        let mut extracted: Vec<PathBuf> = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| CliError::ActionError(CliActionError::ZipError(e)))?;
            let Some(relative) = entry.enclosed_name() else {
                tracing::warn!(
                    "Skipping '{}' in {:?}: its path leads outside the download directory",
                    entry.name(),
                    zip_path
                );
                continue;
            };
            let staged = staging.join(&relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&staged).map_err(io)?;
                continue;
            }
            if let Some(parent) = staged.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            let mut out = std::io::BufWriter::new(File::create(&staged).map_err(io)?);
            std::io::copy(&mut entry, &mut out).map_err(io)?;
            std::io::Write::flush(&mut out).map_err(io)?;
            tracing::debug!("  extracted {:?} ({} bytes)", relative, entry.size());
            extracted.push(relative);
        }

        // The assembly file goes last, so its presence means the bundle is complete.
        extracted.sort_by_key(|relative| relative.as_path() == last);
        for relative in &extracted {
            let target = parent_dir.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            std::fs::rename(staging.join(relative), &target).map_err(io)?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result?;

    std::fs::remove_file(zip_path).map_err(io)?;
    Ok(())
}

/// The output path the user asked for with `-o/--output`.
fn requested_output(sub_matches: &clap::ArgMatches) -> Option<PathBuf> {
    sub_matches.get_one::<PathBuf>(PARAMETER_OUTPUT).cloned()
}

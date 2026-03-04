//! Metadata operations functionality.
//!
//! This module provides functionality for metadata inference operations.

use crate::{
    actions::CliActionError,
    commands::params::{PARAMETER_FORMAT, PARAMETER_HEADERS, PARAMETER_PRETTY},
    configuration::Configuration,
    error::CliError,
    param_utils::get_tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;

/// Apply metadata inference from a reference asset to geometrically similar assets
///
/// This function finds geometrically similar assets to a reference asset and copies specified metadata fields
/// from the reference to the similar assets.
///
/// # Arguments
///
/// * `sub_matches` - The command line arguments
///
/// # Returns
///
/// * `Ok(())` - If the operation completed successfully
/// * `Err(CliError)` - If an error occurred during the operation
pub async fn metadata_inference(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing metadata inference command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get the reference asset path
    let asset_path = sub_matches
        .get_one::<String>("path")
        .ok_or_else(|| CliError::MissingRequiredArgument("path".to_string()))?;

    // Get the metadata field names to copy
    let metadata_names: Vec<String> = sub_matches
        .get_many::<String>("inference_name")
        .ok_or_else(|| CliError::MissingRequiredArgument("name".to_string()))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get format parameters
    let format_str = sub_matches
        .get_one::<String>(PARAMETER_FORMAT)
        .map(|s| s.as_str())
        .unwrap_or("json");

    let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(PARAMETER_PRETTY);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    };

    let format =
        crate::format::OutputFormat::from_string_with_options(format_str, format_options.clone())
            .map_err(|e| {
            CliActionError::FormattingError(crate::format::FormattingError::FormatFailure {
                cause: Box::new(e),
            })
        })?;

    // Get the reference asset
    let reference_asset = api.get_asset_by_path(&tenant.uuid, asset_path).await?;

    // Check if the reference asset has any of the requested metadata fields BEFORE performing expensive geometric search
    let reference_metadata = reference_asset.metadata();
    let mut available_fields = Vec::new();
    if let Some(asset_metadata) = reference_metadata {
        for field_name in &metadata_names {
            if asset_metadata.get(field_name).is_some() {
                available_fields.push(field_name.clone());
            }
        }
    }

    // Fail fast if no requested fields exist in the reference asset
    if available_fields.is_empty() {
        return Err(CliError::from(CliActionError::MissingRequiredArgument(
            format!(
                "Reference asset '{}' has no metadata fields matching: {:?}",
                asset_path, metadata_names
            ),
        )));
    }

    // Only perform expensive geometric search if we know we have fields to copy
    let search_results = api
        .geometric_search(&tenant.uuid, &reference_asset.uuid(), threshold)
        .await?;

    let mut assets_updated = Vec::new();

    // Extract the parent folder path from the reference asset
    let reference_parent_folder_path = {
        let path_str = reference_asset.path().to_string();
        let path_parts: Vec<&str> = path_str.split('/').collect();
        if path_parts.len() > 1 {
            // Join all parts except the last one (filename) to get the parent folder path
            path_parts[..path_parts.len() - 1].join("/")
        } else {
            // If there's only one part, the parent is root
            "".to_string()
        }
    };

    for match_result in search_results.matches {
        // If exclusive flag is set, only process assets in the same parent folder
        if exclusive {
            let candidate_parent_folder_path = {
                let path_str = match_result.asset.path.clone();
                let path_parts: Vec<&str> = path_str.split('/').collect();
                if path_parts.len() > 1 {
                    path_parts[..path_parts.len() - 1].join("/")
                } else {
                    "".to_string()
                }
            };

            // Skip if the candidate asset is not in the same parent folder as the reference
            if candidate_parent_folder_path != reference_parent_folder_path {
                continue;
            }
        }

        // Create a new metadata map with only the specified fields from the reference asset
        let mut new_metadata_map = std::collections::HashMap::new();

        if let Some(asset_metadata) = reference_metadata {
            for field_name in &available_fields {
                if let Some(value) = asset_metadata.get(field_name) {
                    new_metadata_map
                        .insert(field_name.clone(), serde_json::Value::String(value.clone()));
                }
            }
        }

        // Update the similar asset with the copied metadata with automatic registration of new keys
        if !new_metadata_map.is_empty() {
            api.update_asset_metadata_with_registration(
                &tenant.uuid,
                &match_result.asset.uuid,
                &new_metadata_map,
            )
            .await?;

            // Track which assets were updated
            assets_updated.push((match_result.asset.path.clone(), new_metadata_map.clone()));
        }
    }

    // Create a response structure to output the results
    let response = serde_json::json!({
        "reference_asset_path": asset_path,
        "reference_asset_uuid": reference_asset.uuid(),
        "threshold": threshold,
        "fields_copied": metadata_names,
        "assets_updated": assets_updated.len(),
        "updated_assets": assets_updated
    });

    match format {
        crate::format::OutputFormat::Json(options) => {
            if options.pretty {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("{}", serde_json::to_string(&response)?);
            }
        }
        crate::format::OutputFormat::Csv(options) => {
            // For CSV output, create a simple table
            let mut wtr = csv::Writer::from_writer(vec![]);

            if options.with_headers {
                if let Err(e) = wtr.serialize([
                    "REFERENCE_ASSET_PATH",
                    "CANDIDATE_ASSET_PATH",
                    "FIELD_NAME",
                    "FIELD_VALUE",
                    "THRESHOLD",
                ]) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for (candidate_asset_path, metadata_map) in &assets_updated {
                for (field_name, field_value) in metadata_map {
                    if let Err(e) = wtr.serialize([
                        asset_path,                         // REFERENCE_ASSET_PATH - the reference asset path
                        candidate_asset_path, // CANDIDATE_ASSET_PATH - the asset that received the metadata
                        field_name,           // FIELD_NAME
                        field_value.as_str().unwrap_or(""), // FIELD_VALUE
                        &threshold.to_string(), // THRESHOLD
                    ]) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                }
            }

            let data = match wtr.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvIntoInnerError(e),
                    )));
                }
            };
            let csv_string = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };
            print!("{}", csv_string);
        }
        _ => {
            // Default to JSON
            if format_options.pretty {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("{}", serde_json::to_string(&response)?);
            }
        }
    }

    Ok(())
}

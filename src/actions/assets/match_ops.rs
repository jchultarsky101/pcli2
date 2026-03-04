//! Match operations functionality.
//!
//! This module provides functionality for finding matching assets using
//! various search algorithms (geometric, part, visual, text).

use crate::{
    actions::CliActionError,
    commands::params::{
        PARAMETER_FOLDER_PATH, PARAMETER_FORMAT, PARAMETER_FUZZY, PARAMETER_HEADERS,
        PARAMETER_METADATA, PARAMETER_PATH, PARAMETER_PRETTY, PARAMETER_UUID,
    },
    configuration::Configuration,
    error::CliError,
    error_utils,
    format::{CsvRecordProducer, OutputFormatter},
    param_utils::get_tenant,
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tracing::trace;

/// Perform geometric matching on a single asset.
///
/// This function handles the "asset match geometric" command, finding geometrically
/// similar assets to a specified asset.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn geometric_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing geometric match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;
    let _with_metadata = format_params.format_options.with_metadata;
    let _with_headers = format_params.format_options.with_headers;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Perform geometric search
    let mut search_results = ctx
        .api()
        .geometric_search(&tenant_uuid, &asset.uuid(), threshold)
        .await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let comparison_url = if base_url.ends_with("/tenants") {
            format!(
                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.match_percentage
            )
        } else {
            format!(
                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.match_percentage
            )
        };
        match_result.comparison_url = Some(comparison_url);
    }

    // Create a basic AssetResponse from the asset for the reference
    let metadata_map = if let Some(asset_metadata) = asset.metadata() {
        // Convert AssetMetadata to HashMap<String, serde_json::Value>
        let mut map = std::collections::HashMap::new();
        for key in asset_metadata.keys() {
            if let Some(value) = asset_metadata.get(key) {
                map.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let reference_asset_response = crate::model::AssetResponse {
        uuid: asset.uuid(),
        tenant_id: tenant_uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,  // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedGeometricSearchResponse {
        reference_asset: reference_asset_response,
        matches: search_results.matches,
    };

    println!("{}", enhanced_response.format(format)?);

    Ok(())
}

/// Perform part matching on a single asset.
///
/// This function handles the "asset match part" command, finding parts
/// similar to those in a specified asset.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn part_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing part match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;
    let with_metadata = format_params.format_options.with_metadata;
    let _with_headers = format_params.format_options.with_headers;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Perform part search
    let mut search_results = ctx
        .api()
        .part_search(&tenant_uuid, &asset.uuid(), threshold)
        .await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let comparison_url = if base_url.ends_with("/tenants") {
            format!(
                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.forward_match_percentage.unwrap_or(0.0),
                match_result.reverse_match_percentage.unwrap_or(0.0)
            )
        } else {
            format!(
                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
                match_result.forward_match_percentage.unwrap_or(0.0),
                match_result.reverse_match_percentage.unwrap_or(0.0)
            )
        };
        match_result.comparison_url = Some(comparison_url);
    }

    // Create a basic AssetResponse from the asset for the reference
    let metadata_map = if let Some(asset_metadata) = asset.metadata() {
        // Convert AssetMetadata to HashMap<String, serde_json::Value>
        let mut map = std::collections::HashMap::new();
        for key in asset_metadata.keys() {
            if let Some(value) = asset_metadata.get(key) {
                map.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let reference_asset_response = crate::model::AssetResponse {
        uuid: asset.uuid(),
        tenant_id: tenant_uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,  // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedPartSearchResponse {
        reference_asset: reference_asset_response,
        matches: search_results.matches,
    };

    // Format the response considering the metadata flag
    println!(
        "{}",
        enhanced_response.format_with_metadata_flag(format, with_metadata)?
    );

    Ok(())
}

/// Perform visual matching on a single asset.
///
/// This function handles the "asset match visual" command, finding visually
/// similar assets to a specified asset.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn visual_match_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing visual match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    let asset_uuid_param = sub_matches.get_one::<uuid::Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;
    let with_metadata = format_params.format_options.with_metadata;
    let with_headers = format_params.format_options.with_headers;

    // Extract tenant info before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Perform visual search
    let mut search_results = ctx.api().visual_search(&tenant_uuid, &asset.uuid()).await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate comparison URLs for each match
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let comparison_url = if base_url.ends_with("/tenants") {
            format!(
                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                base_url,    // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
            )
        } else {
            format!(
                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                base_url, // Use configurable UI base URL without trailing slash
                tenant_name, // Use tenant short name in path
                asset.uuid(),
                match_result.asset.uuid,
                tenant_uuid, // Use tenant UUID in query params
                tenant_uuid, // Use tenant UUID in query params
            )
        };
        match_result.comparison_url = Some(comparison_url);
    }

    // Create a basic AssetResponse from the asset for the reference
    let metadata_map = if let Some(asset_metadata) = asset.metadata() {
        // Convert AssetMetadata to HashMap<String, serde_json::Value>
        let mut map = std::collections::HashMap::new();
        for key in asset_metadata.keys() {
            if let Some(value) = asset_metadata.get(key) {
                map.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let reference_asset_response = crate::model::AssetResponse {
        uuid: asset.uuid(),
        tenant_id: tenant_uuid, // Use the tenant UUID
        path: asset.path(),
        folder_id: None, // We don't have folder ID in the Asset struct
        asset_type: "asset".to_string(), // Default asset type
        created_at: "".to_string(), // Placeholder for creation time
        updated_at: "".to_string(), // Placeholder for update time
        state: "active".to_string(), // Default state
        is_assembly: false, // Default is not assembly
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,  // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    // Create visual match pairs that exclude match percentages since visual search doesn't have them
    let visual_match_pairs: Vec<crate::model::VisualMatchPair> = search_results
        .matches
        .into_iter()
        .map(|match_result| crate::model::VisualMatchPair {
            reference_asset: reference_asset_response.clone(),
            candidate_asset: match_result.asset,
            comparison_url: match_result.comparison_url,
        })
        .collect();

    // Format the response based on the output format
    match format {
        crate::format::OutputFormat::Json(_) => {
            println!("{}", serde_json::to_string_pretty(&visual_match_pairs)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            let mut wtr = csv::Writer::from_writer(vec![]);

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &visual_match_pairs {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::VisualMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in &visual_match_pairs {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
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
            let output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            println!("{}", serde_json::to_string_pretty(&visual_match_pairs)?);
        }
    }

    Ok(())
}

/// Perform geometric matching on assets in one or more folders.
///
/// This function handles the "folder match geometric" command, finding geometrically
/// similar assets among all assets in the specified folders.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn geometric_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing geometric match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument(
            PARAMETER_FOLDER_PATH.to_string(),
        ))?
        .map(|s| s.to_string())
        .collect();

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;
    let with_metadata = format_params.format_options.with_metadata;
    let with_headers = format_params.format_options.with_headers;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if !(1..=10).contains(&val) {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Invalid value for '--concurrent': must be between 1 and 10, got {}",
                    val
                )));
            }
            val
        }
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api
            .list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str())
            .await?;

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        error_utils::report_error_with_remediation(
            &"No assets found in the specified folder(s)",
            &[
                "Verify the folder path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the specified folder(s)",
            ],
        );
        return Ok(());
    }

    // Create multi-progress bar if show_progress is true
    let multi_progress = if show_progress {
        let mp = MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(ProgressBar::new(all_assets.len() as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent));

    // Prepare for concurrent processing
    let mut all_matches = Vec::new();

    // Use a set to track unique pairs to avoid duplicates (reference UUID, candidate UUID)
    let mut seen_pairs = std::collections::HashSet::new();

    // Create tasks for concurrent processing
    type TaskResult = Result<
        Vec<crate::model::EnhancedGeometricSearchResponse>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();
    for (asset_uuid, asset) in &all_assets {
        let semaphore = semaphore.clone();
        let mut api_clone = api.clone(); // Clone the API client
        let tenant_uuid = tenant.uuid;
        let asset_uuid = *asset_uuid;
        let asset_clone = asset.clone();
        let folder_paths_clone = folder_paths.clone();
        let tenant_clone = tenant.clone();
        let multi_progress_clone = multi_progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this task if multi-progress is enabled
            let individual_pb = if let Some((ref mp, _)) = multi_progress_clone {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "{{spinner:.green}} Processing: {} {{msg}}",
                            asset_clone.name()
                        ))
                        .unwrap(),
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting geometric search...");
            }

            let result = match api_clone
                .geometric_search(&tenant_uuid, &asset_uuid, threshold)
                .await
            {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!(
                            "Processing {} matches...",
                            search_results.matches.len()
                        ));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration =
                            crate::configuration::Configuration::load_or_create_default().map_err(
                                |e| {
                                    CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e),
                                }
                            )
                                },
                            )?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let base_url = ui_base_url.trim_end_matches('/');
                        let comparison_url = if base_url.ends_with("/tenants") {
                            format!(
                                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.match_percentage
                            )
                        } else {
                            format!(
                                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=geometric&matchPercentage={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.match_percentage
                            )
                        };
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        // For exclusive mode, both reference and candidate assets must be in specified folders
                        let candidate_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_candidate_path =
                                    crate::model::normalize_path(&match_result.asset.path);
                                normalized_candidate_path.starts_with(&normalized_folder_path)
                            });

                        let reference_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_reference_path =
                                    crate::model::normalize_path(asset_clone.path());
                                normalized_reference_path.starts_with(&normalized_folder_path)
                            });

                        if exclusive
                            && (!candidate_in_specified_folders || !reference_in_specified_folders)
                        {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
                            // Convert AssetMetadata to HashMap<String, serde_json::Value>
                            let mut map = std::collections::HashMap::new();
                            for key in asset_metadata.keys() {
                                if let Some(value) = asset_metadata.get(key) {
                                    map.insert(
                                        key.clone(),
                                        serde_json::Value::String(value.clone()),
                                    );
                                }
                            }
                            map
                        } else {
                            std::collections::HashMap::new()
                        };

                        let reference_asset_response = crate::model::AssetResponse {
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(),      // Placeholder for creation time
                            updated_at: "".to_string(),      // Placeholder for update time
                            state: "active".to_string(),     // Default state
                            is_assembly: false,              // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None,         // No owner ID
                        };

                        let enhanced_match = crate::model::EnhancedGeometricSearchResponse {
                            reference_asset: reference_asset_response,
                            matches: vec![match_result.clone()],
                        };

                        asset_matches.push(enhanced_match);
                    }

                    // Update progress bar to show completion
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Found {} matches", asset_matches.len()));
                    }

                    Ok(asset_matches)
                }
                Err(e) => {
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform geometric search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    Ok(Vec::new()) // Return empty vector on error
                }
            };

            // Remove the individual progress bar when done
            if let Some(pb) = individual_pb {
                pb.finish_and_clear();
            }

            result
        });

        tasks.push(task);
    }

    // Process tasks and collect results
    for task in tasks {
        match task.await {
            Ok(Ok(asset_matches)) => {
                for enhanced_match in asset_matches {
                    // Apply duplicate filtering to each match
                    for match_result in &enhanced_match.matches {
                        // Create a unique pair identifier to avoid duplicates
                        // We want to avoid having both (A,B) and (B,A) in results
                        let (ref_uuid, cand_uuid) =
                            if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
                                (enhanced_match.reference_asset.uuid, match_result.asset.uuid)
                            } else {
                                (match_result.asset.uuid, enhanced_match.reference_asset.uuid)
                            };

                        let pair_key = (ref_uuid, cand_uuid);

                        if !seen_pairs.contains(&pair_key) {
                            seen_pairs.insert(pair_key);
                            all_matches.push(enhanced_match.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error_utils::report_error_with_remediation(
                    &format!("Error processing asset: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify the asset exists and is accessible",
                        "Retry the operation",
                    ],
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation",
                    ],
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!(
            "Processed {} assets. Found {} unique matches.",
            all_assets.len(),
            all_matches.len()
        ));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(
                        crate::model::GeometricMatchPair::from_reference_and_match(
                            enhanced_response.reference_asset.clone(),
                            match_result,
                        ),
                    );
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(
                        crate::model::GeometricMatchPair::from_reference_and_match(
                            enhanced_response.reference_asset.clone(),
                            match_result,
                        ),
                    );
                }
            }

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv::Writer::from_writer(vec![]);

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &flattened_matches {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::GeometricMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    format!("{}", match_pair.match_percentage),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
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
            let output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(
                        crate::model::GeometricMatchPair::from_reference_and_match(
                            enhanced_response.reference_asset.clone(),
                            match_result,
                        ),
                    );
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

    Ok(())
}

/// Perform part matching on assets in one or more folders.
///
/// This function handles the "folder match part" command, finding parts
/// similar among all assets in the specified folders.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn part_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing part match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument(
            PARAMETER_FOLDER_PATH.to_string(),
        ))?
        .cloned()
        .collect();

    // Get threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

    // Get format parameters
    let format_str = if let Some(format_val) = sub_matches.get_one::<String>(PARAMETER_FORMAT) {
        format_val.clone()
    } else {
        // Check environment variable first, then use default
        if let Ok(env_format) = std::env::var("PCLI2_FORMAT") {
            env_format
        } else {
            "json".to_string()
        }
    };

    let with_headers = sub_matches.get_flag(PARAMETER_HEADERS);
    let pretty = sub_matches.get_flag(PARAMETER_PRETTY);
    let with_metadata = sub_matches.get_flag(PARAMETER_METADATA);

    let format_options = crate::format::OutputFormatOptions {
        with_metadata,
        with_headers,
        pretty,
    };

    #[allow(clippy::needless_borrow)]
    let format = crate::format::OutputFormat::from_string_with_options(&format_str, format_options)
        .map_err(CliActionError::FormattingError)?;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if !(1..=10).contains(&val) {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Invalid value for '--concurrent': must be between 1 and 10, got {}",
                    val
                )));
            }
            val
        }
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api
            .list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str())
            .await?;

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        error_utils::report_error_with_remediation(
            &"No assets found in the specified folder(s)",
            &[
                "Verify the folder path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the specified folder(s)",
            ],
        );
        return Ok(());
    }

    // Create multi-progress bar if show_progress is true
    let multi_progress = if show_progress {
        let mp = MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(ProgressBar::new(all_assets.len() as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent));

    // Prepare for concurrent processing
    let mut all_matches = Vec::new();

    // Use a set to track unique pairs to avoid duplicates (reference UUID, candidate UUID)
    let mut seen_pairs = std::collections::HashSet::new();

    // Create tasks for concurrent processing
    type TaskResult = Result<
        Vec<crate::model::EnhancedPartSearchResponse>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();
    for (asset_uuid, asset) in &all_assets {
        let semaphore = semaphore.clone();
        let mut api_clone = api.clone(); // Clone the API client
        let tenant_uuid = tenant.uuid;
        let asset_uuid = *asset_uuid;
        let asset_clone = asset.clone();
        let folder_paths_clone = folder_paths.clone();
        let tenant_clone = tenant.clone();
        let multi_progress_clone = multi_progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this task if multi-progress is enabled
            let individual_pb = if let Some((ref mp, _)) = multi_progress_clone {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "{{spinner:.green}} Processing: {} {{msg}}",
                            asset_clone.name()
                        ))
                        .unwrap(),
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting part search...");
            }

            let result = match api_clone
                .part_search(&tenant_uuid, &asset_uuid, threshold)
                .await
            {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!(
                            "Processing {} matches...",
                            search_results.matches.len()
                        ));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration =
                            crate::configuration::Configuration::load_or_create_default().map_err(
                                |e| {
                                    CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e),
                                }
                            )
                                },
                            )?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let base_url = ui_base_url.trim_end_matches('/');
                        let comparison_url = if base_url.ends_with("/tenants") {
                            format!(
                                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.forward_match_percentage.unwrap_or(0.0),
                                match_result.reverse_match_percentage.unwrap_or(0.0)
                            )
                        } else {
                            format!(
                                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                                match_result.forward_match_percentage.unwrap_or(0.0),
                                match_result.reverse_match_percentage.unwrap_or(0.0)
                            )
                        };
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        // For exclusive mode, both reference and candidate assets must be in specified folders
                        let candidate_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_candidate_path =
                                    crate::model::normalize_path(&match_result.asset.path);
                                normalized_candidate_path.starts_with(&normalized_folder_path)
                            });

                        let reference_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_reference_path =
                                    crate::model::normalize_path(asset_clone.path());
                                normalized_reference_path.starts_with(&normalized_folder_path)
                            });

                        if exclusive
                            && (!candidate_in_specified_folders || !reference_in_specified_folders)
                        {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
                            // Convert AssetMetadata to HashMap<String, serde_json::Value>
                            let mut map = std::collections::HashMap::new();
                            for key in asset_metadata.keys() {
                                if let Some(value) = asset_metadata.get(key) {
                                    map.insert(
                                        key.clone(),
                                        serde_json::Value::String(value.clone()),
                                    );
                                }
                            }
                            map
                        } else {
                            std::collections::HashMap::new()
                        };

                        let reference_asset_response = crate::model::AssetResponse {
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(),      // Placeholder for creation time
                            updated_at: "".to_string(),      // Placeholder for update time
                            state: "active".to_string(),     // Default state
                            is_assembly: false,              // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None,         // No owner ID
                        };

                        let enhanced_match = crate::model::EnhancedPartSearchResponse {
                            reference_asset: reference_asset_response,
                            matches: vec![match_result.clone()],
                        };

                        asset_matches.push(enhanced_match);
                    }

                    // Update progress bar to show completion
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Found {} matches", asset_matches.len()));
                    }

                    Ok(asset_matches)
                }
                Err(e) => {
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform part search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    Ok(Vec::new()) // Return empty vector on error
                }
            };

            // Remove the individual progress bar when done
            if let Some(pb) = individual_pb {
                pb.finish_and_clear();
            }

            result
        });

        tasks.push(task);
    }

    // Process tasks and collect results
    for task in tasks {
        match task.await {
            Ok(Ok(asset_matches)) => {
                for enhanced_match in asset_matches {
                    // Apply duplicate filtering to each match
                    for match_result in &enhanced_match.matches {
                        // Create a unique pair identifier to avoid duplicates
                        // We want to avoid having both (A,B) and (B,A) in results
                        let (ref_uuid, cand_uuid) =
                            if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
                                (enhanced_match.reference_asset.uuid, match_result.asset.uuid)
                            } else {
                                (match_result.asset.uuid, enhanced_match.reference_asset.uuid)
                            };

                        let pair_key = (ref_uuid, cand_uuid);

                        if !seen_pairs.contains(&pair_key) {
                            seen_pairs.insert(pair_key);
                            all_matches.push(enhanced_match.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error_utils::report_error_with_remediation(
                    &format!("Error processing asset: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify the asset exists and is accessible",
                        "Retry the operation",
                    ],
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation",
                    ],
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!(
            "Processed {} assets. Found {} unique matches.",
            all_assets.len(),
            all_matches.len()
        ));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::PartMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result,
                    ));
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::PartMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result,
                    ));
                }
            }

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv::Writer::from_writer(vec![]);

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &flattened_matches {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::PartMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair
                        .forward_match_percentage
                        .map_or_else(|| "0.0".to_string(), |val| format!("{}", val)),
                    match_pair
                        .reverse_match_percentage
                        .map_or_else(|| "0.0".to_string(), |val| format!("{}", val)),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
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
            let output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::PartMatchPair::from_reference_and_match(
                        enhanced_response.reference_asset.clone(),
                        match_result,
                    ));
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

    Ok(())
}

/// Perform visual matching on assets in one or more folders.
///
/// This function handles the "folder match visual" command, finding visually
/// similar assets among all assets in the specified folders.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn visual_match_folder(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing visual match folder command...");

    let configuration = Configuration::load_or_create_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(PARAMETER_FOLDER_PATH)
        .ok_or(CliError::MissingRequiredArgument(
            PARAMETER_FOLDER_PATH.to_string(),
        ))?
        .cloned()
        .collect();

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;
    let with_metadata = format_params.format_options.with_metadata;
    let with_headers = format_params.format_options.with_headers;

    // Get exclusive flag
    let exclusive = sub_matches.get_flag("exclusive");

    // Get concurrent and progress parameters
    let concurrent_param = sub_matches.get_one::<usize>("concurrent").copied();
    let concurrent = match concurrent_param {
        Some(val) => {
            if !(1..=10).contains(&val) {
                return Err(CliError::MissingRequiredArgument(format!(
                    "Invalid value for '--concurrent': must be between 1 and 10, got {}",
                    val
                )));
            }
            val
        }
        None => 1, // Default value
    };

    let show_progress = sub_matches.get_flag("progress");

    // Collect all assets from the specified folders
    let mut all_assets = std::collections::HashMap::new();

    for folder_path in &folder_paths {
        trace!("Listing assets for folder path: {}", folder_path);
        let assets_response = api
            .list_assets_by_parent_folder_path(&tenant.uuid, folder_path.as_str())
            .await?;

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        error_utils::report_error_with_remediation(
            &"No assets found in the specified folder(s)",
            &[
                "Verify the folder path is correct",
                "Check that the folder contains assets",
                "Ensure you have permissions to access the specified folder(s)",
            ],
        );
        return Ok(());
    }

    // Create multi-progress bar if show_progress is true
    let multi_progress = if show_progress {
        let mp = MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(ProgressBar::new(all_assets.len() as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {per_sec}")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Use a semaphore to limit concurrent operations
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrent));

    // Prepare for concurrent processing
    let mut all_matches = Vec::new();

    // Use a set to track unique pairs to avoid duplicates (reference UUID, candidate UUID)
    let mut seen_pairs = std::collections::HashSet::new();

    // Create tasks for concurrent processing
    type TaskResult = Result<
        Vec<crate::model::EnhancedPartSearchResponse>,
        Box<dyn std::error::Error + Send + Sync>,
    >;
    let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();
    for (asset_uuid, asset) in &all_assets {
        let semaphore = semaphore.clone();
        let mut api_clone = api.clone(); // Clone the API client
        let tenant_uuid = tenant.uuid;
        let asset_uuid = *asset_uuid;
        let asset_clone = asset.clone();
        let folder_paths_clone = folder_paths.clone();
        let tenant_clone = tenant.clone();
        let multi_progress_clone = multi_progress.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create individual progress bar for this task if multi-progress is enabled
            let individual_pb = if let Some((ref mp, _)) = multi_progress_clone {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "{{spinner:.green}} Processing: {} {{msg}}",
                            asset_clone.name()
                        ))
                        .unwrap(),
                );
                Some(pb)
            } else {
                None
            };

            // Update the progress bar to show that we're starting the search
            if let Some(ref pb) = individual_pb {
                pb.set_message("Starting visual search...");
            }

            let result = match api_clone.visual_search(&tenant_uuid, &asset_uuid).await {
                Ok(search_results) => {
                    // Update progress bar to show processing matches
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!(
                            "Processing {} matches...",
                            search_results.matches.len()
                        ));
                    }

                    let mut asset_matches = Vec::new();

                    for mut match_result in search_results.matches {
                        // Skip if the match is with the same asset (self-match)
                        if match_result.asset.uuid == asset_uuid {
                            continue;
                        }

                        // Load configuration to get the UI base URL
                        let configuration =
                            crate::configuration::Configuration::load_or_create_default().map_err(
                                |e| {
                                    CliError::ConfigurationError(
                                crate::configuration::ConfigurationError::FailedToLoadData {
                                    cause: Box::new(e),
                                }
                            )
                                },
                            )?;
                        let ui_base_url = configuration.get_ui_base_url();

                        // Populate comparison URL for this match
                        let base_url = ui_base_url.trim_end_matches('/');
                        let comparison_url = if base_url.ends_with("/tenants") {
                            format!(
                                "{}/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                            )
                        } else {
                            format!(
                                "{}/tenants/{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&searchType=visual",
                                base_url, // Use configurable UI base URL without trailing slash
                                tenant_clone.name, // Use tenant short name in path
                                asset_uuid,
                                match_result.asset.uuid,
                                tenant_uuid, // Use tenant UUID in query params
                                tenant_uuid, // Use tenant UUID in query params
                            )
                        };
                        match_result.comparison_url = Some(comparison_url);

                        // Check if we want to include matches based on exclusive flag
                        // For exclusive mode, both reference and candidate assets must be in specified folders
                        let candidate_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_candidate_path =
                                    crate::model::normalize_path(&match_result.asset.path);
                                normalized_candidate_path.starts_with(&normalized_folder_path)
                            });

                        let reference_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_reference_path =
                                    crate::model::normalize_path(asset_clone.path());
                                normalized_reference_path.starts_with(&normalized_folder_path)
                            });

                        if exclusive
                            && (!candidate_in_specified_folders || !reference_in_specified_folders)
                        {
                            continue;
                        }

                        // Create the enhanced response structure for this match
                        let metadata_map = if let Some(asset_metadata) = asset_clone.metadata() {
                            // Convert AssetMetadata to HashMap<String, serde_json::Value>
                            let mut map = std::collections::HashMap::new();
                            for key in asset_metadata.keys() {
                                if let Some(value) = asset_metadata.get(key) {
                                    map.insert(
                                        key.clone(),
                                        serde_json::Value::String(value.clone()),
                                    );
                                }
                            }
                            map
                        } else {
                            std::collections::HashMap::new()
                        };

                        let reference_asset_response = crate::model::AssetResponse {
                            uuid: asset_uuid,
                            tenant_id: tenant_uuid,
                            path: asset_clone.path(),
                            folder_id: None,
                            asset_type: "asset".to_string(), // Default asset type
                            created_at: "".to_string(),      // Placeholder for creation time
                            updated_at: "".to_string(),      // Placeholder for update time
                            state: "active".to_string(),     // Default state
                            is_assembly: false,              // Default is not assembly
                            metadata: metadata_map,
                            parent_folder_id: None, // No parent folder ID
                            owner_id: None,         // No owner ID
                        };

                        let enhanced_match = crate::model::EnhancedPartSearchResponse {
                            reference_asset: reference_asset_response,
                            matches: vec![match_result.clone()],
                        };

                        asset_matches.push(enhanced_match);
                    }

                    // Update progress bar to show completion
                    if let Some(ref pb) = individual_pb {
                        pb.set_message(format!("Found {} matches", asset_matches.len()));
                    }

                    Ok(asset_matches)
                }
                Err(e) => {
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform visual search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    Ok(Vec::new()) // Return empty vector on error
                }
            };

            // Remove the individual progress bar when done
            if let Some(pb) = individual_pb {
                pb.finish_and_clear();
            }

            result
        });

        tasks.push(task);
    }

    // Process tasks and collect results
    for task in tasks {
        match task.await {
            Ok(Ok(asset_matches)) => {
                for enhanced_match in asset_matches {
                    // Apply duplicate filtering to each match
                    for match_result in &enhanced_match.matches {
                        // Create a unique pair identifier to avoid duplicates
                        // We want to avoid having both (A,B) and (B,A) in results
                        let (ref_uuid, cand_uuid) =
                            if enhanced_match.reference_asset.uuid < match_result.asset.uuid {
                                (enhanced_match.reference_asset.uuid, match_result.asset.uuid)
                            } else {
                                (match_result.asset.uuid, enhanced_match.reference_asset.uuid)
                            };

                        let pair_key = (ref_uuid, cand_uuid);

                        if !seen_pairs.contains(&pair_key) {
                            seen_pairs.insert(pair_key);
                            all_matches.push(enhanced_match.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error_utils::report_error_with_remediation(
                    &format!("Error processing asset: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify the asset exists and is accessible",
                        "Retry the operation",
                    ],
                );
            }
            Err(e) => {
                error_utils::report_error_with_remediation(
                    &format!("Task failed: {:?}", e),
                    &[
                        "Check your network connection",
                        "Verify your authentication credentials are valid",
                        "Retry the operation",
                    ],
                );
            }
        }

        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!(
            "Processed {} assets. Found {} unique matches.",
            all_assets.len(),
            all_matches.len()
        ));
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::VisualMatchPair {
                        reference_asset: enhanced_response.reference_asset.clone(),
                        candidate_asset: match_result.asset,
                        comparison_url: match_result.comparison_url,
                    });
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::VisualMatchPair {
                        reference_asset: enhanced_response.reference_asset.clone(),
                        candidate_asset: match_result.asset,
                        comparison_url: match_result.comparison_url,
                    });
                }
            }

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv::Writer::from_writer(vec![]);

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                let mut all_metadata_keys = std::collections::HashSet::new();
                for match_pair in &flattened_matches {
                    for key in match_pair.reference_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                    for key in match_pair.candidate_asset.metadata.keys() {
                        all_metadata_keys.insert(key.clone());
                    }
                }

                // Sort metadata keys for consistent column ordering
                let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                sorted_keys.sort();
                header_metadata_keys = sorted_keys;
            }

            if with_headers {
                // Build header with metadata columns
                let mut base_headers = crate::model::VisualMatchPair::csv_header();

                if with_metadata {
                    // Add metadata columns with prefixes
                    for key in &header_metadata_keys {
                        base_headers.push(format!("REF_{}", key.to_uppercase()));
                        base_headers.push(format!("CAND_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            for match_pair in flattened_matches {
                let mut base_values = vec![
                    match_pair.reference_asset.path.clone(),
                    match_pair.candidate_asset.path.clone(),
                    match_pair.reference_asset.uuid.to_string(),
                    match_pair.candidate_asset.uuid.to_string(),
                    match_pair.comparison_url.clone().unwrap_or_default(),
                ];

                if with_metadata {
                    // Add metadata values for each key that was included in the header
                    for key in &header_metadata_keys {
                        // Add reference asset metadata value
                        let ref_value = match_pair
                            .reference_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        base_values.push(cand_value);
                    }
                }

                if let Err(e) = wtr.serialize(base_values.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
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
            let output = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            print!("{}", output);
        }
        _ => {
            // Default to JSON
            let mut flattened_matches = Vec::new();
            for enhanced_response in all_matches {
                for match_result in enhanced_response.matches {
                    flattened_matches.push(crate::model::VisualMatchPair {
                        reference_asset: enhanced_response.reference_asset.clone(),
                        candidate_asset: match_result.asset,
                        comparison_url: match_result.comparison_url,
                    });
                }
            }
            println!("{}", serde_json::to_string_pretty(&flattened_matches)?);
        }
    }

    Ok(())
}

/// Perform text matching (search) on assets.
///
/// This function handles the "asset match text" command, performing a text search
/// across all assets in the tenant.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the match operation was successful
/// * `Err(CliError)` - If an error occurred during the match
pub async fn text_match(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing text match command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    // Get the text query parameter
    let text_query = sub_matches
        .get_one::<String>("text")
        .ok_or(CliError::MissingRequiredArgument(
            "text query is required".to_string(),
        ))?
        .clone();

    // Get the fuzzy flag - if not specified, default to false (meaning exact search with quoted text)
    let fuzzy = sub_matches.get_flag(PARAMETER_FUZZY);

    // If fuzzy is false (default), wrap the text query in quotes for exact search
    let search_query = if fuzzy {
        text_query.clone()
    } else {
        format!("\"{}\"", text_query)
    };

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;

    // Extract tenant info before calling text search
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Perform text search
    let mut search_results = ctx.api().text_search(&tenant_uuid, &search_query).await?;

    // Load configuration to get the UI base URL
    let configuration =
        crate::configuration::Configuration::load_or_create_default().map_err(|e| {
            CliError::ConfigurationError(
                crate::configuration::ConfigurationError::FailedToLoadData { cause: Box::new(e) },
            )
        })?;
    let ui_base_url = configuration.get_ui_base_url();

    // Populate asset URLs for each match (not comparison URLs since text search doesn't compare two assets)
    for match_result in &mut search_results.matches {
        let base_url = ui_base_url.trim_end_matches('/');
        let asset_url = format!(
            "{}/tenants/{}/asset/{}",
            base_url,    // Use configurable UI base URL without trailing slash
            tenant_name, // Use tenant short name in path
            match_result.asset.uuid
        );
        match_result.comparison_url = Some(asset_url); // Store asset URL in comparison_url field for text search
    }

    // Create enhanced response that includes the search query information
    let enhanced_response = crate::model::EnhancedTextSearchResponse {
        search_query: text_query.clone(), // Use the original user input for display
        matches: search_results.matches,
    };

    // Format the response based on the output format
    match format {
        crate::format::OutputFormat::Json(options) => {
            if options.pretty {
                println!("{}", serde_json::to_string_pretty(&enhanced_response)?);
            } else {
                println!("{}", serde_json::to_string(&enhanced_response)?);
            }
        }
        crate::format::OutputFormat::Csv(options) => {
            let mut wtr = csv::Writer::from_writer(vec![]);

            if options.with_headers {
                if options.with_metadata {
                    // Include metadata columns in the header
                    let mut base_headers = crate::model::EnhancedTextSearchResponse::csv_header();

                    // Get unique metadata keys from all assets in the response
                    let mut all_metadata_keys = std::collections::HashSet::new();
                    for match_result in &enhanced_response.matches {
                        for key in match_result.asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Extend headers with metadata columns
                    for key in &sorted_keys {
                        base_headers.push(format!("ASSET_{}", key.to_uppercase()));
                    }

                    if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                } else {
                    let headers = crate::model::EnhancedTextSearchResponse::csv_header();
                    if let Err(e) = wtr.serialize(headers.as_slice()) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                }
            }

            for match_result in &enhanced_response.matches {
                if options.with_metadata {
                    // Include metadata values in the output
                    let base_values = vec![
                        match_result
                            .asset
                            .path
                            .rsplit_once('/')
                            .map(|(_, name)| name.to_string())
                            .unwrap_or(match_result.asset.path.clone()), // ASSET_NAME
                        match_result.asset.path.clone(), // ASSET_PATH
                        match_result.asset.asset_type.clone(), // TYPE
                        match_result.asset.state.clone(), // STATE
                        match_result.asset.is_assembly.to_string(), // IS_ASSEMBLY
                        format!("{}", match_result.relevance_score.unwrap_or(0.0)), // RELEVANCE_SCORE
                        match_result.asset.uuid.to_string(),                        // ASSET_UUID
                        match_result.comparison_url.clone().unwrap_or_default(),    // ASSET_URL
                    ];

                    // Get unique metadata keys from all assets in the response
                    let mut all_metadata_keys = std::collections::HashSet::new();
                    for mr in &enhanced_response.matches {
                        for key in mr.asset.metadata.keys() {
                            all_metadata_keys.insert(key.clone());
                        }
                    }

                    // Sort metadata keys for consistent column ordering
                    let mut sorted_keys: Vec<String> = all_metadata_keys.into_iter().collect();
                    sorted_keys.sort();

                    // Add metadata values for each key
                    let mut extended_values = base_values.clone();
                    for key in &sorted_keys {
                        let value = match_result
                            .asset
                            .metadata
                            .get(key)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        extended_values.push(value);
                    }

                    if let Err(e) = wtr.serialize(extended_values.as_slice()) {
                        return Err(CliError::from(CliActionError::FormattingError(
                            crate::format::FormattingError::CsvError(e),
                        )));
                    }
                } else {
                    let records = match_result.as_csv_records();
                    for record in records {
                        if let Err(e) = wtr.serialize(record.as_slice()) {
                            return Err(CliError::from(CliActionError::FormattingError(
                                crate::format::FormattingError::CsvError(e),
                            )));
                        }
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
            let output: String = match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };
            print!("{}", output);
        }
        _ => {
            // Default to JSON
            println!("{}", serde_json::to_string_pretty(&enhanced_response)?);
        }
    }

    Ok(())
}

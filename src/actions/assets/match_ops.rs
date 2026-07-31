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
    terminal::ReportProgress,
};
use clap::ArgMatches;
use indicatif::{HumanCount, MultiProgress, ProgressBar, ProgressStyle};
use tracing::trace;
use uuid::Uuid;

/// Why a per-asset search contributed no matches to a folder match report.
///
/// The distinction matters for the exit status. A large tenant always contains some
/// assets that simply cannot be searched, and a report is not wrong for omitting
/// them. A search that failed because the run could not talk to the API is a
/// different thing: the report is missing rows it should have had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchFailure {
    /// The asset cannot be searched in its current state - not indexed yet, no 3D
    /// data, indexing failed. A property of the data, not of this run.
    NotSearchable,
    /// Anything else: authentication, network, 5xx, exhausted retries. The run did
    /// not do what it was asked to do.
    Operational,
}

impl SearchFailure {
    /// Classify a failed search.
    ///
    /// The search endpoint reports an unsearchable asset as a `409 Conflict`
    /// (`Asset not indexed yet`, `Asset has no 3D data`, `Asset failed to index`),
    /// which is the only failure that is expected in normal operation. Everything
    /// else is treated as operational - deliberately, so that a failure mode nobody
    /// anticipated is loud rather than silently written off as routine.
    fn classify(error: &crate::physna_v3::ApiError) -> Self {
        match error {
            crate::physna_v3::ApiError::ConflictError(_) => Self::NotSearchable,
            _ => Self::Operational,
        }
    }
}

/// Running tally of per-asset search outcomes across a folder match run.
#[derive(Debug, Default, Clone, Copy)]
struct SearchOutcomes {
    attempted: usize,
    not_searchable: usize,
    operational: usize,
}

/// The share of operational failures above which a run is treated as failed rather
/// than merely degraded.
///
/// Not zero: a long run over tens of thousands of assets can lose a couple of
/// searches to a stale token being refreshed mid-flight without the report being
/// meaningfully incomplete, and failing the whole command over that would train
/// people to ignore the exit code. Well below a half: the case this exists to catch
/// is a systemic failure - an expired credential, a network partition - where most
/// of the report is missing. Either way the counts are always reported, so a run
/// under the threshold is still visible rather than silent.
const OPERATIONAL_FAILURE_EXIT_THRESHOLD: f64 = 0.10;

impl SearchOutcomes {
    fn record(&mut self, failure: Option<SearchFailure>) {
        self.attempted += 1;
        match failure {
            Some(SearchFailure::NotSearchable) => self.not_searchable += 1,
            Some(SearchFailure::Operational) => self.operational += 1,
            None => {}
        }
    }

    fn failed(&self) -> usize {
        self.not_searchable + self.operational
    }

    fn succeeded(&self) -> usize {
        self.attempted.saturating_sub(self.failed())
    }

    /// Whether the operational failures are severe enough that the report should not
    /// be presented as a successful result.
    fn is_materially_incomplete(&self) -> bool {
        self.attempted > 0
            && (self.operational as f64 / self.attempted as f64)
                > OPERATIONAL_FAILURE_EXIT_THRESHOLD
    }

    /// One-line account of what the run actually managed to search, or `None` when
    /// everything succeeded and there is nothing worth saying.
    fn summary(&self) -> Option<String> {
        if self.failed() == 0 {
            return None;
        }
        let mut reasons = Vec::new();
        if self.operational > 0 {
            reasons.push(format!("{} failed", HumanCount(self.operational as u64)));
        }
        if self.not_searchable > 0 {
            reasons.push(format!(
                "{} not searchable",
                HumanCount(self.not_searchable as u64)
            ));
        }
        Some(format!(
            "Searched {} of {} asset(s): {}",
            HumanCount(self.succeeded() as u64),
            HumanCount(self.attempted as u64),
            reasons.join(", ")
        ))
    }
}

/// Report what a run managed to search, and fail when too much of it did not.
///
/// Always reports when anything failed, whatever the exit status: the previous
/// behaviour printed a completion summary that implied a whole report when most of
/// the searches had failed, which is worse than an outright error because nothing
/// prompts the user to re-run.
#[allow(clippy::result_large_err)]
fn finish_search_outcomes(outcomes: &SearchOutcomes) -> Result<(), CliError> {
    let Some(summary) = outcomes.summary() else {
        return Ok(());
    };

    if outcomes.is_materially_incomplete() {
        error_utils::report_error_with_remediation(
            &format!("{} - the report would be incomplete", summary),
            &[
                "Check that you are still logged in ('pcli2 auth login')",
                "Check your network connection and retry",
                "Re-run with --verbose to see why the individual searches failed",
            ],
        );
        return Err(CliError::from(CliActionError::IncompleteReport {
            attempted: outcomes.attempted,
            failed: outcomes.operational,
        }));
    }

    error_utils::report_warning(&summary);
    Ok(())
}

/// Count the direct subfolders of the given folder paths.
///
/// Used only to make the "no assets found" message actionable: a folder that holds
/// nothing but subfolders looks empty to a non-recursive report, and the user needs to
/// be told that `--recursive` is what they want. Returns 0 if the hierarchy is
/// unavailable, which merely costs a less specific message.
async fn count_subfolders(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    folder_paths: &[String],
) -> usize {
    let Ok(hierarchy) = crate::folder_cache::FolderCache::get_or_fetch(api, tenant_uuid).await
    else {
        return 0;
    };

    folder_paths
        .iter()
        .map(|folder_path| {
            let normalized = crate::model::normalize_path(folder_path);
            if normalized == "/" {
                return hierarchy.root_uuids.len();
            }
            let lookup = normalized.strip_prefix('/').unwrap_or(&normalized);
            hierarchy
                .get_folder_by_path(lookup)
                .map(|node| node.children.len())
                .unwrap_or(0)
        })
        .sum()
}

/// Collect the assets to report on from the given folder paths.
///
/// By default this is only the assets sitting directly in each folder, matching the
/// Physna contents endpoint. With `recursive` it is every asset in the subtree, which is
/// what a container folder holding nothing but subfolders needs to produce any output.
///
/// Assets are keyed by UUID, so a folder path that overlaps another one supplied on the
/// same command line contributes each asset only once.
///
/// A recursive scan costs one API call per folder in the subtree and runs before any
/// matching starts, so with `show_progress` it reports what it is doing. Without the
/// flag it stays silent, keeping the default output unchanged.
///
/// # Returns
/// * `Ok(Some(assets))` - The assets found, guaranteed non-empty
/// * `Ok(None)` - Nothing was found; remediation advice has already been printed and the
///   caller should return without producing a report
/// * `Err(CliError)` - A path did not resolve, or an API call failed
async fn collect_assets_in_folders(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    folder_paths: &[String],
    recursive: bool,
    show_progress: bool,
) -> Result<Option<std::collections::HashMap<Uuid, crate::model::Asset>>, CliError> {
    let mut all_assets = std::collections::HashMap::new();

    // Only a recursive scan is slow enough to need reporting; the non-recursive path is
    // a single call per folder. The spinner hides itself when stderr is not a terminal,
    // so redirected output stays clean.
    let scan_progress = if recursive && show_progress {
        Some(crate::terminal::spinner("Scanning folders..."))
    } else {
        None
    };

    for folder_path in folder_paths {
        trace!(
            "Listing assets for folder path: {} (recursive: {})",
            folder_path,
            recursive
        );
        let assets_response = if recursive {
            let path_label = folder_path.clone();
            let progress = scan_progress.as_ref();
            api.list_assets_by_parent_folder_path_recursive(
                tenant_uuid,
                folder_path.as_str(),
                |scanned, total, assets| {
                    if let Some(progress) = progress {
                        progress.set_message(format!(
                            "Scanning {}: {}/{} folders, {} assets found",
                            path_label, scanned, total, assets
                        ));
                    }
                },
            )
            .await?
        } else {
            api.list_assets_by_parent_folder_path(tenant_uuid, folder_path.as_str())
                .await?
        };

        for asset in assets_response.get_all_assets() {
            all_assets.insert(asset.uuid(), asset.clone());
        }
    }

    // Clear the spinner before the match progress bars take over the terminal.
    if let Some(progress) = scan_progress {
        progress.finish_and_clear();
        eprintln!(
            "Scanned {} folder path(s), found {} asset(s) to match",
            folder_paths.len(),
            all_assets.len()
        );
    }

    trace!("Found {} assets across all folders", all_assets.len());

    if all_assets.is_empty() {
        // The paths themselves resolved - listing returns `FolderNotFound` otherwise -
        // so this is a genuinely empty result, not a typo in the path.
        let subfolders = if recursive {
            0
        } else {
            count_subfolders(api, tenant_uuid, folder_paths).await
        };

        if subfolders > 0 {
            error_utils::report_error_with_remediation(
                &"No assets found directly in the specified folder(s)",
                &[
                    &format!(
                        "The folder(s) contain {} subfolder(s) - pass --recursive to include the assets in them",
                        subfolders
                    ),
                    "Run 'pcli2 folder list --folder-path <path>' to see what the folder contains",
                    "Ensure you have permissions to access the specified folder(s)",
                ],
            );
        } else {
            error_utils::report_error_with_remediation(
                &"No assets found in the specified folder(s)",
                &[
                    "Run 'pcli2 folder list --folder-path <path>' to see what the folder contains",
                    "Check that the assets have finished processing ('pcli2 asset list' shows their state)",
                    "Ensure you have permissions to access the specified folder(s)",
                ],
            );
        }
        return Ok(None);
    }

    Ok(Some(all_assets))
}

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

    // Maximum number of results to return (visual search ranks every asset, so a
    // limit is required to keep the result set manageable).
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied()
        .unwrap_or(100);

    // Get size threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

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
    let mut search_results = ctx
        .api()
        .visual_search(&tenant_uuid, &asset.uuid(), limit, threshold)
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
                        base_headers.push(format!("CAN_{}", key.to_uppercase()));
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

/// The buffered, locked `stdout` handle the CSV outputs stream through.
type CsvStdoutWriter = csv::Writer<std::io::BufWriter<std::io::StdoutLock<'static>>>;

/// Open a CSV writer that streams rows straight to `stdout` as they are produced.
///
/// The alternative - serializing into a `Vec<u8>`, converting that to a `String`,
/// and printing the lot in one go - holds the whole report in memory three times
/// over. On a large match report that is hundreds of megabytes, and the user waits
/// through all of it before a single byte appears. Streaming keeps peak memory flat
/// and starts producing output immediately.
///
/// The tradeoff is that output is no longer atomic: a failure partway through
/// leaves a truncated report on stdout rather than printing nothing at all. The
/// error still goes to stderr and the exit code is still non-zero, and a truncated
/// CSV still parses. See [`stream_json_report`], which makes the same trade on
/// harsher terms.
///
/// The lock is held for the whole write, which is both correct and faster than
/// re-acquiring it per row. A `--progress` display keeps drawing on stderr
/// meanwhile; it never writes to stdout, so redirected output stays clean whatever
/// the terminal ends up looking like.
fn csv_stdout_writer() -> CsvStdoutWriter {
    csv::Writer::from_writer(std::io::BufWriter::new(std::io::stdout().lock()))
}

/// Wrap a failure to write the CSV stream to stdout as a `CliError`.
fn csv_stream_error(error: std::io::Error) -> CliError {
    CliError::from(CliActionError::FormattingError(
        crate::format::FormattingError::CsvWriterError(format!(
            "failed writing CSV to stdout: {}",
            error
        )),
    ))
}

/// Serialize a finished match report as pretty JSON straight to `stdout`.
///
/// Streams for the same reasons the CSV output does: building the document as one
/// `String` first holds the whole report in memory a second time - pretty-printed
/// JSON is bulkier than the rows it came from - and nothing reaches the terminal
/// until the last byte is ready.
///
/// The atomicity trade is harsher here than for CSV. A truncated CSV still parses;
/// a truncated JSON document does not parse at all, so a mid-write failure leaves
/// output that no consumer can read. The error still goes to stderr with a non-zero
/// exit code, which is what distinguishes it from a silently short file.
///
/// Byte-for-byte identical to the previous `println!("{}", to_string_pretty(…)?)`:
/// `to_string_pretty` is the same serializer against a `String` sink, and the
/// trailing newline `println!` added is written explicitly below.
///
/// Errors surface as `io::Error` - `serde_json::Error` converts into one - so this
/// signature stays clear of the large `CliError` enum; callers wrap it with
/// [`json_stream_error`].
fn stream_json_report<T: serde::Serialize>(
    rows: &[T],
    progress: &ReportProgress,
) -> std::io::Result<()> {
    use std::io::Write;

    progress.phase(format!(
        "Writing {} rows as JSON...",
        HumanCount(rows.len() as u64)
    ));
    let mut writer = std::io::BufWriter::new(std::io::stdout().lock());
    serde_json::to_writer_pretty(&mut writer, rows)?;
    writer.write_all(b"\n")?;
    // Explicit: a BufWriter dropped with data still buffered discards the error,
    // which would truncate the document without anyone noticing.
    writer.flush()
}

/// Wrap a failure to write the JSON stream to stdout as a `CliError`.
fn json_stream_error(error: std::io::Error) -> CliError {
    CliError::from(CliActionError::FormattingError(
        crate::format::FormattingError::FormatFailure {
            cause: Box::new(error),
        },
    ))
}

/// The closing summary line printed once a report is complete.
fn report_summary(rows: usize) -> String {
    format!("Built report of {} row(s)", HumanCount(rows as u64))
}

/// Flatten every (reference, candidate) match into the row model the JSON output
/// serializes.
///
/// Each pair clones its reference asset, metadata included, so on a large report this
/// is a slow pass - and it runs after the match progress bar has already finished.
fn flatten_geometric_matches(
    all_matches: Vec<crate::model::EnhancedGeometricSearchResponse>,
    progress: &ReportProgress,
) -> Vec<crate::model::GeometricMatchPair> {
    progress.start_rows("Flattening matches", all_matches.len());
    let mut flattened = Vec::new();
    for (index, enhanced_response) in all_matches.into_iter().enumerate() {
        progress.set_row(index);
        for match_result in enhanced_response.matches {
            flattened.push(crate::model::GeometricMatchPair::from_reference_and_match(
                enhanced_response.reference_asset.clone(),
                match_result,
            ));
        }
    }
    flattened
}

/// Flatten part matches into the row model shared by the JSON and CSV outputs.
///
/// See [`flatten_geometric_matches`] for why this pass is worth reporting.
fn flatten_part_matches(
    all_matches: Vec<crate::model::EnhancedPartSearchResponse>,
    progress: &ReportProgress,
) -> Vec<crate::model::PartMatchPair> {
    progress.start_rows("Flattening matches", all_matches.len());
    let mut flattened = Vec::new();
    for (index, enhanced_response) in all_matches.into_iter().enumerate() {
        progress.set_row(index);
        for match_result in enhanced_response.matches {
            flattened.push(crate::model::PartMatchPair::from_reference_and_match(
                enhanced_response.reference_asset.clone(),
                match_result,
            ));
        }
    }
    flattened
}

/// Flatten visual matches into the row model shared by the JSON and CSV outputs.
///
/// Visual search reuses [`crate::model::EnhancedPartSearchResponse`] as its carrier;
/// only the pair type it flattens into differs from [`flatten_part_matches`]. See
/// [`flatten_geometric_matches`] for why this pass is worth reporting.
fn flatten_visual_matches(
    all_matches: Vec<crate::model::EnhancedPartSearchResponse>,
    progress: &ReportProgress,
) -> Vec<crate::model::VisualMatchPair> {
    progress.start_rows("Flattening matches", all_matches.len());
    let mut flattened = Vec::new();
    for (index, enhanced_response) in all_matches.into_iter().enumerate() {
        progress.set_row(index);
        for match_result in enhanced_response.matches {
            flattened.push(crate::model::VisualMatchPair {
                reference_asset: enhanced_response.reference_asset.clone(),
                candidate_asset: match_result.asset,
                comparison_url: match_result.comparison_url,
            });
        }
    }
    flattened
}

/// Build the canonical match-report table (headers + rows) shared by the CSV and
/// Excel outputs of `folder geometric-match`.
///
/// Producing both formats from this one function guarantees they are always
/// column-for-column identical — only the presentation differs. The columns are,
/// in order: the base columns from [`crate::model::GeometricMatchPair::csv_header`]
/// except `COMPARISON_URL`; then, when `with_metadata` is set, paired
/// `REF_<field>`/`CAN_<field>` metadata columns (the sorted union of metadata keys
/// across all pairs); and finally `COMPARISON_URL` as the last column (its long,
/// rarely-read value is kept out of the way after the metadata).
///
/// Each phase reports to `progress`: on a large report every one of them costs real
/// time, and they all run after the match progress bar has already finished.
fn build_geometric_match_table(
    all_matches: &[crate::model::EnhancedGeometricSearchResponse],
    with_metadata: bool,
    progress: &ReportProgress,
) -> (Vec<String>, Vec<Vec<String>>) {
    // Every (reference, candidate) pair, borrowed rather than materialized.
    //
    // This used to collect a `Vec<GeometricMatchPair>` first, which cloned the whole
    // reference asset - metadata `HashMap` included - once per row, only ever to read
    // a path and a UUID back out of it. On a 1.3M-row report that intermediate cost
    // seconds to build and a further nine seconds to *drop* at the end of this
    // function, with the progress display parked at 99% for all of it. Both passes
    // below iterate the borrowed data instead, so nothing is duplicated and there is
    // nothing to tear down.
    //
    // `GeometricMatchPair::from_reference_and_match` is a plain field mapping, so
    // reading the fields directly here produces byte-identical rows.
    let pairs = || {
        all_matches.iter().flat_map(|response| {
            response
                .matches
                .iter()
                .map(move |m| (&response.reference_asset, m))
        })
    };
    let total_rows: usize = all_matches
        .iter()
        .map(|response| response.matches.len())
        .sum();

    // Collect the sorted, unique metadata keys present across all pairs.
    let mut metadata_keys: Vec<String> = Vec::new();
    if with_metadata {
        progress.start_rows("Collecting metadata columns", total_rows);
        let mut keys = std::collections::HashSet::new();
        for (index, (reference_asset, match_result)) in pairs().enumerate() {
            progress.set_row(index);
            for key in reference_asset.metadata.keys() {
                keys.insert(key.clone());
            }
            for key in match_result.asset.metadata.keys() {
                keys.insert(key.clone());
            }
        }
        let mut sorted: Vec<String> = keys.into_iter().collect();
        sorted.sort();
        metadata_keys = sorted;
    }

    // Headers: base columns, then REF_/CAN_ metadata pairs, and finally
    // COMPARISON_URL. The comparison URL is long and rarely read, so it is moved
    // out of the base columns to the very last column, after the metadata.
    const COMPARISON_URL_HEADER: &str = "COMPARISON_URL";
    let mut headers = crate::model::GeometricMatchPair::csv_header();
    headers.retain(|header| header != COMPARISON_URL_HEADER);
    if with_metadata {
        for key in &metadata_keys {
            headers.push(format!("REF_{}", key.to_uppercase()));
            headers.push(format!("CAN_{}", key.to_uppercase()));
        }
    }
    headers.push(COMPARISON_URL_HEADER.to_string());

    // Rows, in the same column order as the headers (COMPARISON_URL last).
    progress.start_rows("Building rows", total_rows);
    let mut rows = Vec::with_capacity(total_rows);
    for (index, (reference_asset, match_result)) in pairs().enumerate() {
        progress.set_row(index);
        let mut values = vec![
            reference_asset.path.clone(),
            match_result.asset.path.clone(),
            format!("{}", match_result.match_percentage),
            reference_asset.uuid.to_string(),
            match_result.asset.uuid.to_string(),
        ];
        if with_metadata {
            for key in &metadata_keys {
                let ref_value = reference_asset
                    .metadata
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                values.push(ref_value);
                let candidate_value = match_result
                    .asset
                    .metadata
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                values.push(candidate_value);
            }
        }
        // COMPARISON_URL last, matching the header order above.
        values.push(match_result.comparison_url.clone().unwrap_or_default());
        rows.push(values);
    }

    (headers, rows)
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
    let with_headers = format_params.format_options.with_headers;

    // The `xls` (Excel) format is a binary file output, not a stdout format, and
    // is intentionally not part of the `OutputFormat` enum — `FormatParams` would
    // silently fall back to JSON for it. Detect it from the raw format string
    // instead, and handle it separately below. Excel reports always include
    // metadata (the metadata diff is the whole point), so force it on for xls.
    let is_xls = sub_matches
        .get_one::<String>(crate::commands::params::PARAMETER_FORMAT)
        .map(|value| value.eq_ignore_ascii_case(crate::commands::params::FORMAT_XLS))
        .unwrap_or(false);
    let with_metadata = format_params.format_options.with_metadata || is_xls;

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

    let recursive = sub_matches.get_flag(crate::commands::params::PARAMETER_RECURSIVE);

    // Collect all assets from the specified folders, descending into subfolders only
    // when --recursive was requested
    let all_assets = match collect_assets_in_folders(
        &mut api,
        &tenant.uuid,
        &folder_paths,
        recursive,
        show_progress,
    )
    .await?
    {
        Some(assets) => assets,
        None => return Ok(()),
    };

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
    // The matches an asset contributed, plus why it contributed none if it failed -
    // so the caller can tell an asset with no matches from one that was never
    // successfully searched.
    type TaskResult = Result<
        (
            Vec<crate::model::EnhancedGeometricSearchResponse>,
            Option<SearchFailure>,
        ),
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
                                crate::model::path_is_within_folder(
                                    &normalized_candidate_path,
                                    &normalized_folder_path,
                                )
                            });

                        let reference_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_reference_path =
                                    crate::model::normalize_path(asset_clone.path());
                                crate::model::path_is_within_folder(
                                    &normalized_reference_path,
                                    &normalized_folder_path,
                                )
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

                    Ok((asset_matches, None))
                }
                Err(e) => {
                    let failure = SearchFailure::classify(&e);
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform geometric search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    // The asset contributes no matches either way; the classification
                    // is what lets the caller tell "nothing to find" from "could not
                    // look" once every task has been collected.
                    Ok((Vec::new(), Some(failure)))
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
    let mut outcomes = SearchOutcomes::default();
    for task in tasks {
        match task.await {
            Ok(Ok((asset_matches, failure))) => {
                outcomes.record(failure);
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

    // Account for the searches that failed before presenting a report built from the
    // ones that did not. Runs here rather than at the end so a materially incomplete
    // run stops before spending minutes building a report nobody should trust.
    finish_search_outcomes(&outcomes)?;

    // Everything from here on is CPU- and memory-bound rather than network-bound, and
    // on a large result set it runs for minutes after the match bar has already
    // finished. Report it so the command does not look wedged.
    let report_progress = ReportProgress::new(
        show_progress,
        &format!(
            "Building report from {} matches...",
            HumanCount(all_matches.len() as u64)
        ),
    );

    // Excel (`xls`) output: write a styled .xlsx workbook to a file instead of
    // printing. Handled here because `xls` is not an `OutputFormat` enum variant.
    // The workbook is built from the same table as the CSV output, so the two
    // formats are always column-for-column consistent.
    if is_xls {
        // Refuse an oversized workbook before building anything. The row count is just
        // the number of (reference, candidate) pairs, which is known the moment
        // matching finishes - so a report too tall for a worksheet fails here in
        // milliseconds rather than after minutes of row building and cell formatting.
        crate::xlsx_report::ensure_rows_fit(
            all_matches
                .iter()
                .map(|response| response.matches.len())
                .sum(),
        )?;

        let (headers, rows) =
            build_geometric_match_table(&all_matches, with_metadata, &report_progress);
        let row_count = rows.len();
        let requested_path = sub_matches
            .get_one::<std::path::PathBuf>(crate::commands::params::PARAMETER_OUTPUT)
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("match_report.xlsx"));
        let output_path = crate::xlsx_report::normalize_output_path(&requested_path);
        // Warn if we had to coerce the extension to `.xlsx`. Tracing output
        // goes to stderr, so stdout stays clean per UNIX convention.
        if output_path != requested_path {
            crate::error_utils::report_warning(&format!(
                "Output file extension changed to '.xlsx': writing '{}' instead of '{}'",
                output_path.display(),
                requested_path.display()
            ));
        }
        crate::xlsx_report::write_match_report(headers, rows, &output_path, &report_progress)?;
        report_progress.finish_with_summary(&format!(
            "Wrote {} row(s) to {}",
            HumanCount(row_count as u64),
            output_path.display()
        ));
        // UNIX-style: on success there is no data to print, so print nothing.
        return Ok(());
    }

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let flattened_matches = flatten_geometric_matches(all_matches, &report_progress);
            stream_json_report(&flattened_matches, &report_progress).map_err(json_stream_error)?;
            report_progress.finish_with_summary(&report_summary(flattened_matches.len()));
        }
        crate::format::OutputFormat::Csv(_) => {
            // Build the shared table so CSV and Excel stay column-for-column
            // identical; only the presentation differs between the two formats.
            let (headers, rows) =
                build_geometric_match_table(&all_matches, with_metadata, &report_progress);

            let mut wtr = csv_stdout_writer();

            if with_headers {
                if let Err(e) = wtr.serialize(headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            report_progress.start_rows("Writing CSV", rows.len());
            for (index, row) in rows.iter().enumerate() {
                report_progress.set_row(index);
                if let Err(e) = wtr.serialize(row.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            wtr.flush().map_err(csv_stream_error)?;
            report_progress.finish_with_summary(&report_summary(rows.len()));
        }
        _ => {
            // Default to JSON
            let flattened_matches = flatten_geometric_matches(all_matches, &report_progress);
            stream_json_report(&flattened_matches, &report_progress).map_err(json_stream_error)?;
            report_progress.finish_with_summary(&report_summary(flattened_matches.len()));
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

    let recursive = sub_matches.get_flag(crate::commands::params::PARAMETER_RECURSIVE);

    // Collect all assets from the specified folders, descending into subfolders only
    // when --recursive was requested
    let all_assets = match collect_assets_in_folders(
        &mut api,
        &tenant.uuid,
        &folder_paths,
        recursive,
        show_progress,
    )
    .await?
    {
        Some(assets) => assets,
        None => return Ok(()),
    };

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
    // The matches an asset contributed, plus why it contributed none if it failed -
    // so the caller can tell an asset with no matches from one that was never
    // successfully searched.
    type TaskResult = Result<
        (
            Vec<crate::model::EnhancedPartSearchResponse>,
            Option<SearchFailure>,
        ),
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
                                crate::model::path_is_within_folder(
                                    &normalized_candidate_path,
                                    &normalized_folder_path,
                                )
                            });

                        let reference_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_reference_path =
                                    crate::model::normalize_path(asset_clone.path());
                                crate::model::path_is_within_folder(
                                    &normalized_reference_path,
                                    &normalized_folder_path,
                                )
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

                    Ok((asset_matches, None))
                }
                Err(e) => {
                    let failure = SearchFailure::classify(&e);
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform part search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    // The asset contributes no matches either way; the
                    // classification is what lets the caller tell "nothing to
                    // find" from "could not look" once tasks are collected.
                    Ok((Vec::new(), Some(failure)))
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
    let mut outcomes = SearchOutcomes::default();
    for task in tasks {
        match task.await {
            Ok(Ok((asset_matches, failure))) => {
                outcomes.record(failure);
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

    // Account for the searches that failed before presenting a report built from the
    // ones that did not. Runs here rather than at the end so a materially incomplete
    // run stops before spending minutes building a report nobody should trust.
    finish_search_outcomes(&outcomes)?;

    // Everything from here on is CPU- and memory-bound rather than network-bound, and
    // on a large result set it runs for minutes after the match bar has already
    // finished. Report it so the command does not look wedged.
    let report_progress = ReportProgress::new(
        show_progress,
        &format!(
            "Building report from {} matches...",
            HumanCount(all_matches.len() as u64)
        ),
    );

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let flattened_matches = flatten_part_matches(all_matches, &report_progress);
            stream_json_report(&flattened_matches, &report_progress).map_err(json_stream_error)?;
            report_progress.finish_with_summary(&report_summary(flattened_matches.len()));
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let flattened_matches = flatten_part_matches(all_matches, &report_progress);

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv_stdout_writer();

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                report_progress.start_rows("Collecting metadata columns", flattened_matches.len());
                let mut all_metadata_keys = std::collections::HashSet::new();
                for (index, match_pair) in flattened_matches.iter().enumerate() {
                    report_progress.set_row(index);
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
                        base_headers.push(format!("CAN_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            let total_rows = flattened_matches.len();
            report_progress.start_rows("Writing CSV", total_rows);
            for (index, match_pair) in flattened_matches.into_iter().enumerate() {
                report_progress.set_row(index);
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

            wtr.flush().map_err(csv_stream_error)?;
            report_progress.finish_with_summary(&report_summary(total_rows));
        }
        _ => {
            // Default to JSON
            let flattened_matches = flatten_part_matches(all_matches, &report_progress);
            stream_json_report(&flattened_matches, &report_progress).map_err(json_stream_error)?;
            report_progress.finish_with_summary(&report_summary(flattened_matches.len()));
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

    // Maximum number of visual-search results to return per asset.
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied()
        .unwrap_or(100);

    // Get size threshold parameter
    let threshold = sub_matches
        .get_one::<f64>("threshold")
        .copied()
        .unwrap_or(80.0);

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

    let recursive = sub_matches.get_flag(crate::commands::params::PARAMETER_RECURSIVE);

    // Collect all assets from the specified folders, descending into subfolders only
    // when --recursive was requested
    let all_assets = match collect_assets_in_folders(
        &mut api,
        &tenant.uuid,
        &folder_paths,
        recursive,
        show_progress,
    )
    .await?
    {
        Some(assets) => assets,
        None => return Ok(()),
    };

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
    // The matches an asset contributed, plus why it contributed none if it failed -
    // so the caller can tell an asset with no matches from one that was never
    // successfully searched.
    type TaskResult = Result<
        (
            Vec<crate::model::EnhancedPartSearchResponse>,
            Option<SearchFailure>,
        ),
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

            let result = match api_clone
                .visual_search(&tenant_uuid, &asset_uuid, limit, threshold)
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
                                crate::model::path_is_within_folder(
                                    &normalized_candidate_path,
                                    &normalized_folder_path,
                                )
                            });

                        let reference_in_specified_folders =
                            folder_paths_clone.iter().any(|folder_path| {
                                let normalized_folder_path =
                                    crate::model::normalize_path(folder_path);
                                let normalized_reference_path =
                                    crate::model::normalize_path(asset_clone.path());
                                crate::model::path_is_within_folder(
                                    &normalized_reference_path,
                                    &normalized_folder_path,
                                )
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

                    Ok((asset_matches, None))
                }
                Err(e) => {
                    let failure = SearchFailure::classify(&e);
                    error_utils::report_warning(&format!(
                        "🔍 Failed to perform visual search for asset {}: {}",
                        asset_clone.name(),
                        e
                    ));
                    if let Some(ref pb) = individual_pb {
                        pb.set_message("Failed");
                    }
                    // The asset contributes no matches either way; the
                    // classification is what lets the caller tell "nothing to
                    // find" from "could not look" once tasks are collected.
                    Ok((Vec::new(), Some(failure)))
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
    let mut outcomes = SearchOutcomes::default();
    for task in tasks {
        match task.await {
            Ok(Ok((asset_matches, failure))) => {
                outcomes.record(failure);
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

    // Account for the searches that failed before presenting a report built from the
    // ones that did not. Runs here rather than at the end so a materially incomplete
    // run stops before spending minutes building a report nobody should trust.
    finish_search_outcomes(&outcomes)?;

    // Everything from here on is CPU- and memory-bound rather than network-bound, and
    // on a large result set it runs for minutes after the match bar has already
    // finished. Report it so the command does not look wedged.
    let report_progress = ReportProgress::new(
        show_progress,
        &format!(
            "Building report from {} matches...",
            HumanCount(all_matches.len() as u64)
        ),
    );

    // Output the results based on format
    match format {
        crate::format::OutputFormat::Json(_) => {
            // For JSON, we need to flatten all matches into a single array
            let flattened_matches = flatten_visual_matches(all_matches, &report_progress);
            stream_json_report(&flattened_matches, &report_progress).map_err(json_stream_error)?;
            report_progress.finish_with_summary(&report_summary(flattened_matches.len()));
        }
        crate::format::OutputFormat::Csv(_) => {
            // For CSV, we can output all matches together
            let flattened_matches = flatten_visual_matches(all_matches, &report_progress);

            // For CSV with metadata, we need to create a custom implementation
            let mut wtr = csv_stdout_writer();

            // Pre-calculate the metadata keys that will be used for headers and all records
            let mut header_metadata_keys = Vec::new();
            if with_metadata {
                // Collect all unique metadata keys from ALL match pairs for consistent headers
                report_progress.start_rows("Collecting metadata columns", flattened_matches.len());
                let mut all_metadata_keys = std::collections::HashSet::new();
                for (index, match_pair) in flattened_matches.iter().enumerate() {
                    report_progress.set_row(index);
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
                        base_headers.push(format!("CAN_{}", key.to_uppercase()));
                    }
                }

                if let Err(e) = wtr.serialize(base_headers.as_slice()) {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::CsvError(e),
                    )));
                }
            }

            let total_rows = flattened_matches.len();
            report_progress.start_rows("Writing CSV", total_rows);
            for (index, match_pair) in flattened_matches.into_iter().enumerate() {
                report_progress.set_row(index);
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

            wtr.flush().map_err(csv_stream_error)?;
            report_progress.finish_with_summary(&report_summary(total_rows));
        }
        _ => {
            // Default to JSON
            let flattened_matches = flatten_visual_matches(all_matches, &report_progress);
            stream_json_report(&flattened_matches, &report_progress).map_err(json_stream_error)?;
            report_progress.finish_with_summary(&report_summary(flattened_matches.len()));
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

    // Maximum number of results to return; the search paginates until the
    // limit is reached or all matches are collected.
    let limit = sub_matches
        .get_one::<usize>(crate::commands::params::PARAMETER_LIMIT)
        .copied()
        .unwrap_or(100);

    // Extract tenant info before calling text search
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // Perform text search
    let (mut search_results, truncated) = ctx
        .api()
        .text_search(&tenant_uuid, &search_query, limit)
        .await?;

    // Warn on stderr (so piped stdout data is unaffected) when the limit
    // truncated the result set.
    if truncated {
        eprintln!(
            "⚠️  Result limit reached: showing the first {} matching assets; increase --limit to retrieve more",
            search_results.matches.len()
        );
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::physna_v3::ApiError;

    #[test]
    fn conflicts_are_data_state_everything_else_is_operational() {
        // The search endpoint reports an unsearchable asset as a 409. Those are a
        // property of the tenant, not of the run, and must not fail the command.
        for message in [
            "HTTP 409 Conflict - Asset not indexed yet",
            "HTTP 409 Conflict - Asset has no 3D data and is unavailable for search",
            "HTTP 409 Conflict - Asset failed to index",
        ] {
            assert_eq!(
                SearchFailure::classify(&ApiError::ConflictError(message.to_string())),
                SearchFailure::NotSearchable
            );
        }

        // Everything else means the run could not do its job. Auth expiry is the case
        // that motivated this: it failed 21,068 searches while the command exited 0.
        assert_eq!(
            SearchFailure::classify(&ApiError::AuthError("token expired".to_string())),
            SearchFailure::Operational
        );
        assert_eq!(
            SearchFailure::classify(&ApiError::InvalidToken),
            SearchFailure::Operational
        );
        assert_eq!(
            SearchFailure::classify(&ApiError::RetryFailed("503".to_string())),
            SearchFailure::Operational
        );
    }

    fn outcomes(attempted: usize, not_searchable: usize, operational: usize) -> SearchOutcomes {
        let mut o = SearchOutcomes::default();
        for i in 0..attempted {
            o.record(if i < operational {
                Some(SearchFailure::Operational)
            } else if i < operational + not_searchable {
                Some(SearchFailure::NotSearchable)
            } else {
                None
            });
        }
        o
    }

    #[test]
    fn a_clean_run_says_nothing() {
        let o = outcomes(100, 0, 0);
        assert_eq!(o.succeeded(), 100);
        assert!(
            o.summary().is_none(),
            "no failures means no summary to print"
        );
        assert!(!o.is_materially_incomplete());
    }

    #[test]
    fn unsearchable_assets_alone_never_fail_the_run() {
        // The real baseline: 806 of 22,378 assets are not indexed or have no 3D data.
        // That report is complete, and the command must still exit 0 - but the count
        // is still reported so it is never invisible.
        let o = outcomes(22_378, 806, 0);
        assert!(!o.is_materially_incomplete());
        let summary = o.summary().expect("failures are always reported");
        assert!(summary.contains("806 not searchable"), "{}", summary);
        assert!(!summary.contains("failed"), "{}", summary);
    }

    #[test]
    fn a_systemic_failure_fails_the_run() {
        // The case this exists for: the token expired mid-run and 21,068 of 22,378
        // searches failed, while the command exited 0 with a report missing 94% of
        // its rows.
        let o = outcomes(22_378, 51, 21_068);
        assert!(o.is_materially_incomplete());
        assert!(finish_search_outcomes(&o).is_err());
        let summary = o.summary().expect("failures are always reported");
        assert!(summary.contains("21,068 failed"), "{}", summary);
        assert!(summary.contains("51 not searchable"), "{}", summary);
    }

    #[test]
    fn a_few_operational_failures_are_reported_but_do_not_fail_the_run() {
        // Two stale-token retries out of 22,378 was the healthy baseline. Failing the
        // whole command over that would teach people to ignore the exit code.
        let o = outcomes(22_378, 806, 2);
        assert!(!o.is_materially_incomplete());
        assert!(finish_search_outcomes(&o).is_ok());
        assert!(o.summary().expect("reported").contains("2 failed"));
    }

    #[test]
    fn the_threshold_boundary_is_exclusive() {
        // Exactly at the threshold is tolerated; above it is not.
        let at = outcomes(1000, 0, 100);
        assert!(!at.is_materially_incomplete(), "10% exactly must not fail");
        let over = outcomes(1000, 0, 101);
        assert!(over.is_materially_incomplete());
    }

    #[test]
    fn an_empty_run_is_not_incomplete() {
        // Guards the division: no assets attempted must not be a divide-by-zero or a
        // spurious failure.
        let o = outcomes(0, 0, 0);
        assert!(!o.is_materially_incomplete());
        assert!(o.summary().is_none());
    }

    fn asset(path: &str, uuid: &str, metadata: &[(&str, &str)]) -> crate::model::AssetResponse {
        crate::model::AssetResponse {
            uuid: Uuid::parse_str(uuid).expect("valid uuid"),
            tenant_id: Uuid::nil(),
            path: path.to_string(),
            folder_id: None,
            asset_type: "asset".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            state: "active".to_string(),
            is_assembly: false,
            metadata: metadata
                .iter()
                .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
                .collect::<HashMap<_, _>>(),
            parent_folder_id: None,
            owner_id: None,
        }
    }

    fn geometric_match(
        candidate: crate::model::AssetResponse,
        percentage: f64,
        url: Option<&str>,
    ) -> crate::model::GeometricMatch {
        crate::model::GeometricMatch {
            asset: candidate,
            match_percentage: percentage,
            transformation: None,
            comparison_url: url.map(|u| u.to_string()),
        }
    }

    const REF_UUID: &str = "9ebd8801-388a-4fb8-a4fd-dd77d91e7cac";
    const CAN_A_UUID: &str = "02c37ef8-caad-4a16-a992-6abbdda852f9";
    const CAN_B_UUID: &str = "f16e93ca-bc0b-41c1-a98f-d1eeef01e4c1";

    /// One response carrying two matches, to prove rows are produced per *match*
    /// rather than per response.
    fn sample() -> Vec<crate::model::EnhancedGeometricSearchResponse> {
        vec![crate::model::EnhancedGeometricSearchResponse {
            reference_asset: asset("/a/ref.prt", REF_UUID, &[("material", "steel")]),
            matches: vec![
                geometric_match(
                    asset("/b/one.prt", CAN_A_UUID, &[("material", "alu")]),
                    100.0,
                    Some("https://example.com/compare?a=1"),
                ),
                geometric_match(
                    asset("/c/two.prt", CAN_B_UUID, &[("finish", "anodized")]),
                    81.5,
                    None,
                ),
            ],
        }]
    }

    #[test]
    fn table_without_metadata_has_base_columns_and_url_last() {
        let (headers, rows) =
            build_geometric_match_table(&sample(), false, &ReportProgress::disabled());

        assert_eq!(headers.last().map(String::as_str), Some("COMPARISON_URL"));
        assert!(!headers.iter().any(|h| h.starts_with("REF_")));

        assert_eq!(rows.len(), 2, "one row per match, not per response");
        assert_eq!(
            rows[0],
            vec![
                "/a/ref.prt".to_string(),
                "/b/one.prt".to_string(),
                "100".to_string(),
                REF_UUID.to_string(),
                CAN_A_UUID.to_string(),
                "https://example.com/compare?a=1".to_string(),
            ]
        );
        // A missing comparison URL becomes an empty cell, not a dropped column.
        assert_eq!(rows[1].len(), headers.len());
        assert_eq!(rows[1][2], "81.5");
        assert_eq!(rows[1].last().map(String::as_str), Some(""));
    }

    #[test]
    fn table_with_metadata_pairs_the_sorted_key_union() {
        let (headers, rows) =
            build_geometric_match_table(&sample(), true, &ReportProgress::disabled());

        // Keys are the sorted union across both sides of every pair: finish, material.
        let metadata_headers: Vec<&str> = headers
            .iter()
            .filter(|h| h.starts_with("REF_") || h.starts_with("CAN_"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            metadata_headers,
            vec!["REF_FINISH", "CAN_FINISH", "REF_MATERIAL", "CAN_MATERIAL"]
        );
        assert_eq!(headers.last().map(String::as_str), Some("COMPARISON_URL"));

        // Row 0: reference has material=steel and no finish; candidate has
        // material=alu. Absent values are empty strings, keeping columns aligned.
        let finish_ref = headers.iter().position(|h| h == "REF_FINISH").unwrap();
        let material_ref = headers.iter().position(|h| h == "REF_MATERIAL").unwrap();
        let material_can = headers.iter().position(|h| h == "CAN_MATERIAL").unwrap();
        assert_eq!(rows[0][finish_ref], "");
        assert_eq!(rows[0][material_ref], "steel");
        assert_eq!(rows[0][material_can], "alu");

        for row in &rows {
            assert_eq!(
                row.len(),
                headers.len(),
                "every row matches the header width"
            );
        }
    }

    #[test]
    fn empty_input_produces_headers_but_no_rows() {
        let (headers, rows) = build_geometric_match_table(&[], false, &ReportProgress::disabled());
        assert!(!headers.is_empty());
        assert!(rows.is_empty());
    }
}

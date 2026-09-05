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

/// Record one symmetric (reference, candidate) pair into a deduplicating map.
///
/// `(A,B)` and `(B,A)` are the same pair and share a key, so a pair searched from both
/// ends is stored once. Which of the two records survives must not depend on which
/// concurrent search finished first, or two identical runs produce different reports -
/// so when both directions are seen, the one whose reference sorts first wins.
///
/// A record is only ever stored as it was actually searched. The transformation matrix
/// and the comparison URL describe one direction, so rewriting a record to make it
/// canonical would quietly corrupt both; a pair searched in only one direction keeps
/// whichever direction that was.
///
/// The stored flag is that orientation, kept alongside the record so this decision
/// needs no knowledge of the record's shape.
fn record_unique_pair<T>(
    deduped: &mut std::collections::BTreeMap<(Uuid, Uuid), (bool, T)>,
    reference_uuid: Uuid,
    candidate_uuid: Uuid,
    record: T,
) {
    let is_canonical = reference_uuid < candidate_uuid;
    let key = if is_canonical {
        (reference_uuid, candidate_uuid)
    } else {
        (candidate_uuid, reference_uuid)
    };

    match deduped.entry(key) {
        std::collections::btree_map::Entry::Vacant(slot) => {
            slot.insert((is_canonical, record));
        }
        std::collections::btree_map::Entry::Occupied(mut slot) => {
            if is_canonical && !slot.get().0 {
                slot.insert((is_canonical, record));
            }
        }
    }
}

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
    /// The search was never attempted, because an earlier failure made it certain
    /// that every remaining one would fail the same way. See [`SearchAbort`].
    Aborted,
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

/// How many authentication failures in a row end the run.
///
/// Not one. The API client renews the token automatically on a 401 and retries, which
/// is what lets a long run survive its credentials expiring halfway through - and
/// `refresh_token` reports *every* cause of a failed renewal as the same error, so a
/// rejected credential is indistinguishable from a 5xx or a rate limit at the auth
/// endpoint. Stopping on the first one would abandon a 25-minute run over a hiccup
/// the very next request would have recovered from.
///
/// Not unbounded either: a genuinely dead credential fails every asset, and issuing
/// twenty thousand doomed requests only delays the user finding out.
///
/// Three consecutive, with any success resetting the count. A transient failure is
/// absorbed because successes keep interleaving; a systemic one has no successes to
/// reset it and trips almost immediately.
const CONSECUTIVE_AUTH_FAILURES_BEFORE_STOP: usize = 3;

/// Shared stop signal for a folder match run.
///
/// Every per-asset task is spawned up front and gated by a semaphore, so there is no
/// dispatch loop to break out of. Instead each task checks this before doing any
/// work: once the run has given up, the rest return without touching the network.
///
/// Also carries the "already reported" latch, so the reason is printed once rather
/// than once per remaining asset - 21,068 copies of the same line is not diagnostics,
/// it is noise that buries the failures worth reading.
#[derive(Debug, Default)]
struct SearchAbort {
    consecutive_auth_failures: std::sync::atomic::AtomicUsize,
    stopped: std::sync::atomic::AtomicBool,
}

impl SearchAbort {
    fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Note that a search succeeded, so renewal is evidently working and any earlier
    /// failures were transient.
    fn record_success(&self) {
        self.consecutive_auth_failures
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// Note an authentication failure.
    ///
    /// Returns true only for the caller whose failure crosses the threshold, which is
    /// the one that should explain why the run is stopping.
    fn record_auth_failure(&self) -> bool {
        let consecutive = self
            .consecutive_auth_failures
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        if consecutive < CONSECUTIVE_AUTH_FAILURES_BEFORE_STOP {
            return false;
        }
        !self.stopped.swap(true, std::sync::atomic::Ordering::SeqCst)
    }
}

/// Run `f` with the progress display lifted, so what it prints is not painted
/// over by the next redraw. Without a display it just runs.
fn under_progress<F: FnOnce()>(progress: Option<&indicatif::MultiProgress>, f: F) {
    match progress {
        Some(mp) => mp.suspend(f),
        None => f(),
    }
}

/// The UUIDs of the folders named on the command line, for the server-side
/// `folderIds` search filter. The tenant root has no UUID and means "no filter".
async fn resolve_exclusive_folder_ids(
    api: &mut PhysnaApiClient,
    tenant: &crate::model::Tenant,
    folder_paths: &[String],
) -> Result<Vec<Uuid>, CliError> {
    let mut ids = Vec::with_capacity(folder_paths.len());
    for path in folder_paths {
        if crate::model::normalize_path(path) == "/" {
            return Ok(Vec::new());
        }
        ids.push(crate::actions::folders::resolve_folder_uuid_by_path(api, tenant, path).await?);
    }
    Ok(ids)
}

/// Running tally of per-asset search outcomes across a folder match run.
#[derive(Debug, Default, Clone, Copy)]
struct SearchOutcomes {
    attempted: usize,
    not_searchable: usize,
    operational: usize,
    aborted: usize,
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
            Some(SearchFailure::Aborted) => self.aborted += 1,
            None => {}
        }
    }

    fn failed(&self) -> usize {
        self.not_searchable + self.operational + self.aborted
    }

    fn succeeded(&self) -> usize {
        self.attempted.saturating_sub(self.failed())
    }

    /// Searches the run should have completed but did not, for reasons about the run
    /// rather than the data. An aborted search counts here: it was skipped precisely
    /// because the run had already broken.
    fn incomplete(&self) -> usize {
        self.operational + self.aborted
    }

    /// Whether the shortfall is severe enough that the report should not be presented
    /// as a successful result.
    fn is_materially_incomplete(&self) -> bool {
        // An abort is a decision that the run is broken, not a scattered blip: a
        // credential that dies at 92% still leaves 8% of the report missing on
        // purpose, and that must not exit 0 just because it is under the threshold.
        self.aborted > 0
            || (self.attempted > 0
                && (self.incomplete() as f64 / self.attempted as f64)
                    > OPERATIONAL_FAILURE_EXIT_THRESHOLD)
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
        if self.aborted > 0 {
            reasons.push(format!("{} not attempted", HumanCount(self.aborted as u64)));
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
        // Reported above with its remediation steps; main only needs the exit code.
        // (`IncompleteReport` carried different numbers and was printed a second
        // time on the way out.)
        return Err(CliError::AlreadyReported(
            crate::exit_codes::PcliExitCode::TempFail,
        ));
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
            let result = api
                .list_assets_by_parent_folder_path_recursive(
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
                .await;

            // Clear the spinner before any error reaches the terminal. It redraws
            // every 100ms, so a message written underneath it is wiped on the next
            // tick - which is exactly how the old silent fallback stayed invisible.
            if result.is_err() {
                if let Some(progress) = scan_progress.as_ref() {
                    progress.finish_and_clear();
                }
            }
            result?
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
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);

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
        .geometric_search(&tenant_uuid, &asset.uuid(), threshold, &[])
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
        folder_id: None,
        asset_type: asset.file_type().cloned().unwrap_or_default(),
        created_at: asset.created_at().cloned().unwrap_or_default(),
        updated_at: asset.updated_at().cloned().unwrap_or_default(),
        state: asset.normalized_processing_status(),
        is_assembly: asset.is_assembly(),
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,         // No owner ID
    };

    // Create enhanced response that includes the reference asset information
    let enhanced_response = crate::model::EnhancedGeometricSearchResponse {
        reference_asset: reference_asset_response,
        matches: search_results.matches,
    };

    crate::format::print_output(&enhanced_response.format(format)?);

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
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);

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
        .part_search(&tenant_uuid, &asset.uuid(), threshold, &[])
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
        folder_id: None,
        asset_type: asset.file_type().cloned().unwrap_or_default(),
        created_at: asset.created_at().cloned().unwrap_or_default(),
        updated_at: asset.updated_at().cloned().unwrap_or_default(),
        state: asset.normalized_processing_status(),
        is_assembly: asset.is_assembly(),
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,         // No owner ID
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
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);

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
        .visual_search(&tenant_uuid, &asset.uuid(), limit, threshold, &[])
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
        folder_id: None,
        asset_type: asset.file_type().cloned().unwrap_or_default(),
        created_at: asset.created_at().cloned().unwrap_or_default(),
        updated_at: asset.updated_at().cloned().unwrap_or_default(),
        state: asset.normalized_processing_status(),
        is_assembly: asset.is_assembly(),
        metadata: metadata_map, // Include the asset's metadata
        parent_folder_id: None, // No parent folder ID
        owner_id: None,         // No owner ID
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
                            .map(crate::model::metadata_cell)
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .map(crate::model::metadata_cell)
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
                        crate::format::FormattingError::CsvIntoInnerError(Box::new(e)),
                    )));
                }
            };
            let output = match crate::format::csv_text(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };

            crate::format::print_output(&output);
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
                    .map(crate::model::metadata_cell)
                    .unwrap_or_default();
                values.push(ref_value);
                let candidate_value = match_result
                    .asset
                    .metadata
                    .get(key)
                    .map(crate::model::metadata_cell)
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
    // Read once here; it used to be loaded from disk again for every match row.
    let ui_base_url = configuration.get_ui_base_url();
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = crate::actions::utils::split_list_values(
        sub_matches
            .get_many::<String>(PARAMETER_FOLDER_PATH)
            .ok_or(CliError::MissingRequiredArgument(
                PARAMETER_FOLDER_PATH.to_string(),
            ))?
            .map(|s| s.to_string())
            .collect::<Vec<String>>(),
    );

    // Get threshold parameter
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);

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
    // With --exclusive the server pre-filters to these folders and their subfolders,
    // so tenant-wide result pages are no longer downloaded only to be discarded.
    // The client-side path check on each match still decides the exact set.
    let exclusive_folder_ids: Vec<Uuid> = if exclusive {
        resolve_exclusive_folder_ids(&mut api, &tenant, &folder_paths).await?
    } else {
        Vec::new()
    };

    // With --checkpoint, results recorded by an interrupted run are reused and
    // only the remaining assets are searched. Opened before the folder scan so a
    // file from a different run is refused before minutes are spent scanning.
    let checkpoint_path = sub_matches
        .get_one::<std::path::PathBuf>(crate::commands::params::PARAMETER_CHECKPOINT)
        .cloned();
    let (checkpoint, mut recorded_matches) = match &checkpoint_path {
        Some(path) => {
            let fingerprint = crate::checkpoint::Fingerprint::new(
                "geometric",
                tenant.uuid,
                &folder_paths,
                recursive,
                exclusive,
                threshold,
                None,
            );
            let (checkpoint, done) = crate::checkpoint::Checkpoint::<
                crate::model::EnhancedGeometricSearchResponse,
            >::open(path, fingerprint)?;
            (Some(std::sync::Arc::new(checkpoint)), done)
        }
        None => (None, std::collections::HashMap::new()),
    };

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
        None => {
            // Nothing to search, so nothing to resume.
            if let Some(cp) = &checkpoint {
                cp.finish();
            }
            return Ok(());
        }
    };

    if let Some(cp) = &checkpoint {
        let reusable = all_assets
            .keys()
            .filter(|uuid| recorded_matches.contains_key(uuid))
            .count();
        if reusable > 0 {
            eprintln!(
                "Resuming from checkpoint '{}': {} of {} asset(s) already searched",
                cp.path().display(),
                reusable,
                all_assets.len()
            );
        }
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
    // Shared stop signal, so one terminal failure does not have to be rediscovered
    // by every remaining asset.
    let abort = std::sync::Arc::new(SearchAbort::default());

    // Prepare for concurrent processing.
    //
    // Symmetric pairs are deduplicated into a BTreeMap keyed on the *unordered* pair,
    // so neither the surviving row nor the output order depends on which concurrent
    // search happened to finish first. A HashSet plus a Vec made both depend on it:
    // two identical runs produced byte-different reports.
    let mut deduped = std::collections::BTreeMap::new();

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
        let abort = abort.clone();
        let ui_base_url_for_task = ui_base_url.clone();
        let exclusive_folder_ids = exclusive_folder_ids.clone();
        let checkpoint = checkpoint.clone();
        let recorded = recorded_matches.remove(&asset_uuid);

        let task = tokio::spawn(async move {
            // Already searched by the run this one resumes.
            if let Some(matches) = recorded {
                return Ok((matches, None));
            }

            let _permit = semaphore.acquire().await.unwrap();

            // An earlier task hit something that makes every remaining search
            // pointless. Return without touching the network.
            if abort.is_stopped() {
                return Ok((Vec::new(), Some(SearchFailure::Aborted)));
            }

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
                .geometric_search(&tenant_uuid, &asset_uuid, threshold, &exclusive_folder_ids)
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

                        let ui_base_url = ui_base_url_for_task.clone();

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
                            asset_type: asset_clone.file_type().cloned().unwrap_or_default(),
                            created_at: asset_clone.created_at().cloned().unwrap_or_default(),
                            updated_at: asset_clone.updated_at().cloned().unwrap_or_default(),
                            state: asset_clone.normalized_processing_status(),
                            is_assembly: asset_clone.is_assembly(),
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

                    // Renewal is evidently working; forget any earlier blip.
                    abort.record_success();
                    if let Some(cp) = &checkpoint {
                        cp.record(asset_uuid, &asset_matches);
                    }
                    Ok((asset_matches, None))
                }
                Err(e) => {
                    let failure = SearchFailure::classify(&e);
                    // Authentication failures are counted rather than acted on
                    // immediately: the client renews the token and retries by itself,
                    // and one failed renewal may be nothing more than a blip at the
                    // auth endpoint. Only an unbroken run of them means the credentials
                    // are genuinely gone - then the run stops, and explains once.
                    let stopping = e.is_authentication_failure() && abort.record_auth_failure();
                    if stopping {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_error_with_remediation(
                            &format!(
                                "Stopping after {} consecutive authentication failures: {}. Remaining assets were not searched.",
                                CONSECUTIVE_AUTH_FAILURES_BEFORE_STOP, e
                            ),
                            &[
                                "Log in again with 'pcli2 auth login'",
                                "Then re-run this command",
                            ],
                        );
                        });
                    } else if !abort.is_stopped() {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_warning(&format!(
                                "🔍 Failed to perform geometric search for asset {}: {}",
                                asset_clone.name(),
                                e
                            ))
                        });
                    }
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
                    // Apply duplicate filtering to each match. (A,B) and (B,A) are the
                    // same pair, so they share a key.
                    for match_result in &enhanced_match.matches {
                        record_unique_pair(
                            &mut deduped,
                            enhanced_match.reference_asset.uuid,
                            match_result.asset.uuid,
                            enhanced_match.clone(),
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                outcomes.record(Some(SearchFailure::Operational));
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
                outcomes.record(Some(SearchFailure::Operational));
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

    // BTreeMap iteration is ordered by the unordered pair key, so row order is a
    // property of the data rather than of this run's scheduling. Two runs over
    // unchanged data now produce identical output.
    let all_matches: Vec<_> = deduped.into_values().map(|(_, record)| record).collect();

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
        if let Some(cp) = &checkpoint {
            cp.finish();
        }
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

    // The report is written; there is nothing left to resume.
    if let Some(cp) = &checkpoint {
        cp.finish();
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
    // Read once here; it used to be loaded from disk again for every match row.
    let ui_base_url = configuration.get_ui_base_url();
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = crate::actions::utils::split_list_values(
        sub_matches
            .get_many::<String>(PARAMETER_FOLDER_PATH)
            .ok_or(CliError::MissingRequiredArgument(
                PARAMETER_FOLDER_PATH.to_string(),
            ))?
            .cloned()
            .collect::<Vec<String>>(),
    );

    // Get threshold parameter
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);

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
    // With --exclusive the server pre-filters to these folders and their subfolders,
    // so tenant-wide result pages are no longer downloaded only to be discarded.
    // The client-side path check on each match still decides the exact set.
    let exclusive_folder_ids: Vec<Uuid> = if exclusive {
        resolve_exclusive_folder_ids(&mut api, &tenant, &folder_paths).await?
    } else {
        Vec::new()
    };

    // With --checkpoint, results recorded by an interrupted run are reused and
    // only the remaining assets are searched. Opened before the folder scan so a
    // file from a different run is refused before minutes are spent scanning.
    let checkpoint_path = sub_matches
        .get_one::<std::path::PathBuf>(crate::commands::params::PARAMETER_CHECKPOINT)
        .cloned();
    let (checkpoint, mut recorded_matches) = match &checkpoint_path {
        Some(path) => {
            let fingerprint = crate::checkpoint::Fingerprint::new(
                "part",
                tenant.uuid,
                &folder_paths,
                recursive,
                exclusive,
                threshold,
                None,
            );
            let (checkpoint, done) = crate::checkpoint::Checkpoint::<
                crate::model::EnhancedPartSearchResponse,
            >::open(path, fingerprint)?;
            (Some(std::sync::Arc::new(checkpoint)), done)
        }
        None => (None, std::collections::HashMap::new()),
    };

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
        None => {
            // Nothing to search, so nothing to resume.
            if let Some(cp) = &checkpoint {
                cp.finish();
            }
            return Ok(());
        }
    };

    if let Some(cp) = &checkpoint {
        let reusable = all_assets
            .keys()
            .filter(|uuid| recorded_matches.contains_key(uuid))
            .count();
        if reusable > 0 {
            eprintln!(
                "Resuming from checkpoint '{}': {} of {} asset(s) already searched",
                cp.path().display(),
                reusable,
                all_assets.len()
            );
        }
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
    // Shared stop signal, so one terminal failure does not have to be rediscovered
    // by every remaining asset.
    let abort = std::sync::Arc::new(SearchAbort::default());

    // Prepare for concurrent processing.
    //
    // Symmetric pairs are deduplicated into a BTreeMap keyed on the *unordered* pair,
    // so neither the surviving row nor the output order depends on which concurrent
    // search happened to finish first. A HashSet plus a Vec made both depend on it:
    // two identical runs produced byte-different reports.
    let mut deduped = std::collections::BTreeMap::new();

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
        let abort = abort.clone();
        let ui_base_url_for_task = ui_base_url.clone();
        let exclusive_folder_ids = exclusive_folder_ids.clone();
        let checkpoint = checkpoint.clone();
        let recorded = recorded_matches.remove(&asset_uuid);

        let task = tokio::spawn(async move {
            // Already searched by the run this one resumes.
            if let Some(matches) = recorded {
                return Ok((matches, None));
            }

            let _permit = semaphore.acquire().await.unwrap();

            // An earlier task hit something that makes every remaining search
            // pointless. Return without touching the network.
            if abort.is_stopped() {
                return Ok((Vec::new(), Some(SearchFailure::Aborted)));
            }

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
                .part_search(&tenant_uuid, &asset_uuid, threshold, &exclusive_folder_ids)
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

                        let ui_base_url = ui_base_url_for_task.clone();

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
                            asset_type: asset_clone.file_type().cloned().unwrap_or_default(),
                            created_at: asset_clone.created_at().cloned().unwrap_or_default(),
                            updated_at: asset_clone.updated_at().cloned().unwrap_or_default(),
                            state: asset_clone.normalized_processing_status(),
                            is_assembly: asset_clone.is_assembly(),
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

                    // Renewal is evidently working; forget any earlier blip.
                    abort.record_success();
                    if let Some(cp) = &checkpoint {
                        cp.record(asset_uuid, &asset_matches);
                    }
                    Ok((asset_matches, None))
                }
                Err(e) => {
                    let failure = SearchFailure::classify(&e);
                    // Authentication failures are counted rather than acted on
                    // immediately: the client renews the token and retries by itself,
                    // and one failed renewal may be nothing more than a blip at the
                    // auth endpoint. Only an unbroken run of them means the credentials
                    // are genuinely gone - then the run stops, and explains once.
                    let stopping = e.is_authentication_failure() && abort.record_auth_failure();
                    if stopping {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_error_with_remediation(
                            &format!(
                                "Stopping after {} consecutive authentication failures: {}. Remaining assets were not searched.",
                                CONSECUTIVE_AUTH_FAILURES_BEFORE_STOP, e
                            ),
                            &[
                                "Log in again with 'pcli2 auth login'",
                                "Then re-run this command",
                            ],
                        );
                        });
                    } else if !abort.is_stopped() {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_warning(&format!(
                                "🔍 Failed to perform part search for asset {}: {}",
                                asset_clone.name(),
                                e
                            ))
                        });
                    }
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
                    // Apply duplicate filtering to each match. (A,B) and (B,A) are the
                    // same pair, so they share a key.
                    for match_result in &enhanced_match.matches {
                        record_unique_pair(
                            &mut deduped,
                            enhanced_match.reference_asset.uuid,
                            match_result.asset.uuid,
                            enhanced_match.clone(),
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                outcomes.record(Some(SearchFailure::Operational));
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
                outcomes.record(Some(SearchFailure::Operational));
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

    // BTreeMap iteration is ordered by the unordered pair key, so row order is a
    // property of the data rather than of this run's scheduling. Two runs over
    // unchanged data now produce identical output.
    let all_matches: Vec<_> = deduped.into_values().map(|(_, record)| record).collect();

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
                            .map(crate::model::metadata_cell)
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .map(crate::model::metadata_cell)
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

    // The report is written; there is nothing left to resume.
    if let Some(cp) = &checkpoint {
        cp.finish();
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
    // Read once here; it used to be loaded from disk again for every match row.
    let ui_base_url = configuration.get_ui_base_url();
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    // Get folder paths
    let folder_paths: Vec<String> = crate::actions::utils::split_list_values(
        sub_matches
            .get_many::<String>(PARAMETER_FOLDER_PATH)
            .ok_or(CliError::MissingRequiredArgument(
                PARAMETER_FOLDER_PATH.to_string(),
            ))?
            .cloned()
            .collect::<Vec<String>>(),
    );

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
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);

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
    // With --exclusive the server pre-filters to these folders and their subfolders,
    // so tenant-wide result pages are no longer downloaded only to be discarded.
    // The client-side path check on each match still decides the exact set.
    let exclusive_folder_ids: Vec<Uuid> = if exclusive {
        resolve_exclusive_folder_ids(&mut api, &tenant, &folder_paths).await?
    } else {
        Vec::new()
    };

    // With --checkpoint, results recorded by an interrupted run are reused and
    // only the remaining assets are searched. Opened before the folder scan so a
    // file from a different run is refused before minutes are spent scanning.
    let checkpoint_path = sub_matches
        .get_one::<std::path::PathBuf>(crate::commands::params::PARAMETER_CHECKPOINT)
        .cloned();
    let (checkpoint, mut recorded_matches) = match &checkpoint_path {
        Some(path) => {
            let fingerprint = crate::checkpoint::Fingerprint::new(
                "visual",
                tenant.uuid,
                &folder_paths,
                recursive,
                exclusive,
                threshold,
                Some(limit),
            );
            let (checkpoint, done) = crate::checkpoint::Checkpoint::<
                crate::model::EnhancedPartSearchResponse,
            >::open(path, fingerprint)?;
            (Some(std::sync::Arc::new(checkpoint)), done)
        }
        None => (None, std::collections::HashMap::new()),
    };

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
        None => {
            // Nothing to search, so nothing to resume.
            if let Some(cp) = &checkpoint {
                cp.finish();
            }
            return Ok(());
        }
    };

    if let Some(cp) = &checkpoint {
        let reusable = all_assets
            .keys()
            .filter(|uuid| recorded_matches.contains_key(uuid))
            .count();
        if reusable > 0 {
            eprintln!(
                "Resuming from checkpoint '{}': {} of {} asset(s) already searched",
                cp.path().display(),
                reusable,
                all_assets.len()
            );
        }
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
    // Shared stop signal, so one terminal failure does not have to be rediscovered
    // by every remaining asset.
    let abort = std::sync::Arc::new(SearchAbort::default());

    // Prepare for concurrent processing.
    //
    // Symmetric pairs are deduplicated into a BTreeMap keyed on the *unordered* pair,
    // so neither the surviving row nor the output order depends on which concurrent
    // search happened to finish first. A HashSet plus a Vec made both depend on it:
    // two identical runs produced byte-different reports.
    let mut deduped = std::collections::BTreeMap::new();

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
        let abort = abort.clone();
        let ui_base_url_for_task = ui_base_url.clone();
        let exclusive_folder_ids = exclusive_folder_ids.clone();
        let checkpoint = checkpoint.clone();
        let recorded = recorded_matches.remove(&asset_uuid);

        let task = tokio::spawn(async move {
            // Already searched by the run this one resumes.
            if let Some(matches) = recorded {
                return Ok((matches, None));
            }

            let _permit = semaphore.acquire().await.unwrap();

            // An earlier task hit something that makes every remaining search
            // pointless. Return without touching the network.
            if abort.is_stopped() {
                return Ok((Vec::new(), Some(SearchFailure::Aborted)));
            }

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
                .visual_search(
                    &tenant_uuid,
                    &asset_uuid,
                    limit,
                    threshold,
                    &exclusive_folder_ids,
                )
                .await
            {
                Ok(search_results) => {
                    if search_results.matches.len() >= limit {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_warning(&format!(
                                "Visual search for asset {} returned the --limit of {} matches; further matches were not fetched",
                                asset_clone.name(),
                                limit
                            ))
                        });
                    }
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

                        let ui_base_url = ui_base_url_for_task.clone();

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
                            asset_type: asset_clone.file_type().cloned().unwrap_or_default(),
                            created_at: asset_clone.created_at().cloned().unwrap_or_default(),
                            updated_at: asset_clone.updated_at().cloned().unwrap_or_default(),
                            state: asset_clone.normalized_processing_status(),
                            is_assembly: asset_clone.is_assembly(),
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

                    // Renewal is evidently working; forget any earlier blip.
                    abort.record_success();
                    if let Some(cp) = &checkpoint {
                        cp.record(asset_uuid, &asset_matches);
                    }
                    Ok((asset_matches, None))
                }
                Err(e) => {
                    let failure = SearchFailure::classify(&e);
                    // Authentication failures are counted rather than acted on
                    // immediately: the client renews the token and retries by itself,
                    // and one failed renewal may be nothing more than a blip at the
                    // auth endpoint. Only an unbroken run of them means the credentials
                    // are genuinely gone - then the run stops, and explains once.
                    let stopping = e.is_authentication_failure() && abort.record_auth_failure();
                    if stopping {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_error_with_remediation(
                            &format!(
                                "Stopping after {} consecutive authentication failures: {}. Remaining assets were not searched.",
                                CONSECUTIVE_AUTH_FAILURES_BEFORE_STOP, e
                            ),
                            &[
                                "Log in again with 'pcli2 auth login'",
                                "Then re-run this command",
                            ],
                        );
                        });
                    } else if !abort.is_stopped() {
                        under_progress(multi_progress_clone.as_ref().map(|(mp, _)| mp), || {
                            error_utils::report_warning(&format!(
                                "🔍 Failed to perform visual search for asset {}: {}",
                                asset_clone.name(),
                                e
                            ))
                        });
                    }
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
                    // Apply duplicate filtering to each match. (A,B) and (B,A) are the
                    // same pair, so they share a key.
                    for match_result in &enhanced_match.matches {
                        record_unique_pair(
                            &mut deduped,
                            enhanced_match.reference_asset.uuid,
                            match_result.asset.uuid,
                            enhanced_match.clone(),
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                outcomes.record(Some(SearchFailure::Operational));
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
                outcomes.record(Some(SearchFailure::Operational));
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

    // BTreeMap iteration is ordered by the unordered pair key, so row order is a
    // property of the data rather than of this run's scheduling. Two runs over
    // unchanged data now produce identical output.
    let all_matches: Vec<_> = deduped.into_values().map(|(_, record)| record).collect();

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
                            .map(crate::model::metadata_cell)
                            .unwrap_or_default();
                        base_values.push(ref_value);

                        // Add candidate asset metadata value
                        let cand_value = match_pair
                            .candidate_asset
                            .metadata
                            .get(key)
                            .map(crate::model::metadata_cell)
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

    // The report is written; there is nothing left to resume.
    if let Some(cp) = &checkpoint {
        cp.finish();
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
                            .map(crate::model::metadata_cell)
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
                        crate::format::FormattingError::CsvIntoInnerError(Box::new(e)),
                    )));
                }
            };
            let output: String = match crate::format::csv_text(data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(CliError::from(CliActionError::FormattingError(
                        crate::format::FormattingError::Utf8Error(e),
                    )));
                }
            };
            crate::format::print_output(&output);
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
            SearchFailure::classify(&ApiError::HttpStatus {
                status: 503,
                message: "Service Unavailable".to_string()
            }),
            SearchFailure::Operational
        );
        assert_eq!(
            SearchFailure::classify(&ApiError::HttpStatus {
                status: 429,
                message: "Too Many Requests".to_string()
            }),
            SearchFailure::Operational
        );
        assert_eq!(
            SearchFailure::classify(&ApiError::RetryFailed("503".to_string())),
            SearchFailure::Operational
        );
    }

    fn outcomes(attempted: usize, not_searchable: usize, operational: usize) -> SearchOutcomes {
        outcomes_with_aborted(attempted, not_searchable, operational, 0)
    }

    fn outcomes_with_aborted(
        attempted: usize,
        not_searchable: usize,
        operational: usize,
        aborted: usize,
    ) -> SearchOutcomes {
        let mut o = SearchOutcomes::default();
        for i in 0..attempted {
            o.record(if i < operational {
                Some(SearchFailure::Operational)
            } else if i < operational + aborted {
                Some(SearchFailure::Aborted)
            } else if i < operational + aborted + not_searchable {
                Some(SearchFailure::NotSearchable)
            } else {
                None
            });
        }
        o
    }

    /// Two UUIDs with a known order, so "canonical" is unambiguous in the tests.
    fn uuids() -> (Uuid, Uuid) {
        let low = Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap();
        let high = Uuid::parse_str("ffffffff-0000-0000-0000-00000000000f").unwrap();
        assert!(low < high);
        (low, high)
    }

    fn dedup(pairs: &[(Uuid, Uuid, &'static str)]) -> Vec<&'static str> {
        let mut map = std::collections::BTreeMap::new();
        for (reference, candidate, record) in pairs {
            record_unique_pair(&mut map, *reference, *candidate, *record);
        }
        map.into_values().map(|(_, record)| record).collect()
    }

    #[test]
    fn a_pair_is_kept_once_whichever_end_it_was_searched_from() {
        let (low, high) = uuids();
        assert_eq!(dedup(&[(low, high, "low->high")]).len(), 1);
        assert_eq!(dedup(&[(high, low, "high->low")]).len(), 1);
        assert_eq!(
            dedup(&[(low, high, "low->high"), (high, low, "high->low")]).len(),
            1,
            "(A,B) and (B,A) are the same pair"
        );
    }

    #[test]
    fn the_surviving_orientation_does_not_depend_on_arrival_order() {
        // The bug: whichever direction finished first won, so two identical runs
        // disagreed about which asset was the reference.
        let (low, high) = uuids();
        let forward_first = dedup(&[(low, high, "low->high"), (high, low, "high->low")]);
        let reverse_first = dedup(&[(high, low, "high->low"), (low, high, "low->high")]);
        assert_eq!(forward_first, reverse_first);
        assert_eq!(forward_first, vec!["low->high"], "reference sorts first");
    }

    #[test]
    fn a_one_directional_pair_keeps_the_direction_it_was_searched_in() {
        // Only one end gets searched when the other asset is outside the folder set or
        // its own search failed. Dropping the pair, or flipping it to look canonical,
        // would lose or corrupt it - the transformation and comparison URL are
        // directional.
        let (low, high) = uuids();
        assert_eq!(dedup(&[(high, low, "high->low")]), vec!["high->low"]);
    }

    #[test]
    fn output_order_is_independent_of_arrival_order() {
        // Row order came from task completion order, so two runs produced the same
        // rows in different sequences.
        let a = Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap();
        let b = Uuid::parse_str("00000000-0000-0000-0000-00000000000b").unwrap();
        let c = Uuid::parse_str("00000000-0000-0000-0000-00000000000c").unwrap();

        let one = dedup(&[(a, b, "ab"), (a, c, "ac"), (b, c, "bc")]);
        let another = dedup(&[(b, c, "bc"), (a, b, "ab"), (a, c, "ac")]);
        let reversed = dedup(&[(a, c, "ac"), (b, c, "bc"), (a, b, "ab")]);

        assert_eq!(one, vec!["ab", "ac", "bc"]);
        assert_eq!(one, another);
        assert_eq!(one, reversed);
    }

    #[test]
    fn a_single_auth_failure_does_not_stop_the_run() {
        // The whole point of the client's renew-and-retry is that a long run survives
        // its token expiring halfway through. `refresh_token` reports every cause of a
        // failed renewal identically, so a rejected credential looks exactly like a
        // 5xx or a rate limit at the auth endpoint - stopping on the first would
        // abandon a 25-minute run over a hiccup the next request would have survived.
        let abort = SearchAbort::default();
        assert!(!abort.record_auth_failure());
        assert!(!abort.is_stopped());
    }

    #[test]
    fn a_success_forgets_earlier_failures() {
        // Scattered blips must never accumulate into a stop. Renewal demonstrably
        // working is the evidence that the earlier failures were transient.
        let abort = SearchAbort::default();
        for _ in 0..20 {
            assert!(!abort.record_auth_failure());
            assert!(!abort.record_auth_failure());
            abort.record_success();
        }
        assert!(
            !abort.is_stopped(),
            "40 blips, none consecutive, keep going"
        );
    }

    #[test]
    fn an_unbroken_run_of_auth_failures_stops_the_run() {
        // A genuinely dead credential fails every asset, and there is no success to
        // reset the count. Trips at the threshold rather than after 21,068 of them.
        let abort = SearchAbort::default();
        for _ in 1..CONSECUTIVE_AUTH_FAILURES_BEFORE_STOP {
            assert!(!abort.record_auth_failure());
        }
        assert!(
            abort.record_auth_failure(),
            "the failure crossing the threshold explains why"
        );
        assert!(abort.is_stopped());
    }

    #[test]
    fn only_one_task_reports_the_stop() {
        // 21,068 copies of the same message is not diagnostics, it is noise that
        // buries the failures worth reading.
        let abort = SearchAbort::default();
        let mut announced = 0;
        for _ in 0..50 {
            if abort.record_auth_failure() {
                announced += 1;
            }
        }
        assert_eq!(announced, 1, "exactly one caller explains why");
    }

    #[test]
    fn the_classifier_covers_renewal_failure_and_post_renewal_rejection() {
        use crate::physna_v3::ApiError;

        // Renewal itself failed.
        assert!(ApiError::AuthError("expired".into()).is_authentication_failure());
        assert!(ApiError::InvalidToken.is_authentication_failure());
        assert!(ApiError::MissingCredentials.is_authentication_failure());

        // Renewal succeeded but the fresh token was still rejected. Counting only the
        // variants above would miss this and grind through every remaining asset.
        assert!(ApiError::RetryFailed(
            "Original error: 401 Unauthorized, Retry failed with status: 401".into()
        )
        .is_authentication_failure());

        // Not authentication: an unsearchable asset says nothing about credentials.
        assert!(
            !ApiError::ConflictError("Asset not indexed yet".into()).is_authentication_failure()
        );
    }

    #[test]
    fn aborted_searches_count_as_incomplete() {
        // The auth-expiry run, as it would now unfold: one asset actually fails, the
        // remaining ~21k are skipped rather than issued as doomed API calls. The run
        // must still be reported as incomplete and still exit non-zero.
        let o = outcomes_with_aborted(22_378, 51, 1, 21_067);
        assert_eq!(o.incomplete(), 21_068);
        assert!(o.is_materially_incomplete());
        assert!(finish_search_outcomes(&o).is_err());

        let summary = o.summary().expect("failures are always reported");
        assert!(summary.contains("1 failed"), "{}", summary);
        assert!(summary.contains("21,067 not attempted"), "{}", summary);
        assert!(summary.contains("51 not searchable"), "{}", summary);
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

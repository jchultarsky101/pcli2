//! Asset states, metadata coverage, report jobs and failure diagnostics.

use super::*;

/// Represents asset state counts for a tenant
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct AssetStateCounts {
    /// Number of assets currently indexing/processing
    #[serde(rename = "indexing", default)]
    pub processing: Option<u32>,
    /// Number of assets that have finished processing (ready)
    #[serde(rename = "finished", default)]
    pub ready: Option<u32>,
    /// Number of assets that failed to process
    #[serde(rename = "failed", default)]
    pub failed: Option<u32>,
    /// Number of assets that are unsupported
    #[serde(rename = "unsupported", default)]
    pub unsupported: Option<u32>,
    /// Number of assets with no 3D data
    #[serde(rename = "no-3d-data", default)]
    pub no_3d_data: Option<u32>,
}

impl AssetStateCounts {
    /// Create a new AssetStateCounts instance
    pub fn new(
        processing: Option<u32>,
        ready: Option<u32>,
        failed: Option<u32>,
        unsupported: Option<u32>,
        no_3d_data: Option<u32>,
    ) -> AssetStateCounts {
        AssetStateCounts {
            processing,
            ready,
            failed,
            unsupported,
            no_3d_data,
        }
    }
}

/// Which of the paths a client asked about already hold an asset.
///
/// Returned by `POST /tenants/{tenantId}/assets/existing-paths`. Every path is
/// echoed back exactly as it was sent, so a caller tests membership on the
/// string it asked with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExistingPathsResponse {
    #[serde(rename = "existingPaths")]
    pub existing_paths: Vec<String>,
}

/// How many of the tenant's assets carry at least one metadata value.
///
/// `GET /tenants/{tenantId}/metadata-coverage`; the spec types both counts as
/// JSON numbers. Demo assets uploaded by Physna are excluded from both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataCoverageResponse {
    #[serde(rename = "coveredAssets")]
    pub covered_assets: f64,
    #[serde(rename = "totalAssets")]
    pub total_assets: f64,
}

/// The `tenant metadata coverage` output: the counts as integers plus the
/// percentage, so a script does not have to compute it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataCoverage {
    #[serde(rename = "coveredAssets")]
    pub covered_assets: u64,
    #[serde(rename = "totalAssets")]
    pub total_assets: u64,
    /// `coveredAssets / totalAssets * 100`, or 0 for an empty tenant.
    #[serde(rename = "coveragePercent")]
    pub coverage_percent: f64,
}

impl From<MetadataCoverageResponse> for MetadataCoverage {
    fn from(response: MetadataCoverageResponse) -> Self {
        let covered_assets = response.covered_assets.max(0.0).round() as u64;
        let total_assets = response.total_assets.max(0.0).round() as u64;
        let coverage_percent = if total_assets == 0 {
            0.0
        } else {
            covered_assets as f64 / total_assets as f64 * 100.0
        };
        MetadataCoverage {
            covered_assets,
            total_assets,
            coverage_percent,
        }
    }
}

// ---- reports ----------------------------------------------------------------

/// Where a report job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    /// A value this build does not know yet. Physna adds values without bumping
    /// the API version; one new value must not turn a whole listing into a
    /// deserialization error. The weekly spec-drift check reports the new value.
    #[serde(other)]
    Unknown,
}

impl JobStatus {
    /// Every status, as the API spells them.
    pub const ALL: [&'static str; 5] = ["PENDING", "RUNNING", "COMPLETED", "FAILED", "CANCELLED"];

    /// The value as the API spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "PENDING",
            JobStatus::Running => "RUNNING",
            JobStatus::Completed => "COMPLETED",
            JobStatus::Failed => "FAILED",
            JobStatus::Cancelled => "CANCELLED",
            JobStatus::Unknown => "UNKNOWN",
        }
    }

    /// True once the job will not change any more.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

/// What a report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReportType {
    Duplication,
    Simplification,
    Custom,
    /// A value this build does not know yet. Physna adds values without bumping
    /// the API version; one new value must not turn a whole listing into a
    /// deserialization error. The weekly spec-drift check reports the new value.
    #[serde(other)]
    Unknown,
}

impl ReportType {
    /// Every type, as the API spells them.
    pub const ALL: [&'static str; 3] = ["DUPLICATION", "SIMPLIFICATION", "CUSTOM"];

    /// The value as the API spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReportType::Duplication => "DUPLICATION",
            ReportType::Simplification => "SIMPLIFICATION",
            ReportType::Custom => "CUSTOM",
            ReportType::Unknown => "UNKNOWN",
        }
    }
}

/// Who created a report or a metadata field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Creator {
    pub id: Uuid,
    pub email: String,
}

/// A report job, as `GET /tenants/{tenantId}/reports` and friends describe it.
///
/// Only the fields the spec marks required are plain; everything else is
/// optional and left out of the JSON output when the API did not send it.
/// `metadataFilters` is kept as raw JSON: pcli2 never interprets it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub status: JobStatus,
    /// Percent complete, 0 to 100.
    pub progress: f64,
    #[serde(rename = "reportType")]
    pub report_type: ReportType,
    #[serde(rename = "minThreshold")]
    pub min_threshold: f64,
    #[serde(rename = "maxThreshold")]
    pub max_threshold: f64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "groupCount")]
    pub group_count: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<Creator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    #[serde(rename = "folderIds", default, skip_serializing_if = "Option::is_none")]
    pub folder_ids: Option<Vec<String>>,
    #[serde(
        rename = "folderPaths",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub folder_paths: Option<Vec<String>>,
    #[serde(
        rename = "excludedFolderIds",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub excluded_folder_ids: Option<Vec<String>>,
    #[serde(
        rename = "excludeAssemblies",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub exclude_assemblies: Option<bool>,
    #[serde(
        rename = "excludeExactDuplicates",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub exclude_exact_duplicates: Option<bool>,
    #[serde(
        rename = "includeHomeFolderAssets",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub include_home_folder_assets: Option<bool>,
    #[serde(
        rename = "metadataFilters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata_filters: Option<serde_json::Value>,
    #[serde(
        rename = "searchQuery",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub search_query: Option<String>,
    #[serde(
        rename = "modelIndexConfigId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub model_index_config_id: Option<String>,
}

impl Report {
    /// The name to show for the report: its name, or its id when it has none.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// One page of `GET /tenants/{tenantId}/reports`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportListResponse {
    pub reports: Vec<Report>,
    #[serde(rename = "pageData", default)]
    pub page_data: PageData,
}

/// `GET /tenants/{tenantId}/reports/{id}` and the answer to a create.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleReportResponse {
    pub report: Report,
}

/// The `report list` output: every report fetched, as the API ordered them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportList {
    pub reports: Vec<Report>,
}

/// What `report create` sends for a duplication report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateDuplicationReportRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "minThreshold")]
    pub min_threshold: f64,
    #[serde(rename = "maxThreshold")]
    pub max_threshold: f64,
    #[serde(rename = "folderIds", skip_serializing_if = "Vec::is_empty")]
    pub folder_ids: Vec<String>,
    #[serde(rename = "excludedFolderIds", skip_serializing_if = "Vec::is_empty")]
    pub excluded_folder_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    #[serde(rename = "excludeAssemblies")]
    pub exclude_assemblies: bool,
    #[serde(rename = "excludeExactDuplicates")]
    pub exclude_exact_duplicates: bool,
    #[serde(rename = "includeHomeFolderAssets")]
    pub include_home_folder_assets: bool,
}

/// The `report diagnose` output: the report's identity next to what the
/// server knows about its failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportFailureDiagnostics {
    #[serde(rename = "reportId")]
    pub report_id: String,
    #[serde(rename = "reportName", skip_serializing_if = "Option::is_none")]
    pub report_name: Option<String>,
    #[serde(rename = "reportStatus")]
    pub report_status: JobStatus,
    pub status: FailureDiagnosticsStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<FailureKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(rename = "occurredAt", skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<String>,
}

impl ReportFailureDiagnostics {
    /// Pair a report with the diagnostics the server returned for it.
    pub fn new(report: &Report, diagnostics: FailureDiagnostics) -> Self {
        ReportFailureDiagnostics {
            report_id: report.id.clone(),
            report_name: report.name.clone(),
            report_status: report.status,
            status: diagnostics.status,
            kind: diagnostics.kind,
            summary: diagnostics.summary,
            trace_id: diagnostics.trace_id,
            occurred_at: diagnostics.occurred_at,
        }
    }
}

// ---- failure diagnostics ---------------------------------------------------

/// Whether the deployment can look up why an asset failed.
///
/// Returned by `GET /tenants/{tenantId}/failure-diagnostics/availability`. It is
/// a property of the deployment, not of the tenant: customer-managed and enclave
/// stacks have no failure log search, and every lookup there answers
/// `unavailable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureDiagnosticsAvailability {
    pub available: bool,
}

/// Outcome of a failure-diagnostics lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureDiagnosticsStatus {
    /// A matching failure log entry was located.
    Found,
    /// The record is not failed, or the entry has aged out of log retention.
    NotFound,
    /// Log search is not configured for this deployment.
    Unavailable,
    /// A value this build does not know yet. Physna adds values without bumping
    /// the API version; one new value must not turn a whole listing into a
    /// deserialization error. The weekly spec-drift check reports the new value.
    #[serde(other)]
    Unknown,
}

impl FailureDiagnosticsStatus {
    /// The value as the API spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureDiagnosticsStatus::Found => "found",
            FailureDiagnosticsStatus::NotFound => "not-found",
            FailureDiagnosticsStatus::Unavailable => "unavailable",
            FailureDiagnosticsStatus::Unknown => "unknown",
        }
    }
}

/// Which side a failure is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureKind {
    /// A failure the requester can act on; `summary` says what to change.
    User,
    /// A failure on Physna's side; `summary` is a fixed string and the
    /// `traceId` is what to quote to support.
    Internal,
    /// A value this build does not know yet. Physna adds values without bumping
    /// the API version; one new value must not turn a whole listing into a
    /// deserialization error. The weekly spec-drift check reports the new value.
    #[serde(other)]
    Unknown,
}

impl FailureKind {
    /// The value as the API spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureKind::User => "user",
            FailureKind::Internal => "internal",
            FailureKind::Unknown => "unknown",
        }
    }
}

/// Why an asset failed to process, as the ingestion logs remember it.
///
/// Returned by `GET /tenants/{tenantId}/assets/{assetId}/failure-diagnostics`.
/// Nothing is persisted server-side: the lookup searches the logs on demand,
/// so the answer is only as durable as log retention. Only `status` is always
/// present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureDiagnostics {
    pub status: FailureDiagnosticsStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FailureKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "traceId", default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(
        rename = "occurredAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub occurred_at: Option<String>,
}

/// The `asset diagnose` output: the asset's identity and state next to what
/// the server knows about its failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetFailureDiagnostics {
    #[serde(rename = "assetPath")]
    pub asset_path: String,
    #[serde(rename = "assetUuid")]
    pub asset_uuid: Uuid,
    /// The asset's processing state as the asset record reports it
    /// (`failed`, `finished`, ...), so a `not-found` answer can be read.
    #[serde(rename = "assetState", skip_serializing_if = "Option::is_none")]
    pub asset_state: Option<String>,
    pub status: FailureDiagnosticsStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<FailureKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(rename = "occurredAt", skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<String>,
}

impl AssetFailureDiagnostics {
    /// Pair an asset with the diagnostics the server returned for it.
    pub fn new(asset: &Asset, diagnostics: FailureDiagnostics) -> Self {
        AssetFailureDiagnostics {
            asset_path: asset.path(),
            asset_uuid: asset.uuid(),
            asset_state: asset.processing_status().cloned(),
            status: diagnostics.status,
            kind: diagnostics.kind,
            summary: diagnostics.summary,
            trace_id: diagnostics.trace_id,
            occurred_at: diagnostics.occurred_at,
        }
    }
}

/// Which surface a recent failure came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureSource {
    Asset,
    Report,
    PartFinderReport,
    /// A value this build does not know yet. Physna adds values without bumping
    /// the API version; one new value must not turn a whole listing into a
    /// deserialization error. The weekly spec-drift check reports the new value.
    #[serde(other)]
    Unknown,
}

impl FailureSource {
    /// Every kind the listing can filter by, as the API spells them.
    pub const ALL: [&'static str; 3] = ["asset", "report", "part-finder-report"];

    /// The value as the API spells it.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureSource::Asset => "asset",
            FailureSource::Report => "report",
            FailureSource::PartFinderReport => "part-finder-report",
            FailureSource::Unknown => "unknown",
        }
    }
}

/// One failed asset, report or part-finder report in a tenant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentFailure {
    pub kind: FailureSource,
    pub id: Uuid,
    pub name: String,
    /// When the record last changed: nothing persists the moment of failure,
    /// so this is the closest available signal.
    #[serde(rename = "failedAt")]
    pub failed_at: String,
}

/// Tenant-wide failure totals, not just the current page.
///
/// The spec types these as JSON numbers, so they are read as floats and
/// exposed as counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureCountsByKind {
    pub asset: f64,
    pub report: f64,
    #[serde(rename = "part-finder-report")]
    pub part_finder_report: f64,
}

/// One page of `GET /tenants/{tenantId}/failures`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentFailuresPage {
    pub failures: Vec<RecentFailure>,
    #[serde(rename = "countsByKind")]
    pub counts_by_kind: FailureCountsByKind,
    #[serde(rename = "pageData")]
    pub page_data: PageData,
}

/// The `tenant failures` output: every failure fetched, newest first, with the
/// tenant-wide totals so a limited listing still says how many there are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentFailuresList {
    pub failures: Vec<RecentFailure>,
    #[serde(rename = "countsByKind")]
    pub counts_by_kind: FailureCountsByKind,
}

/// Represents a health report computed from all assets in a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetHealthReport {
    pub total: u32,
    pub finished: u32,
    pub indexing: u32,
    pub failed: u32,
    pub unsupported: u32,
    pub no_3d_data: u32,
    pub missing_dependencies: u32,
    pub assemblies: u32,
    pub parts: u32,
    #[serde(serialize_with = "serialize_sorted")]
    pub file_types: HashMap<String, u32>,
    /// Assets in a state none of the counters above covers (a state added to the
    /// API later, or none at all). Without it the counters did not add up to
    /// `total`, with nothing to explain the gap.
    #[serde(default)]
    pub other: u32,
}

impl AssetHealthReport {
    pub fn from_assets(assets: &AssetList) -> Self {
        let mut report = AssetHealthReport {
            total: 0,
            finished: 0,
            indexing: 0,
            failed: 0,
            unsupported: 0,
            no_3d_data: 0,
            missing_dependencies: 0,
            assemblies: 0,
            parts: 0,
            file_types: HashMap::new(),
            other: 0,
        };

        for asset in assets.iter() {
            report.total += 1;

            match asset.processing_status().map(|s| s.as_str()) {
                Some("finished") => report.finished += 1,
                Some("indexing") => report.indexing += 1,
                Some("failed") => report.failed += 1,
                Some("unsupported") => report.unsupported += 1,
                Some("no-3d-data") => report.no_3d_data += 1,
                Some("missing-dependencies") => report.missing_dependencies += 1,
                _ => report.other += 1,
            }

            if asset.is_assembly() {
                report.assemblies += 1;
            } else {
                report.parts += 1;
            }

            if let Some(ft) = asset.file_type() {
                *report.file_types.entry(ft.to_lowercase()).or_insert(0) += 1;
            }
        }

        report
    }

    pub fn error_total(&self) -> u32 {
        self.failed + self.unsupported + self.no_3d_data + self.missing_dependencies
    }
}

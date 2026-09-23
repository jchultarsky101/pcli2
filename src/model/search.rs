//! Geometric, part and visual search results and pairwise scores.

use super::*;

/// Represents a match result from the geometric search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricMatch {
    /// The matching asset details
    pub asset: AssetResponse,
    /// The similarity percentage
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl GeometricMatch {
    /// Get the asset ID
    pub fn asset_uuid(&self) -> &Uuid {
        &self.asset.uuid
    }

    /// Get the asset path
    pub fn path(&self) -> &str {
        &self.asset.path
    }

    /// Get the similarity score (0.0 to 100.0)
    pub fn score(&self) -> f64 {
        self.match_percentage
    }
}

/// Represents a part match with forward and reverse similarity percentages
///
/// This structure holds information about a single part match, including the
/// matching asset and both forward and reverse match percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartMatch {
    /// The matching asset details
    pub asset: AssetResponse,
    /// The forward match percentage
    #[serde(rename = "forwardMatchPercentage")]
    pub forward_match_percentage: Option<f64>,
    /// The reverse match percentage
    #[serde(rename = "reverseMatchPercentage")]
    pub reverse_match_percentage: Option<f64>,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl PartMatch {
    /// Get the asset ID
    pub fn asset_uuid(&self) -> &Uuid {
        &self.asset.uuid
    }

    /// Get the asset path
    pub fn path(&self) -> &str {
        &self.asset.path
    }

    /// Get the forward match percentage (0.0 to 100.0)
    pub fn forward_score(&self) -> f64 {
        self.forward_match_percentage.unwrap_or(0.0)
    }

    /// Get the reverse match percentage (0.0 to 100.0)
    pub fn reverse_score(&self) -> f64 {
        self.reverse_match_percentage.unwrap_or(0.0)
    }
}

/// Response structure for part search operations
///
/// This structure holds the results of a part search operation, including
/// the list of matching assets and pagination/filter information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartSearchResponse {
    /// The list of matching assets
    pub matches: Vec<PartMatch>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
    /// Filter information
    #[serde(rename = "filterData")]
    pub filter_data: Option<FilterData>,
}

/// Enhanced response structure for part search that includes reference asset information
///
/// This structure extends the basic PartSearchResponse by including information about
/// the reference asset that was searched against, making it easier to understand
/// the context of the matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedPartSearchResponse {
    /// The reference asset that was searched against
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The list of matching assets
    pub matches: Vec<PartMatch>,
}

/// Represents a matching pair from part search with both reference and candidate assets
///
/// This structure holds information about a single part match, including both the
/// reference asset (the one being searched) and the candidate asset (the matching one),
/// along with both forward and reverse match percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartMatchPair {
    /// The reference asset that was searched against
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The matching candidate asset
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The forward match percentage
    #[serde(rename = "forwardMatchPercentage")]
    pub forward_match_percentage: Option<f64>,
    /// The reverse match percentage
    #[serde(rename = "reverseMatchPercentage")]
    pub reverse_match_percentage: Option<f64>,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

/// Represents a pair of assets that matched in a visual search
/// This structure excludes match percentages since visual search doesn't provide them
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualMatchPair {
    /// The reference asset (the one being compared against)
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The candidate asset (the one that matched)
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl PartMatchPair {
    /// Create a new PartMatchPair from a reference asset and a part match
    pub fn from_reference_and_match(
        reference_asset: AssetResponse,
        match_result: PartMatch,
    ) -> Self {
        PartMatchPair {
            reference_asset,
            candidate_asset: match_result.asset,
            forward_match_percentage: match_result.forward_match_percentage,
            reverse_match_percentage: match_result.reverse_match_percentage,
            transformation: match_result.transformation,
            comparison_url: match_result.comparison_url,
        }
    }
}

/// Represents a 4x4 transformation matrix
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformationMatrix {
    /// The 4x4 matrix values in row-major order
    pub matrix: [f64; 16],
}

/// Represents filter data in API responses
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterData {
    /// Extensions filter information
    pub extensions: Vec<FilterCount>,
    /// Folders filter information
    pub folders: Vec<FilterCount>,
    /// Metadata filter information
    #[serde(serialize_with = "serialize_sorted")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Represents a filter and its count
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterCount {
    /// The filter value
    pub filter: String,
    /// The count of items matching this filter
    pub count: u32,
}

/// Represents a matching pair from folder-based geometric matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderGeometricMatch {
    /// Name of the reference asset (the one from the source folder)
    #[serde(rename = "referenceAssetName")]
    pub reference_asset_name: String,
    /// Name of the candidate asset (the one that matched)
    #[serde(rename = "candidateAssetName")]
    pub candidate_asset_name: String,
    /// Match percentage between the assets
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// Full path of the reference asset
    #[serde(rename = "referenceAssetPath")]
    pub reference_asset_path: String,
    /// Full path of the candidate asset
    #[serde(rename = "candidateAssetPath")]
    pub candidate_asset_path: String,
    /// UUID of the reference asset
    #[serde(rename = "referenceAssetUuid")]
    pub reference_asset_uuid: Uuid,
    /// UUID of the candidate asset
    #[serde(rename = "candidateAssetUuid")]
    pub candidate_asset_uuid: Uuid,
    /// URL to the Physna comparison viewer for these two assets
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: String,
}

/// Represents the response from folder-based geometric matching
pub type FolderGeometricMatchResponse = Vec<FolderGeometricMatch>;

/// Represents the response from the geometric search API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricSearchResponse {
    /// The list of matching assets
    pub matches: Vec<GeometricMatch>,
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
    /// Filter information
    #[serde(rename = "filterData")]
    pub filter_data: Option<FilterData>,
}

/// Enhanced geometric search response that includes the reference asset information.
///
/// This structure extends the basic geometric search response by including the reference
/// asset that was searched against, making it easier to display match results with context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedGeometricSearchResponse {
    /// The reference asset that was searched against
    pub reference_asset: AssetResponse,
    /// The list of matching assets
    pub matches: Vec<GeometricMatch>,
}

/// Structure to represent a geometric match with both reference and candidate assets.
///
/// This structure holds information about a single geometric match, including both the
/// reference asset (the one being searched) and the candidate asset (the matching one),
/// along with the similarity percentage and transformation matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricMatchPair {
    /// The reference asset that was searched against
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    /// The matching candidate asset
    #[serde(rename = "candidateAsset")]
    pub candidate_asset: AssetResponse,
    /// The similarity percentage
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// The transformation matrix for the match
    #[serde(rename = "transformation")]
    pub transformation: Option<TransformationMatrix>,
    /// The comparison URL for viewing the match in the UI
    #[serde(rename = "comparisonUrl")]
    pub comparison_url: Option<String>,
}

impl GeometricMatchPair {
    /// Create a new GeometricMatchPair from a reference asset and a geometric match
    pub fn from_reference_and_match(
        reference_asset: AssetResponse,
        match_result: GeometricMatch,
    ) -> Self {
        GeometricMatchPair {
            reference_asset,
            candidate_asset: match_result.asset,
            match_percentage: match_result.match_percentage,
            transformation: match_result.transformation,
            comparison_url: match_result.comparison_url,
        }
    }
}

/// Geometric similarity scores between two assets.
///
/// Returned by the "match scores" endpoint as part of [`MatchScoresResponse`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricMatchScores {
    /// Overall geometric similarity. 100% means the two assets are geometrically identical.
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// How much of the source (reference) asset's geometry exists in the target (candidate) asset.
    #[serde(rename = "forwardMatchPercentage")]
    pub forward_match_percentage: f64,
    /// How much of the target (candidate) asset's geometry exists in the source (reference) asset.
    #[serde(rename = "reverseMatchPercentage")]
    pub reverse_match_percentage: f64,
}

/// Volumetric similarity scores between two assets.
///
/// Volumetric scoring must be explicitly enabled for the tenant, so this is
/// optional in [`MatchScoresResponse`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VolumetricMatchScores {
    /// Overall volumetric similarity: how much of the source asset's volume is
    /// contained in the target asset.
    #[serde(rename = "matchPercentage")]
    pub match_percentage: f64,
    /// Holes in the source asset with no counterpart in the target.
    #[serde(rename = "sourceHoleMisses", skip_serializing_if = "Option::is_none")]
    pub source_hole_misses: Option<Vec<HoleDescriptor>>,
    /// Holes in the target asset with no counterpart in the source.
    #[serde(rename = "targetHoleMisses", skip_serializing_if = "Option::is_none")]
    pub target_hole_misses: Option<Vec<HoleDescriptor>>,
    /// Sum of the centre-to-centre distances of the matched holes, in millimetres.
    #[serde(
        rename = "matchedHoleCenterDistanceTotalMm",
        skip_serializing_if = "Option::is_none"
    )]
    pub matched_hole_center_distance_total_mm: Option<f64>,
    /// Volume that the two assets do not share, broken down by side.
    #[serde(rename = "mismatchVolumeMm3", skip_serializing_if = "Option::is_none")]
    pub mismatch_volume_mm3: Option<MismatchVolume>,
}

/// A point or direction in three dimensions, as the volumetric scores report it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A hole the volumetric comparison found in one asset but not the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleDescriptor {
    /// Hole centre after alignment, in millimetres.
    #[serde(rename = "centerMm")]
    pub center_mm: Vector3,
    /// Unitless direction vector along the hole axis.
    pub axis: Vector3,
    /// Detected hole radius, in millimetres.
    #[serde(rename = "radiusMm")]
    pub radius_mm: f64,
}

/// Volume the two assets do not share, in cubic millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MismatchVolume {
    /// Source volume with no counterpart in the target.
    #[serde(rename = "sourceMissMm3")]
    pub source_miss_mm3: f64,
    /// Target volume with no counterpart in the source.
    #[serde(rename = "targetMissMm3")]
    pub target_miss_mm3: f64,
    /// Source hole volume with no counterpart in the target.
    #[serde(rename = "sourceHoleMissMm3")]
    pub source_hole_miss_mm3: f64,
    /// Target hole volume with no counterpart in the source.
    #[serde(rename = "targetHoleMissMm3")]
    pub target_hole_miss_mm3: f64,
    /// Total mismatched volume.
    #[serde(rename = "totalMm3")]
    pub total_mm3: f64,
}

/// Response from the "match scores" endpoint.
///
/// Holds the pairwise match scores between two specific assets, as returned by
/// `GET /tenants/{tenantId}/assets/{assetId}/match-scores/{targetAssetId}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchScoresResponse {
    /// Geometric similarity scores (always present).
    pub geometric: GeometricMatchScores,
    /// Volumetric similarity scores (present only when enabled for the tenant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumetric: Option<VolumetricMatchScores>,
}

/// Pairwise similarity result between a reference and a candidate asset.
///
/// This is the output structure for the `asset similarity` command. It combines
/// the identities of both assets with the match scores returned by the API and a
/// link to the comparison view in the Physna UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetSimilarity {
    /// The path of the reference (source) asset.
    #[serde(rename = "referenceAssetPath")]
    pub reference_asset_path: String,
    /// The UUID of the reference (source) asset.
    #[serde(rename = "referenceAssetUuid")]
    pub reference_asset_uuid: Uuid,
    /// The path of the candidate (target) asset.
    #[serde(rename = "candidateAssetPath")]
    pub candidate_asset_path: String,
    /// The UUID of the candidate (target) asset.
    #[serde(rename = "candidateAssetUuid")]
    pub candidate_asset_uuid: Uuid,
    /// Geometric similarity scores between the two assets.
    pub geometric: GeometricMatchScores,
    /// Volumetric similarity scores between the two assets, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumetric: Option<VolumetricMatchScores>,
    /// The comparison URL for viewing the two assets side by side in the UI.
    #[serde(rename = "comparisonUrl", skip_serializing_if = "Option::is_none")]
    pub comparison_url: Option<String>,
}

// Metadata field models for Physna V3 API

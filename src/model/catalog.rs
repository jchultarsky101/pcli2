//! Catalog search: matching one of your assets against a catalog tenant.
//!
//! A catalog is a tenant Physna marks as searchable from other tenants
//! (`isRemoteSearchTenant` in `/users/me`), such as a supplier catalog. The
//! models here are separate from the single-tenant match models on purpose, so
//! the `catalog` commands can never change what `asset geometric-match` and
//! friends print.

use super::*;
use uuid::Uuid;

/// A tenant the user may search into with `pcli2 catalog`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub id: Uuid,
    /// Short name, as `--in` accepts it.
    pub name: String,
    pub description: String,
    /// The user's role in the catalog (usually `search`).
    pub role: String,
}

impl Catalog {
    /// Every catalog among the user's tenants: flagged remote and enabled.
    /// Sorted by name so listings and error messages are stable.
    pub fn from_tenant_settings(settings: &[TenantSetting]) -> Vec<Catalog> {
        let mut catalogs: Vec<Catalog> = settings
            .iter()
            .filter(|s| s.is_remote_search_tenant && s.user_enabled)
            .map(|s| Catalog {
                id: s.tenant_uuid,
                name: s.tenant_short_name.clone(),
                description: s.tenant_display_name.clone(),
                role: s.tenant_role.clone(),
            })
            .collect();
        catalogs.sort_by(|a, b| a.name.cmp(&b.name));
        catalogs
    }
}

/// The `catalog list` output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogList {
    pub catalogs: Vec<Catalog>,
}

/// Which catalog search ran; decides the score columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogSearchKind {
    Geometric,
    Part,
    Visual,
}

impl CatalogSearchKind {
    /// The API endpoint under `/tenants/assets/`.
    pub fn endpoint(&self) -> &'static str {
        match self {
            CatalogSearchKind::Geometric => "geometric-search",
            CatalogSearchKind::Part => "part-search",
            CatalogSearchKind::Visual => "visual-search",
        }
    }

    /// The body field `--threshold` goes into: a similarity floor for
    /// geometric and part search, a size tolerance for visual search.
    pub fn threshold_field(&self) -> &'static str {
        match self {
            CatalogSearchKind::Geometric | CatalogSearchKind::Part => "minThreshold",
            CatalogSearchKind::Visual => "sizeThreshold",
        }
    }
}

/// One match, as all three cross-tenant search endpoints return it: the
/// asset plus whichever scores that search computes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogMatch {
    pub asset: AssetResponse,
    #[serde(
        rename = "matchPercentage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub match_percentage: Option<f64>,
    #[serde(
        rename = "forwardMatchPercentage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub forward_match_percentage: Option<f64>,
    #[serde(
        rename = "reverseMatchPercentage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reverse_match_percentage: Option<f64>,
    /// Filled in by pcli2: the side-by-side view in the web application.
    #[serde(rename = "comparisonUrl", default)]
    pub comparison_url: Option<String>,
}

/// One page of a cross-tenant search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogSearchPage {
    pub matches: Vec<CatalogMatch>,
    #[serde(rename = "pageData")]
    pub page_data: Option<PageData>,
}

/// The output of `catalog geometric-match | part-match | visual-match`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogMatchReport {
    #[serde(rename = "searchType")]
    pub search_type: CatalogSearchKind,
    #[serde(rename = "referenceAsset")]
    pub reference_asset: AssetResponse,
    pub catalog: Catalog,
    pub matches: Vec<CatalogMatch>,
}

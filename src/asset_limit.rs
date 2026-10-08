//! The tenant asset limit.
//!
//! `/users/me` carries `assetLimit` for the few tenants that have one, and
//! `assetLimitWarningThresholdPercent` (85 on every tenant seen so far) for
//! when the web UI starts warning. pcli2 warns at the same point: in
//! `tenant usage`, and before a bulk upload that would reach it.
//!
//! The asset count is the sum of `assets/type-counts`, which matches the
//! `tenant state` total. Checking is best-effort: an error is logged at debug
//! level and the command goes on as if the tenant had no limit.

use crate::model::{Counts, TenantSetting};
use crate::physna_v3::PhysnaApiClient;
use serde::Serialize;
use uuid::Uuid;

/// A tenant's limit and where it stands against it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetLimitStatus {
    pub limit: u64,
    /// Absent when the server did not send one; pcli2 then warns only at the
    /// limit itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning_threshold_percent: Option<f64>,
    pub assets: u64,
}

impl AssetLimitStatus {
    /// `None` when the tenant has no limit.
    pub fn new(setting: &TenantSetting, asset_types: &Counts) -> Option<Self> {
        let limit = setting.asset_limit.filter(|limit| *limit > 0.0)?;
        Some(AssetLimitStatus {
            limit: limit.round() as u64,
            warning_threshold_percent: setting.asset_limit_warning_threshold_percent,
            assets: asset_types.0.iter().map(|(_, count)| count).sum(),
        })
    }

    /// The count at which the warning starts.
    fn warning_at(&self) -> u64 {
        match self.warning_threshold_percent {
            Some(percent) if percent > 0.0 && percent < 100.0 => {
                (self.limit as f64 * percent / 100.0).ceil() as u64
            }
            _ => self.limit,
        }
    }

    /// What to tell the user before adding `incoming` assets (0 for none),
    /// or `None` when the tenant stays below the warning threshold.
    pub fn warning(&self, tenant: &str, incoming: u64) -> Option<String> {
        let after = self.assets.saturating_add(incoming);
        if after < self.warning_at() {
            return None;
        }
        let percent = |count: u64| count as f64 * 100.0 / self.limit as f64;
        Some(if self.assets >= self.limit {
            format!(
                "tenant '{}' is at its asset limit: {} of {} assets; new uploads may be refused",
                tenant, self.assets, self.limit
            )
        } else if incoming == 0 {
            format!(
                "tenant '{}' holds {} of its {} asset limit ({:.0}%)",
                tenant,
                self.assets,
                self.limit,
                percent(self.assets)
            )
        } else if after > self.limit {
            format!(
                "uploading {} file(s) would take tenant '{}' past its asset limit: it holds {} of {} assets",
                incoming, tenant, self.assets, self.limit
            )
        } else {
            format!(
                "after uploading {} file(s), tenant '{}' will hold {} of its {} asset limit ({:.0}%)",
                incoming,
                tenant,
                after,
                self.limit,
                percent(after)
            )
        })
    }
}

/// The tenant's settings from the tenant cache (no request when it is fresh).
async fn tenant_setting(api: &mut PhysnaApiClient, tenant_uuid: &Uuid) -> Option<TenantSetting> {
    crate::tenant_cache::TenantCache::get_all_tenants(api, false)
        .await
        .map_err(|e| tracing::debug!("Asset limit check skipped: {}", e))
        .ok()?
        .into_iter()
        .find(|t| t.tenant_uuid == *tenant_uuid)
}

/// The tenant's limit status, given its asset counts. `None` when it has no
/// limit or its settings could not be read.
pub async fn status_from_counts(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    asset_types: &Counts,
) -> Option<AssetLimitStatus> {
    AssetLimitStatus::new(&tenant_setting(api, tenant_uuid).await?, asset_types)
}

/// Warn when uploading `incoming` files would reach the tenant's warning
/// threshold. Costs one request, and only for a tenant that has a limit.
pub async fn warn_before_upload(
    api: &mut PhysnaApiClient,
    tenant_uuid: &Uuid,
    tenant_name: &str,
    incoming: usize,
) {
    if incoming == 0 {
        return;
    }
    let Some(setting) = tenant_setting(api, tenant_uuid).await else {
        return;
    };
    if setting.asset_limit.is_none_or(|limit| limit <= 0.0) {
        return;
    }
    let asset_types = match api.get_asset_type_counts(tenant_uuid).await {
        Ok(counts) => counts,
        Err(e) => {
            tracing::debug!("Asset limit check skipped: {}", e);
            return;
        }
    };
    if let Some(warning) = AssetLimitStatus::new(&setting, &asset_types)
        .and_then(|status| status.warning(tenant_name, incoming as u64))
    {
        crate::error_utils::report_warning(&warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(limit: u64, percent: Option<f64>, assets: u64) -> AssetLimitStatus {
        AssetLimitStatus {
            limit,
            warning_threshold_percent: percent,
            assets,
        }
    }

    #[test]
    fn a_tenant_without_a_limit_has_no_status() {
        let mut setting: TenantSetting = serde_json::from_str(
            r#"{"tenantId":"25493af9-b110-4731-8934-87f8ea7e008b","tenantRole":"admin",
                "userEnabled":true,"assetLimitWarningThresholdPercent":85}"#,
        )
        .unwrap();
        let counts = Counts(vec![("model".to_string(), 10), ("image".to_string(), 2)]);
        assert_eq!(AssetLimitStatus::new(&setting, &counts), None);

        setting.asset_limit = Some(20000.0);
        assert_eq!(
            AssetLimitStatus::new(&setting, &counts),
            Some(status(20000, Some(85.0), 12))
        );
    }

    #[test]
    fn below_the_threshold_there_is_no_warning() {
        assert_eq!(
            status(20000, Some(85.0), 10065).warning("harpereng", 0),
            None
        );
        // 16,999 is one short of 85% of 20,000.
        assert_eq!(status(20000, Some(85.0), 16000).warning("t", 999), None);
    }

    #[test]
    fn the_warning_starts_at_the_threshold() {
        assert_eq!(
            status(20000, Some(85.0), 17000).warning("t", 0).unwrap(),
            "tenant 't' holds 17000 of its 20000 asset limit (85%)"
        );
        assert_eq!(
            status(20000, Some(85.0), 16000).warning("t", 1000).unwrap(),
            "after uploading 1000 file(s), tenant 't' will hold 17000 of its 20000 asset limit (85%)"
        );
    }

    #[test]
    fn going_past_or_being_at_the_limit_says_so() {
        assert_eq!(
            status(20000, Some(85.0), 19990).warning("t", 50).unwrap(),
            "uploading 50 file(s) would take tenant 't' past its asset limit: it holds 19990 of 20000 assets"
        );
        assert_eq!(
            status(20000, Some(85.0), 20000).warning("t", 1).unwrap(),
            "tenant 't' is at its asset limit: 20000 of 20000 assets; new uploads may be refused"
        );
    }

    #[test]
    fn without_a_threshold_only_the_limit_warns() {
        assert_eq!(status(100, None, 99).warning("t", 0), None);
        assert!(status(100, None, 99).warning("t", 2).is_some());
        assert!(status(100, Some(0.0), 99).warning("t", 0).is_none());
    }
}

//! Cross-tenant search into a catalog tenant.

use super::*;
use crate::model::{CatalogMatch, CatalogSearchKind, CatalogSearchPage};

/// Largest page asked for; the endpoints document no maximum, and this is the
/// size the single-tenant searches use.
const MAX_PER_PAGE: usize = 1000;

impl PhysnaApiClient {
    /// Search `catalog_tenant` for matches of an asset that lives in
    /// `asset_tenant`.
    ///
    /// `POST /tenants/assets/{geometric,part,visual}-search`, one pair of
    /// tenants per call, everything (paging included) in the body. Pages are
    /// fetched until `limit` matches are in hand or the results run out.
    ///
    /// Returns the matches and whether `limit` cut them short.
    pub async fn catalog_search(
        &mut self,
        kind: CatalogSearchKind,
        asset_tenant: &Uuid,
        asset: &Uuid,
        catalog_tenant: &Uuid,
        threshold: f64,
        limit: usize,
    ) -> Result<(Vec<CatalogMatch>, bool), ApiError> {
        let url = format!("{}/tenants/assets/{}", self.base_url, kind.endpoint());
        let per_page = limit.clamp(1, MAX_PER_PAGE);
        let mut pager = crate::paging::Pager::new("catalog search");
        let mut matches = Vec::new();
        let mut more = false;
        loop {
            let mut body = serde_json::json!({
                "page": pager.page(),
                "perPage": per_page,
                "assetId": asset,
                "assetTenantId": asset_tenant,
                "searchTenantId": catalog_tenant,
            });
            body[kind.threshold_field()] = serde_json::json!(threshold);
            debug!("Catalog {} request: {}", kind.endpoint(), body);

            let page: CatalogSearchPage = self.post_query(&url, &body).await?;
            matches.extend(page.matches);
            let Some(page_data) = page.page_data else {
                break;
            };
            if matches.len() >= limit {
                more = matches.len() > limit || page_data.current_page < page_data.last_page;
                break;
            }
            if !pager.advance(page_data.current_page, page_data.last_page, matches.len()) {
                break;
            }
        }
        matches.truncate(limit);
        Ok((matches, more))
    }
}

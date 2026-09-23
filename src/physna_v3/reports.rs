//! Report jobs and failure diagnostics.

use super::*;

impl PhysnaApiClient {
    /// The tenant's reports, newest first as the API orders them.
    ///
    /// `GET /tenants/{tenantId}/reports?type&status&page&perPage`; every page
    /// (`perPage=1000`, the maximum) unless `limit` stops it early.
    pub async fn list_reports(
        &mut self,
        tenant_uuid: &Uuid,
        report_type: Option<&str>,
        status: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::model::Report>, ApiError> {
        const PER_PAGE: usize = 1000;
        let mut filters = String::new();
        if let Some(report_type) = report_type {
            filters.push_str(&format!("&type={}", urlencoding::encode(report_type)));
        }
        if let Some(status) = status {
            filters.push_str(&format!("&status={}", urlencoding::encode(status)));
        }
        // One page size for the whole walk. It used to shrink on the last request
        // to fit --limit, but the page number still counted in the old size, so
        // `page=2&perPage=500` after a first page of 1000 returned records 501-1000
        // again and never reached 1001-1500.
        let per_page = limit.map_or(PER_PAGE, |limit| PER_PAGE.min(limit.max(1)));
        let mut pager = crate::paging::Pager::new("report listing");
        let mut reports = Vec::new();
        loop {
            let page = pager.page();
            let url = format!(
                "{}/tenants/{}/reports?page={}&perPage={}{}",
                self.base_url, tenant_uuid, page, per_page, filters
            );
            debug!("Reports request URL: {}", url);
            let response: crate::model::ReportListResponse = self.get(&url).await?;
            let current_page = response.page_data.current_page;
            let last_page = response.page_data.last_page;
            reports.extend(response.reports);
            let enough = limit.is_some_and(|limit| reports.len() >= limit);
            if enough || !pager.advance(current_page, last_page, reports.len()) {
                break;
            }
        }
        if let Some(limit) = limit {
            reports.truncate(limit);
        }
        Ok(reports)
    }

    /// One report. `GET /tenants/{tenantId}/reports/{id}`.
    pub async fn get_report(
        &mut self,
        tenant_uuid: &Uuid,
        report_id: &Uuid,
    ) -> Result<crate::model::Report, ApiError> {
        let url = format!(
            "{}/tenants/{}/reports/{}",
            self.base_url, tenant_uuid, report_id
        );
        let response: crate::model::SingleReportResponse = self.get(&url).await?;
        Ok(response.report)
    }

    /// Delete a report. `DELETE /tenants/{tenantId}/reports/{id}`, 204.
    pub async fn delete_report(
        &mut self,
        tenant_uuid: &Uuid,
        report_id: &Uuid,
    ) -> Result<(), ApiError> {
        self.delete(&format!("/tenants/{}/reports/{}", tenant_uuid, report_id))
            .await
    }

    /// Why a report failed, from the job service logs.
    ///
    /// `GET /tenants/{tenantId}/reports/{id}/failure-diagnostics`; same answer
    /// shape as an asset's.
    pub async fn get_report_failure_diagnostics(
        &mut self,
        tenant_uuid: &Uuid,
        report_id: &Uuid,
    ) -> Result<crate::model::FailureDiagnostics, ApiError> {
        let url = format!(
            "{}/tenants/{}/reports/{}/failure-diagnostics",
            self.base_url, tenant_uuid, report_id
        );
        self.get(&url).await
    }

    /// Start a duplication report. `POST /tenants/{tenantId}/reports/duplication`, 201.
    pub async fn create_duplication_report(
        &mut self,
        tenant_uuid: &Uuid,
        request: &crate::model::CreateDuplicationReportRequest,
    ) -> Result<crate::model::Report, ApiError> {
        let url = format!(
            "{}/tenants/{}/reports/duplication",
            self.base_url, tenant_uuid
        );
        let response: crate::model::SingleReportResponse = self.post(&url, request).await?;
        Ok(response.report)
    }

    /// Download a report's data as CSV or XLSX straight to disk.
    ///
    /// `GET /tenants/{tenantId}/reports/{id}/file?format=csv|xlsx`; the report
    /// must be COMPLETED. Same temporary-file discipline as an asset download.
    pub async fn download_report_to_file(
        &mut self,
        tenant_uuid: &Uuid,
        report_id: &Uuid,
        format: &str,
        dest: &std::path::Path,
    ) -> Result<u64, ApiError> {
        let url = format!(
            "{}/tenants/{}/reports/{}/file?format={}",
            self.base_url, tenant_uuid, report_id, format
        );
        self.download_url_to_file(&url, &format!("report {}", report_id), dest)
            .await
    }

    /// Whether this deployment can look up why an asset failed.
    ///
    /// `GET /tenants/{tenantId}/failure-diagnostics/availability`. The tenant
    /// only scopes authorisation: availability is a property of the deployment.
    pub async fn get_failure_diagnostics_availability(
        &mut self,
        tenant_uuid: &Uuid,
    ) -> Result<crate::model::FailureDiagnosticsAvailability, ApiError> {
        let url = format!(
            "{}/tenants/{}/failure-diagnostics/availability",
            self.base_url, tenant_uuid
        );
        debug!("Failure diagnostics availability request URL: {}", url);
        self.get(&url).await
    }

    /// Why an asset failed to process, from the ingestion service logs.
    ///
    /// `GET /tenants/{tenantId}/assets/{assetId}/failure-diagnostics`. The
    /// lookup searches logs on demand and answers `not-found` for an asset that
    /// is not failed or whose entry has aged out, and `unavailable` where log
    /// search is not configured.
    pub async fn get_asset_failure_diagnostics(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
    ) -> Result<crate::model::FailureDiagnostics, ApiError> {
        let url = format!(
            "{}/tenants/{}/assets/{}/failure-diagnostics",
            self.base_url, tenant_uuid, asset_uuid
        );
        debug!("Asset failure diagnostics request URL: {}", url);
        self.get(&url).await
    }

    /// The failed assets, reports and part-finder reports of a tenant, newest
    /// first, as `GET /tenants/{tenantId}/failures` lists them.
    ///
    /// `kinds` narrows the listing (empty means every kind). Pages are fetched
    /// until `limit` failures are collected or the listing ends; the
    /// tenant-wide totals come from the first page.
    pub async fn list_recent_failures(
        &mut self,
        tenant_uuid: &Uuid,
        kinds: &[crate::model::FailureSource],
        limit: Option<usize>,
    ) -> Result<crate::model::RecentFailuresList, ApiError> {
        // The spec maximum for this endpoint.
        const PER_PAGE: usize = 100;
        let kinds_query = if kinds.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = kinds.iter().map(|k| k.as_str()).collect();
            format!("&kinds={}", names.join(","))
        };

        // One page size for the whole walk. It used to shrink on the last request
        // to fit --limit, but the page number still counted in the old size, so
        // `page=2&perPage=500` after a first page of 1000 returned records 501-1000
        // again and never reached 1001-1500.
        let per_page = limit.map_or(PER_PAGE, |limit| PER_PAGE.min(limit.max(1)));
        let mut pager = crate::paging::Pager::new("failure listing");
        let mut failures = Vec::new();
        let mut counts_by_kind = None;
        loop {
            let page = pager.page();
            let url = format!(
                "{}/tenants/{}/failures?page={}&perPage={}{}",
                self.base_url, tenant_uuid, page, per_page, kinds_query
            );
            debug!("Recent failures request URL: {}", url);
            let response: crate::model::RecentFailuresPage = self.get(&url).await?;
            let current_page = response.page_data.current_page;
            let last_page = response.page_data.last_page;
            if counts_by_kind.is_none() {
                counts_by_kind = Some(response.counts_by_kind);
            }
            failures.extend(response.failures);

            let enough = limit.is_some_and(|limit| failures.len() >= limit);
            if enough || !pager.advance(current_page, last_page, failures.len()) {
                break;
            }
        }
        if let Some(limit) = limit {
            failures.truncate(limit);
        }
        Ok(crate::model::RecentFailuresList {
            failures,
            counts_by_kind: counts_by_kind.unwrap_or(crate::model::FailureCountsByKind {
                asset: 0.0,
                report: 0.0,
                part_finder_report: 0.0,
            }),
        })
    }
}

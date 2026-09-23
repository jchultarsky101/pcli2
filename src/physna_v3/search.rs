//! Geometric, part, visual and text search, and pairwise scores.

use super::*;

impl PhysnaApiClient {
    /// Perform a geometric search for similar assets with pagination support
    ///
    /// This method searches for assets that are geometrically similar to the reference asset.
    /// It uses Physna's advanced geometric matching algorithms to find assets with similar
    /// shapes, regardless of orientation, scale, or position differences.
    ///
    /// The method includes automatic retry logic for handling conflict errors (HTTP 409),
    /// which can occur when the search service is temporarily busy.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the reference asset
    /// * `asset_id` - The UUID of the reference asset to search for similar matches
    /// * `threshold` - The similarity threshold as a percentage (0.00 to 100.00)
    ///   Lower values return more matches, higher values return fewer but more similar matches
    ///
    /// # Returns
    /// * `Ok(crate::model::GeometricSearchResponse)` - The search results containing similar assets
    /// * `Err(ApiError)` - If there's an HTTP error, authentication issue, or other API error
    ///
    /// # Example
    /// ```no_run
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// use uuid::Uuid;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let tenant_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let asset_uuid = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
    ///     let matches = client.geometric_search(&tenant_uuid, &asset_uuid, 85.0, &[]).await?;
    ///     for match_result in &matches.matches {
    ///         println!("Found match: {} ({}% similar)", match_result.path(), match_result.score());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn geometric_search(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        threshold: f64,
        folder_ids: &[Uuid],
    ) -> Result<crate::model::GeometricSearchResponse, ApiError> {
        debug!(
            "Starting geometric search for tenant_uuid: {}, asset_uuid: {}, threshold: {}",
            tenant_uuid, asset_uuid, threshold
        );
        let url = format!(
            "{}/tenants/{}/assets/{}/geometric-search",
            self.base_url, tenant_uuid, asset_uuid
        );

        // Initialize with page 1 and reasonable page size
        let mut all_matches = Vec::new();
        let mut page = 1;
        let per_page = 500; // search pages carry full asset records; the API allows up to 1000

        // Hard limit to prevent runaway pagination if the server misbehaves,
        // consistent with part_search and visual_search.
        let max_pages_limit = 1000;

        loop {
            debug!("Fetching page {} of geometric search results", page);

            if page > max_pages_limit {
                tracing::warn!(
                    "Geometric search results truncated at {} matches ({}-page safety limit); results are incomplete",
                    all_matches.len(),
                    max_pages_limit
                );
                break;
            }

            // Build request body with the correct structure
            let body = serde_json::json!({
                "page": page,
                "perPage": per_page,
                "searchQuery": "",
                "filters": {
                    "folders": [],
                    // Folder ids restrict the search server-side (subfolders included);
                    // empty means the whole tenant.
                    "folderIds": folder_ids,
                    "metadata": {},
                    "extensions": []
                },
                "minThreshold": threshold  // Use threshold directly as percentage
            });

            debug!("Sending geometric search request to: {}", url);
            // Execute POST request
            let result: Result<crate::model::GeometricSearchResponse, ApiError> =
                self.post_query(&url, &body).await;

            match result {
                Ok(response) => {
                    // Check if we have pagination data
                    if let Some(page_data) = &response.page_data {
                        debug!(
                            "Page {}/{} with {} total matches",
                            page_data.current_page, page_data.last_page, page_data.total
                        );

                        // Guard against the endpoint failing to advance
                        // `current_page` (would otherwise loop forever
                        // re-appending duplicate matches).
                        let stalled = page_data.current_page < page;
                        let last_page_reached = page_data.current_page >= page_data.last_page;

                        // Add matches from this page to our collection
                        all_matches.extend(response.matches);

                        if last_page_reached || stalled {
                            debug!("Reached last page of results (stalled: {})", stalled);
                            break;
                        }

                        // Move to next page
                        page += 1;
                    } else {
                        // No pagination data - the endpoint returned everything
                        // at once. Keep this page's matches together with any
                        // previously accumulated pages instead of discarding
                        // them.
                        debug!("No pagination data in response, returning accumulated matches");
                        all_matches.extend(response.matches);
                        break;
                    }
                }
                Err(e) => {
                    // Return error immediately
                    debug!("Geometric search failed: {}", e);
                    return Err(e);
                }
            }
        }

        // Create a response with all matches and combined pagination data
        let final_response = crate::model::GeometricSearchResponse {
            matches: all_matches,
            page_data: None, // We've aggregated all pages
            filter_data: None,
        };

        debug!(
            "Geometric search completed for asset_id: {} with {} total matches",
            asset_uuid,
            final_response.matches.len()
        );
        Ok(final_response)
    }

    /// Get pairwise match scores between two assets.
    ///
    /// Returns geometric (and, when enabled for the tenant, volumetric) match
    /// percentages comparing how similar two 3D models are to each other. Both
    /// assets must be 3D models in a finished state.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant containing both assets
    /// * `source_asset_uuid` - The UUID of the source (reference) asset
    /// * `target_asset_uuid` - The UUID of the target (candidate) asset to compare against
    ///
    /// # Returns
    /// * `Ok(MatchScoresResponse)` - The pairwise match scores
    /// * `Err(ApiError)` - If there's an HTTP error, authentication issue, or other API error
    pub async fn match_scores(
        &mut self,
        tenant_uuid: &Uuid,
        source_asset_uuid: &Uuid,
        target_asset_uuid: &Uuid,
    ) -> Result<crate::model::MatchScoresResponse, ApiError> {
        debug!(
            "Getting match scores for tenant_uuid: {}, source_asset_uuid: {}, target_asset_uuid: {}",
            tenant_uuid, source_asset_uuid, target_asset_uuid
        );
        let url = format!(
            "{}/tenants/{}/assets/{}/match-scores/{}",
            self.base_url, tenant_uuid, source_asset_uuid, target_asset_uuid
        );

        self.get(&url).await
    }

    /// Perform a part search to find geometrically similar assets using the part search algorithm
    ///
    /// This method uses Physna's advanced part search algorithms to find assets with similar
    /// geometry to the provided reference asset. The part search algorithm may provide different
    /// results than the standard geometric search, potentially with forward and reverse match percentages.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant containing the assets
    /// * `asset_uuid` - The UUID of the reference asset to search for matches
    /// * `threshold` - The similarity threshold as a percentage (0.00 to 100.00)
    ///   Lower values return more matches, higher values return fewer but more similar matches
    ///
    /// # Returns
    /// * `Ok(GeometricSearchResponse)` - The search results containing similar assets
    /// * `Err(ApiError)` - If there's an HTTP error, authentication issue, or other API error
    ///
    /// # Example
    /// ```no_run
    /// use pcli2::physna_v3::PhysnaApiClient;
    /// use uuid::Uuid;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = PhysnaApiClient::new();
    ///     let tenant_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    ///     let asset_uuid = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000")?;
    ///     let matches = client.part_search(&tenant_uuid, &asset_uuid, 85.0, &[]).await?;
    ///     for match_result in &matches.matches {
    ///         println!("Found match: {} (forward: {:.2}%, reverse: {:.2}%)",
    ///             match_result.path(),
    ///             match_result.forward_match_percentage.unwrap_or(0.0),
    ///             match_result.reverse_match_percentage.unwrap_or(0.0));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn part_search(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        threshold: f64,
        folder_ids: &[Uuid],
    ) -> Result<crate::model::PartSearchResponse, ApiError> {
        debug!(
            "Starting part search for tenant_uuid: {}, asset_uuid: {}, threshold: {}",
            tenant_uuid, asset_uuid, threshold
        );
        let url = format!(
            "{}/tenants/{}/assets/{}/part-search",
            self.base_url, tenant_uuid, asset_uuid
        );

        // Initialize with page 1 and reasonable page size
        let mut all_matches = Vec::new();
        let per_page = 500; // search pages carry full asset records; the API allows up to 1000
                            // Stops on the last page, on a repeated page number (which used to append
                            // the same matches again), and at the page cap with a warning.
        let mut pager = crate::paging::Pager::new("part search");

        loop {
            let page = pager.page();
            debug!("Fetching page {} of part search results", page);

            // Build request body with the correct structure
            let body = serde_json::json!({
                "page": page,
                "perPage": per_page,
                "searchQuery": "",
                "filters": {
                    "folders": [],
                    // Folder ids restrict the search server-side (subfolders included);
                    // empty means the whole tenant.
                    "folderIds": folder_ids,
                    "metadata": {},
                    "extensions": []
                },
                "minThreshold": threshold  // Use threshold directly as percentage
            });

            debug!("Sending part search request to: {}", url);
            // Execute POST request
            let result: Result<crate::model::PartSearchResponse, ApiError> =
                self.post_query(&url, &body).await;

            match result {
                Ok(response) => {
                    // Check if we have pagination data
                    if let Some(page_data) = &response.page_data {
                        debug!(
                            "Page {}/{} with {} total matches",
                            page_data.current_page, page_data.last_page, page_data.total
                        );

                        // A repeated page is not added again.
                        if page_data.current_page < page {
                            pager.advance(
                                page_data.current_page,
                                page_data.last_page,
                                all_matches.len(),
                            );
                            break;
                        }
                        all_matches.extend(response.matches);
                        if !pager.advance(
                            page_data.current_page,
                            page_data.last_page,
                            all_matches.len(),
                        ) {
                            break;
                        }
                    } else {
                        // No pagination data - the endpoint returned everything
                        // at once. Keep this page's matches together with any
                        // previously accumulated pages instead of discarding
                        // them.
                        debug!("No pagination data in response, returning accumulated matches");
                        all_matches.extend(response.matches);
                        break;
                    }
                }
                Err(e) => {
                    // Return error immediately
                    debug!("Part search failed: {}", e);
                    return Err(e);
                }
            }
        }

        // Create a response with all matches and combined pagination data
        let final_response = crate::model::PartSearchResponse {
            matches: all_matches,
            page_data: None, // We've aggregated all pages
            filter_data: None,
        };

        debug!(
            "Part search completed for asset_id: {} with {} total matches",
            asset_uuid,
            final_response.matches.len()
        );
        Ok(final_response)
    }

    /// Performs a visual search for similar assets
    ///
    /// This method performs a visual search to find assets that are visually similar to the provided asset.
    /// The search results are ordered by relevance as determined by the visual search algorithm.
    ///
    /// It uses the `/tenants/assets/visual-search` endpoint (operation
    /// `CrossTenantVisualSearch`), which supersedes the deprecated
    /// `/tenants/{tenantId}/assets/{assetId}/visual-search` endpoint. All
    /// inputs, including pagination, are carried in the request body. The
    /// search is performed within a single tenant by passing the same tenant
    /// UUID as both `assetTenantId` and `searchTenantId`.
    ///
    /// # Arguments
    ///
    /// * `tenant_uuid` - The UUID of the tenant that owns the reference asset and is searched for matches
    /// * `asset_uuid` - The UUID of the reference asset to search for visually similar matches
    /// * `limit` - Maximum number of matches to return
    /// * `threshold` - Size threshold percentage (0.00 to 100.00) controlling how strictly
    ///   matches are filtered by geometric size relative to the reference asset.
    ///   Higher values are stricter (90 means within ±10% of the reference size);
    ///   0 disables size filtering entirely
    ///
    /// # Returns
    ///
    /// * `Ok(PartSearchResponse)` - The search results with visually similar assets
    /// * `Err(ApiError)` - If the API request fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use pcli2::physna_v3::PhysnaApiClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut client = PhysnaApiClient::new();
    /// # let tenant_uuid = Uuid::nil();
    /// # let asset_uuid = Uuid::nil();
    /// let matches = client.visual_search(&tenant_uuid, &asset_uuid, 100, 80.0, &[]).await?;
    /// for match_result in &matches.matches {
    ///     println!("Found visually similar asset: {}", match_result.path());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn visual_search(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        limit: usize,
        threshold: f64,
        folder_ids: &[Uuid],
    ) -> Result<crate::model::PartSearchResponse, ApiError> {
        debug!(
            "Starting visual search for tenant_uuid: {}, asset_uuid: {}, limit: {}, threshold: {}",
            tenant_uuid, asset_uuid, limit, threshold
        );
        let url = format!("{}/tenants/assets/visual-search", self.base_url);

        // Accumulate matches across pages until we have at least `limit`.
        let mut all_matches = Vec::new();
        let mut page = 1;
        // Request only as many per page as we need (capped at the endpoint
        // maximum of 1000), so a small limit costs a single request.
        let per_page = limit.clamp(1, 1000);

        // Track the maximum last_page value seen to prevent infinite loops.
        let mut max_last_page_seen = 0;
        let max_pages_limit = 1000; // Hard limit to prevent excessive API calls

        loop {
            // Check if we've hit the hard limit.
            if page > max_pages_limit {
                warn!(
                    "Reached hard page limit of {}, stopping visual search pagination",
                    max_pages_limit
                );
                break;
            }

            // Unlike the deprecated per-asset endpoint (which took pagination
            // as query parameters), this endpoint carries everything in the
            // request body, including the asset/tenant identifiers. The
            // optional `filters` object is omitted entirely.
            let mut body = serde_json::json!({
                "page": page,
                "perPage": per_page,
                "searchQuery": "",
                "assetId": asset_uuid,
                "assetTenantId": tenant_uuid,
                "searchTenantId": tenant_uuid,
                "sizeThreshold": threshold
            });
            // The visual endpoint takes no filters object by default; only add one
            // when --exclusive supplies folders to restrict the search to. The API
            // requires `metadata` in any filters object (without it: 400
            // "Validation Failed" for every asset).
            if !folder_ids.is_empty() {
                body["filters"] = serde_json::json!({ "folderIds": folder_ids, "metadata": {} });
            }

            debug!("Sending visual search request to: {}", url);
            // Execute POST request.
            let result: Result<crate::model::PartSearchResponse, ApiError> =
                self.post_query(&url, &body).await;

            match result {
                Ok(response) => {
                    // Check if we have pagination data.
                    if let Some(page_data) = &response.page_data {
                        debug!(
                            "Page {}/{} with {} total matches",
                            page_data.current_page, page_data.last_page, page_data.total
                        );

                        // Update the maximum last_page value seen.
                        if page_data.last_page > max_last_page_seen {
                            max_last_page_seen = page_data.last_page;
                        }

                        // Add matches from this page to our collection.
                        all_matches.extend(response.matches);

                        // Stop once we have enough results, reach the last page,
                        // or the endpoint fails to advance `current_page` (guard
                        // against looping forever on the same page).
                        if all_matches.len() >= limit
                            || page_data.current_page >= page_data.last_page
                            || page_data.current_page < page
                        {
                            break;
                        }

                        // Move to the next page.
                        page += 1;
                    } else {
                        // No pagination data - return this single page, capped at
                        // the requested limit.
                        debug!(
                            "No pagination data in visual search response, returning single page"
                        );
                        let mut response = response;
                        response.matches.truncate(limit);
                        return Ok(response);
                    }
                }
                Err(e) => {
                    // Return error immediately.
                    debug!("Visual search failed: {}", e);
                    return Err(e);
                }
            }
        }

        // The last page may overshoot the requested limit; cap it here.
        all_matches.truncate(limit);

        // Create a response with all matches aggregated across pages.
        let final_response = crate::model::PartSearchResponse {
            matches: all_matches,
            page_data: None, // We've aggregated all pages
            filter_data: None,
        };

        debug!(
            "Visual search completed for asset_id: {} with {} total matches",
            asset_uuid,
            final_response.matches.len()
        );
        Ok(final_response)
    }

    /// Performs a text search for assets matching the provided text query
    ///
    /// This method performs a text search to find assets that match the provided text query.
    /// The search looks through asset names, paths, and associated metadata to find relevant matches.
    /// The search results are ordered by relevance as determined by the text search algorithm.
    ///
    /// Results are accumulated across pages until `limit` matches are
    /// collected or the last page is reached, mirroring `visual_search`.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant to search within
    /// * `text_query` - The text query to search for in assets
    /// * `limit` - Maximum number of matches to return
    ///
    /// # Returns
    /// * `Ok((TextSearchResponse, bool))` - The search results with
    ///   text-matched assets, plus a flag that is `true` when `limit` cut the
    ///   result set short (more matches were available from the endpoint)
    ///
    pub async fn text_search(
        &mut self,
        tenant_uuid: &Uuid,
        text_query: &str,
        limit: usize,
    ) -> Result<(crate::model::TextSearchResponse, bool), ApiError> {
        debug!(
            "Starting text search for tenant_uuid: {}, query: {}, limit: {}",
            tenant_uuid, text_query, limit
        );
        let url = format!(
            "{}/tenants/{}/assets/text-search",
            self.base_url, tenant_uuid
        );

        // Accumulate matches across pages until we have at least `limit`.
        let mut all_matches = Vec::new();
        let mut page = 1;
        // Request only as many per page as we need (capped at the endpoint
        // maximum of 1000), so a small limit costs a single request.
        let per_page = limit.clamp(1, 1000);

        // Track the maximum last_page value seen to prevent infinite loops.
        let mut max_last_page_seen = 0;
        let max_pages_limit = 1000; // Hard limit to prevent excessive API calls

        // Total number of matches reported by the endpoint. Note: the endpoint
        // may report a larger total than it actually serves across pages, so
        // this is informational only — truncation is detected from pagination
        // state, not from this figure.
        let mut total_available: Option<usize> = None;

        // Set when pagination stops early because `limit` was reached while
        // more pages remained.
        let mut truncated = false;

        loop {
            if page > max_pages_limit {
                warn!(
                    "Reached hard page limit of {}, stopping text search pagination",
                    max_pages_limit
                );
                break;
            }

            let body = serde_json::json!({
                "page": page,
                "perPage": per_page,
                "searchQuery": text_query,
                "filters": {
                    "folders": [],
                    "metadata": {},
                    "extensions": []
                }
            });

            debug!("Sending text search request to: {}", url);
            let result: Result<crate::model::TextSearchResponse, ApiError> =
                self.post_query(&url, &body).await;

            match result {
                Ok(response) => {
                    if let Some(page_data) = &response.page_data {
                        debug!(
                            "Page {}/{} with {} total matches",
                            page_data.current_page, page_data.last_page, page_data.total
                        );

                        if page_data.last_page > max_last_page_seen {
                            max_last_page_seen = page_data.last_page;
                        }

                        total_available = Some(page_data.total);

                        all_matches.extend(response.matches);

                        // Stop once we have enough results, reach the last page,
                        // or the endpoint fails to advance `current_page` (guard
                        // against looping forever on the same page).
                        if all_matches.len() >= limit
                            || page_data.current_page >= page_data.last_page
                            || page_data.current_page < page
                        {
                            truncated = all_matches.len() >= limit
                                && page_data.current_page < page_data.last_page;
                            break;
                        }

                        page += 1;
                    } else {
                        // No pagination data - return this single page, capped at
                        // the requested limit.
                        debug!("No pagination data in text search response, returning single page");
                        let mut response = response;
                        total_available = Some(response.matches.len());
                        truncated = response.matches.len() > limit;
                        response.matches.truncate(limit);
                        all_matches = response.matches;
                        break;
                    }
                }
                Err(e) => {
                    debug!("Text search failed: {}", e);
                    return Err(e);
                }
            }
        }

        // The last page may overshoot the requested limit; cap it here.
        truncated = truncated || all_matches.len() > limit;
        all_matches.truncate(limit);

        debug!(
            "Text search completed for query: '{}' with {} total matches (truncated: {})",
            text_query,
            all_matches.len(),
            truncated
        );

        // All pages have been aggregated into a single result set; keep the
        // endpoint-reported total for informational purposes.
        let page_data = total_available.map(|total| crate::model::PageData {
            total,
            per_page,
            current_page: 1,
            last_page: 1,
            start_index: 0,
            end_index: all_matches.len(),
        });

        Ok((
            crate::model::TextSearchResponse {
                matches: all_matches,
                page_data,
                filter_data: None,
            },
            truncated,
        ))
    }
}

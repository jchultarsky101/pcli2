//! Asset files and thumbnails.

use super::*;

impl PhysnaApiClient {
    /// Download an asset's file straight to disk.
    ///
    /// The body is streamed into `<dest>.part` and renamed into place only once it
    /// has arrived whole, so an interrupted download never leaves a truncated file
    /// under the final name. An empty body is an error: a zero-byte model file is
    /// never what was asked for. Returns the number of bytes written.
    pub async fn download_asset_to_file(
        &mut self,
        tenant_id: &str,
        asset_id: &str,
        asset_name_opt: Option<&str>,
        dest: &std::path::Path,
    ) -> Result<u64, ApiError> {
        let what = format!("asset {}", describe_asset(asset_id, asset_name_opt));
        let url = format!(
            "{}/tenants/{}/assets/{}/file",
            self.base_url, tenant_id, asset_id
        );
        self.download_url_to_file(&url, &what, dest).await
    }

    /// Download asset thumbnail
    ///
    /// This function downloads the thumbnail image for a specific asset.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to download the thumbnail for
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - Thumbnail image content as bytes
    /// * `Err(ApiError)` - If there was an error during API calls
    ///   Generate the thumbnail URL for an asset
    pub fn generate_asset_thumbnail_url(&self, tenant_id: &str, asset_id: &str) -> String {
        format!(
            "{}/tenants/{}/assets/{}/thumbnail.png",
            self.base_url, tenant_id, asset_id
        )
    }

    pub async fn download_asset_thumbnail(
        &mut self,
        tenant_id: &str,
        asset_id: &str,
    ) -> Result<Vec<u8>, ApiError> {
        debug!(
            "Downloading asset thumbnail for tenant_id: {}, asset_id: {}",
            tenant_id, asset_id
        );

        let url = self.generate_asset_thumbnail_url(tenant_id, asset_id);
        debug!("Download asset thumbnail request URL: {}", url);

        let response = self
            .request_with_auth(|client| Ok(client.get(&url)), true)
            .await
            .map_err(|e| match e {
                ApiError::NotFoundError(message) => ApiError::NotFoundError(format!(
                    "Asset thumbnail not found - the asset {} may not have a thumbnail or the asset ID is incorrect. API Response: {}",
                    asset_id, message
                )),
                other => other.about(&format!("thumbnail of asset {}", asset_id)),
            })?;

        let bytes = response.bytes().await.map_err(|e| {
            debug!("Failed to read thumbnail response bytes: {}", e);
            ApiError::from(e)
        })?;
        debug!("Successfully downloaded thumbnail for asset: {}", asset_id);
        Ok(bytes.to_vec())
    }
}

//! Metadata values and the metadata-field registry.

use super::*;

impl PhysnaApiClient {
    /// Update an asset's metadata fields
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to update
    /// * `metadata` - A map of metadata key-value pairs to update
    ///
    /// # Returns
    /// * `Ok(crate::model::AssetResponse)` - Successfully updated asset with new metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn update_asset_metadata(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        metadata: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/tenants/{}/assets/{}",
            self.base_url, tenant_uuid, asset_uuid
        );

        let body = serde_json::json!({
            "metadata": metadata
        });

        // Log the request body for debugging
        debug!(
            "Updating asset metadata with JSON body: {}",
            serde_json::to_string_pretty(&body)
                .unwrap_or_else(|_| "Unable to serialize body".to_string())
        );

        self.patch_no_response(&url, &body).await
    }

    /// Update an asset's metadata fields, automatically registering new metadata keys if needed
    ///
    /// # Arguments
    /// * `tenant_uuid` - The ID of the tenant that owns the asset
    /// * `asset_uuid` - The UUID of the asset to update
    /// * `metadata` - A map of metadata key-value pairs to update
    ///
    /// # Returns
    /// * `Ok(())` - Successfully updated asset metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error, including MetadataTypeMismatch if a type conflict is detected
    pub async fn update_asset_metadata_with_registration(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        metadata: &std::collections::HashMap<String, serde_json::Value>,
        declared_types: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<(), ApiError> {
        let mut registry = self.fetch_metadata_field_types(tenant_uuid).await?;
        self.update_asset_metadata_with_registry(
            tenant_uuid,
            asset_uuid,
            metadata,
            declared_types,
            &mut registry,
        )
        .await
    }

    /// The tenant's registered metadata fields, name to type.
    ///
    /// A failure here is returned rather than treated as "no fields": with an empty
    /// registry every value is inferred as a new text field, the API then rejects
    /// `"18"` for a number field, and the user is told their CSV has a type
    /// conflict when the real problem was a request that did not go through.
    pub async fn fetch_metadata_field_types(
        &mut self,
        tenant_uuid: &Uuid,
    ) -> Result<std::collections::HashMap<String, String>, ApiError> {
        let fields = self.get_metadata_fields(&tenant_uuid.to_string()).await?;
        debug!(
            "Retrieved {} existing metadata fields for tenant",
            fields.metadata_fields.len()
        );
        Ok(fields
            .metadata_fields
            .into_iter()
            .map(|field| (field.name, field.field_type))
            .collect())
    }

    /// Like [`Self::update_asset_metadata_with_registration`], with the field
    /// registry supplied by the caller and kept up to date as fields are
    /// registered - so a batch fetches the registry once instead of once per row.
    pub async fn update_asset_metadata_with_registry(
        &mut self,
        tenant_uuid: &Uuid,
        asset_uuid: &Uuid,
        metadata: &std::collections::HashMap<String, serde_json::Value>,
        declared_types: Option<&std::collections::HashMap<String, String>>,
        field_type_map: &mut std::collections::HashMap<String, String>,
    ) -> Result<(), ApiError> {
        // Build the outgoing payload, coercing every value to the type the field
        // is (or will be) registered with. The registered type is authoritative:
        // a string "18" is sent as the JSON number 18 for a number-typed field, a
        // URL string is kept as a string for a url-typed field, and so on. A
        // value that cannot be represented as the field's type is a genuine
        // conflict and returns MetadataTypeMismatch.
        let mut coerced: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::with_capacity(metadata.len());

        for (key, value) in metadata.iter() {
            if let Some(expected_type) = field_type_map.get(key) {
                // Field already exists: coerce the value to its registered type.
                match Self::coerce_value_to_type(value, expected_type) {
                    Some(coerced_value) => {
                        debug!(
                            "Coerced metadata field '{}' to registered type '{}'",
                            key, expected_type
                        );
                        coerced.insert(key.clone(), coerced_value);
                    }
                    None => {
                        return Err(ApiError::MetadataTypeMismatch {
                            field_name: key.clone(),
                            expected_type: expected_type.clone(),
                            provided_type: Self::infer_json_value_type(value),
                        });
                    }
                }
            } else {
                // New field: register it with the caller-declared type when
                // provided, otherwise infer it from the value (defaulting to
                // text). A declared type must still be able to hold the value.
                let register_type = Self::new_field_type(key, value, declared_types);

                let coerced_value = match Self::coerce_value_to_type(value, register_type) {
                    Some(v) => v,
                    None => {
                        return Err(ApiError::MetadataTypeMismatch {
                            field_name: key.clone(),
                            expected_type: register_type.to_string(),
                            provided_type: Self::infer_json_value_type(value),
                        });
                    }
                };

                debug!(
                    "Metadata field '{}' not found; registering as type '{}'",
                    key, register_type
                );
                match self
                    .create_metadata_field(&tenant_uuid.to_string(), key, Some(register_type))
                    .await
                {
                    Ok(_) => {
                        debug!("Successfully registered new metadata field: {}", key);
                        field_type_map.insert(key.clone(), register_type.to_string());
                    }
                    Err(e) => {
                        debug!("Failed to register metadata field '{}': {}", key, e);
                        // Continue anyway, as the API might allow setting values
                        // for unregistered keys.
                    }
                }
                coerced.insert(key.clone(), coerced_value);
            }
        }

        // Now update the asset metadata with the coerced values
        self.update_asset_metadata(tenant_uuid, asset_uuid, &coerced)
            .await
    }

    /// Register the fields of `metadata` that `field_type_map` does not know yet,
    /// exactly as [`Self::update_asset_metadata_with_registry`] would on its
    /// first write of each: the declared type, else the inferred one. A field
    /// whose value cannot be held by that type, or that fails to register, is
    /// left for the write to report.
    ///
    /// Batch writes run concurrently; registering up front, in row order, keeps
    /// two rows from racing to create the same field.
    pub async fn register_missing_metadata_fields(
        &mut self,
        tenant_uuid: &Uuid,
        metadata: &std::collections::HashMap<String, serde_json::Value>,
        declared_types: Option<&std::collections::HashMap<String, String>>,
        field_type_map: &mut std::collections::HashMap<String, String>,
    ) {
        let mut keys: Vec<&String> = metadata.keys().collect();
        keys.sort();
        for key in keys {
            if field_type_map.contains_key(key) {
                continue;
            }
            let value = &metadata[key];
            let register_type = Self::new_field_type(key, value, declared_types);
            if Self::coerce_value_to_type(value, register_type).is_none() {
                continue;
            }
            match self
                .create_metadata_field(&tenant_uuid.to_string(), key, Some(register_type))
                .await
            {
                Ok(_) => {
                    debug!(
                        "Registered new metadata field '{}' as {}",
                        key, register_type
                    );
                    field_type_map.insert(key.clone(), register_type.to_string());
                }
                Err(e) => debug!("Failed to register metadata field '{}': {}", key, e),
            }
        }
    }

    /// Delete specific metadata fields from an asset
    ///
    /// This method deletes specific metadata fields from the specified asset.
    /// The metadata keys are sent as a direct array in the request body.
    ///
    /// # Arguments
    /// Delete specific metadata fields from an asset
    ///
    /// This method deletes specific metadata fields from the specified asset.
    /// The metadata keys are sent as an object with a "metadataFieldNames" array.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant that owns the asset
    /// * `asset_id` - The UUID of the asset to update
    /// * `metadata_keys` - A vector of metadata field names to delete
    ///
    /// # Returns
    /// * `Ok(())` - Successfully deleted metadata from the asset
    /// * `Err(ApiError)` - HTTP error or other error occurred
    pub async fn delete_asset_metadata(
        &mut self,
        tenant_id: &str,
        asset_id: &str,
        metadata_keys: Vec<&str>,
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/tenants/{}/assets/{}/metadata",
            self.base_url, tenant_id, asset_id
        );

        // Send metadata keys wrapped in "metadataFieldNames" object as required by API
        let body = serde_json::json!({
            "metadataFieldNames": metadata_keys
        });

        // Log the request body for debugging
        debug!(
            "Deleting asset metadata with JSON body: {}",
            serde_json::to_string_pretty(&body)
                .unwrap_or_else(|_| "Unable to serialize body".to_string())
        );

        self.delete_with_body(&url, &body).await
    }

    /// * `field_name` - The name of the metadata field to create
    /// * `field_type` - The type of the metadata field (e.g., "text", "number", "boolean") - defaults to "text"
    ///
    /// # Returns
    /// * `Ok(serde_json::Value)` - Response from the API confirming the field was created
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn create_metadata_field(
        &mut self,
        tenant_id: &str,
        field_name: &str,
        field_type: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/tenants/{}/metadata-fields", self.base_url, tenant_id);

        let effective_type = field_type.unwrap_or("text");
        let body = serde_json::json!({
            "name": field_name,
            "type": effective_type
        });

        self.post(&url, &body).await
    }

    /// Get all metadata fields for a tenant
    ///
    /// This method retrieves the list of all metadata fields defined for the specified
    /// tenant, walking every page of the paginated endpoint. Without explicit
    /// pagination the API returns only the first page (20 fields), which previously
    /// made fields beyond that page look unregistered.
    ///
    /// # Arguments
    /// * `tenant_id` - The ID of the tenant
    ///
    /// # Returns
    /// * `Ok(MetadataFieldListResponse)` - All metadata fields for the tenant
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_metadata_fields(
        &mut self,
        tenant_id: &str,
    ) -> Result<crate::model::MetadataFieldListResponse, ApiError> {
        let mut page: usize = 1;
        let per_page: usize = 1000; // the API maximum for this endpoint
        let mut metadata_fields: Vec<crate::model::MetadataField> = Vec::new();

        loop {
            let url = format!(
                "{}/tenants/{}/metadata-fields?page={}&perPage={}",
                self.base_url, tenant_id, page, per_page
            );
            let response: crate::model::MetadataFieldListResponse = self.get(&url).await?;
            metadata_fields.extend(response.metadata_fields);

            match response.page_data {
                Some(page_data) if page_data.current_page < page_data.last_page => {
                    page = page_data.current_page + 1;
                }
                // No pagination info means the endpoint returned everything at once.
                _ => break,
            }
        }

        Ok(crate::model::MetadataFieldListResponse {
            metadata_fields,
            page_data: None,
        })
    }

    /// The assets that carry a value for a metadata field.
    ///
    /// `GET /tenants/{tenantId}/metadata-fields/{fieldId}/assets`, every page
    /// unless `limit` stops it early.
    pub async fn list_assets_using_metadata_field(
        &mut self,
        tenant_uuid: &Uuid,
        field_id: &Uuid,
        limit: Option<usize>,
    ) -> Result<AssetList, ApiError> {
        let url = format!(
            "{}/tenants/{}/metadata-fields/{}/assets",
            self.base_url, tenant_uuid, field_id
        );
        self.collect_asset_pages(&url, limit).await
    }

    /// The assets that have no metadata value at all, oldest first.
    ///
    /// `GET /tenants/{tenantId}/assets/without-metadata`; `folders` and
    /// `extensions` narrow it (comma-separated on the wire). Demo assets
    /// uploaded by Physna are excluded by the server.
    pub async fn list_assets_without_metadata(
        &mut self,
        tenant_uuid: &Uuid,
        folders: &[String],
        extensions: &[String],
        limit: Option<usize>,
    ) -> Result<AssetList, ApiError> {
        let mut url = format!(
            "{}/tenants/{}/assets/without-metadata",
            self.base_url, tenant_uuid
        );
        let mut query: Vec<String> = Vec::new();
        if !folders.is_empty() {
            query.push(format!(
                "folders={}",
                urlencoding::encode(&folders.join(","))
            ));
        }
        if !extensions.is_empty() {
            query.push(format!(
                "extensions={}",
                urlencoding::encode(&extensions.join(","))
            ));
        }
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query.join("&"));
        }
        self.collect_asset_pages(&url, limit).await
    }

    /// How many of the tenant's assets carry at least one metadata value.
    pub async fn get_metadata_coverage(
        &mut self,
        tenant_uuid: &Uuid,
    ) -> Result<crate::model::MetadataCoverageResponse, ApiError> {
        let url = format!(
            "{}/tenants/{}/metadata-coverage",
            self.base_url, tenant_uuid
        );
        self.get(&url).await
    }

    /// Rename a metadata field. `PATCH /tenants/{tenantId}/metadata-fields/{fieldId}`, 204.
    pub async fn rename_metadata_field(
        &mut self,
        tenant_uuid: &Uuid,
        field_id: &Uuid,
        new_name: &str,
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/tenants/{}/metadata-fields/{}",
            self.base_url, tenant_uuid, field_id
        );
        debug!("Renaming metadata field {} to '{}'", field_id, new_name);
        self.patch_no_response(&url, &serde_json::json!({ "name": new_name }))
            .await
    }

    /// Delete a metadata field. `DELETE /tenants/{tenantId}/metadata-fields/{fieldId}`, 204.
    ///
    /// Without `force` the server refuses a field that assets still use; with
    /// it the field goes and its values are removed from every asset.
    pub async fn delete_metadata_field(
        &mut self,
        tenant_uuid: &Uuid,
        field_id: &Uuid,
        force: bool,
    ) -> Result<(), ApiError> {
        debug!("Deleting metadata field {} (force: {})", field_id, force);
        self.delete(&format!(
            "/tenants/{}/metadata-fields/{}?force={}",
            tenant_uuid, field_id, force
        ))
        .await
    }
}

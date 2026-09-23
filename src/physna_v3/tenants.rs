//! Users and tenants.

use super::*;

impl PhysnaApiClient {
    /// Get the current user's information from the Physna V3 API
    ///
    /// This method fetches information about the currently authenticated user,
    /// including their tenant settings and other user-specific configuration.
    /// The response contains the user's profile information and available tenants.
    ///
    /// # Returns
    /// * `Ok(CurrentUserResponse)` - Successfully fetched current user information
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_current_user(&mut self) -> Result<CurrentUserResponse, ApiError> {
        let url = format!("{}/users/me", self.base_url);
        self.get(&url).await
    }

    /// List all available tenants for the current user
    ///
    /// This method fetches all tenants available to the currently authenticated user.
    /// Tenants represent different organizations or environments that the user has access to.
    /// Each tenant has its own set of folders, assets, and configurations.
    ///
    /// # Returns
    /// * `Ok(Vec<TenantSetting>)` - Successfully fetched list of available tenants
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn list_tenants(&mut self) -> Result<Vec<crate::model::TenantSetting>, ApiError> {
        let user = self.get_current_user().await?;
        Ok(user.user.settings)
    }

    /// List users in a specific tenant with pagination support
    ///
    /// This method fetches a list of users in the specified tenant.
    /// It supports pagination through the optional `page` and `per_page` parameters.
    /// The method implements safeguards to prevent infinite loops during pagination.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant whose users to list
    ///
    /// # Returns
    /// * `Ok(UserListResponse)` - Successfully fetched list of users with pagination metadata
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn list_tenant_users(
        &mut self,
        tenant_uuid: &Uuid,
    ) -> Result<crate::actions::users::UserListResponse, ApiError> {
        let per_page: usize = 100; // Reasonable page size for user listings
        let mut all_users: Vec<crate::actions::users::User> = Vec::new();
        // It used to stop at 50 pages (5,000 users) without a word.
        let mut pager = crate::paging::Pager::new("user listing");

        loop {
            let page = pager.page();
            let url = format!(
                "{}/tenants/{}/users?{}",
                self.base_url,
                tenant_uuid,
                serde_urlencoded::to_string([
                    ("page", page.to_string()),
                    ("perPage", per_page.to_string()),
                ])
                .unwrap()
            );
            debug!("Fetching users page {} for tenant {}", page, tenant_uuid);

            let mut response: crate::actions::users::UserListResponse = self.get(&url).await?;
            all_users.extend(std::mem::take(&mut response.users));

            // Without pagination data the endpoint returned everything at once.
            let Some(page_data) = &response.page_data else {
                break;
            };
            if !pager.advance(page_data.current_page, page_data.last_page, all_users.len()) {
                break;
            }
        }

        // Create a response with all users and combined pagination data
        let total_users = all_users.len();
        let final_response = crate::actions::users::UserListResponse {
            users: all_users,
            page_data: Some(crate::model::PageData {
                current_page: 1,
                per_page,
                total: total_users,
                last_page: ((total_users as f64) / (per_page as f64)).ceil() as usize,
                start_index: 0,
                end_index: total_users,
            }),
        };

        Ok(final_response)
    }

    /// Get details for a specific user
    ///
    /// This method fetches details for a specific user by their ID.
    ///
    /// # Arguments
    /// * `tenant_uuid` - The UUID of the tenant where the user belongs
    /// * `user_id` - The ID of the user to retrieve
    ///
    /// # Returns
    /// * `Ok(User)` - Successfully fetched user details
    /// * `Err(ApiError)` - HTTP error or JSON parsing error
    pub async fn get_user(
        &mut self,
        tenant_uuid: &Uuid,
        user_id: &str,
    ) -> Result<crate::actions::users::User, ApiError> {
        let url = format!(
            "{}/tenants/{}/users/{}",
            self.base_url,
            tenant_uuid,
            urlencoding::encode(user_id)
        );

        debug!(
            "Fetching user details for user ID: {} in tenant: {}",
            user_id, tenant_uuid
        );

        // Execute GET request to fetch the user response
        let response: crate::actions::users::SingleUserResponse = self.get(&url).await?;

        // Return the user from the response
        Ok(response.user)
    }
}

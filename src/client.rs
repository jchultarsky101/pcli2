use std::time::Duration;

use crate::configuration::TenantConfiguration;
use base64::{engine::general_purpose, Engine};
use log::trace;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to obtain access token from provider")]
    FailedToObtainToken,
    #[error("invalid client ID in configuration")]
    InvalidClientId,
    #[error("invalid client secret in configuration")]
    InvalidClientSecret,
    #[error("invalid tenant ID in configuration")]
    InvalidTenantId,
    #[error("error during HTTP request")]
    HttpError(#[from] reqwest::Error),
    #[error("unexpected response from server: {0}")]
    UnexpectedResponse(StatusCode),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct AuthenticationResponse {
    token_type: String, //e.g. "Bearer"
    expires_in: u64,    //e.g. 36000
    access_token: String,
    scope: String, //e.g. "tenantApp"
}

pub struct PhysnaHttpClient {
    tenant_configuration: TenantConfiguration,
}

impl PhysnaHttpClient {
    pub fn new(tenant_configuration: TenantConfiguration) -> PhysnaHttpClient {
        PhysnaHttpClient {
            tenant_configuration,
        }
    }

    pub fn request_new_token_from_provider(
        &self,
        client_secret: String,
    ) -> Result<String, ClientError> {
        let tenant = self.tenant_configuration.tenant_id();
        let client_id = self.tenant_configuration.client_id();

        trace!(
            "Requesting new token from provider for tenant {}...",
            &tenant
        );

        if tenant.is_empty() {
            return Err(ClientError::InvalidTenantId);
        }

        if client_id.is_empty() {
            return Err(ClientError::InvalidClientId);
        }

        if client_secret.is_empty() {
            return Err(ClientError::InvalidClientSecret);
        }

        // 0. Encode Base64: clientId + ":" + clientSecret
        // 1. Set the headers
        // "Authorization", "Basic " + encodedCredentials
        // "cache-control", "no-cache"
        // "scope", "tenantApp"
        // 2. Prepare multi value request body:
        // "grant_type", "client_credentials"
        // "scope", "tenantApp"
        // 3. POST to the provider URL

        // Example:
        /*
            curl --request POST --url https://physna.okta.com/oauth2/default/v1/token \
            --header 'accept: application/json' \
            --header 'authorization: Basic MG9h...' \
            --header 'cache-control: no-cache' \
            --header 'content-type: application/x-www-form-urlencoded' \
            --data 'grant_type=client_credentials&scope=tenantApp roles'
        */

        let combined_credentials = [client_id.clone(), client_secret.clone()]
            .join(":")
            .to_owned();
        let encoded_credentials = general_purpose::STANDARD.encode(combined_credentials.to_owned());
        let mut authorization_header_value = String::from("Basic ");
        authorization_header_value.push_str(encoded_credentials.as_str());

        let params = [
            ("grant_type", "client_credentials"),
            ("scope", "tenantApp roles"),
        ];

        // Create the HTTP client instance
        //let client = reqwest::Client::new();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;

        let url = self.tenant_configuration.oidc_url();
        let response = client
            .post(url)
            .header("Authorization", authorization_header_value.as_str())
            .header("cache-control", "no-cache")
            .form(&params)
            .send();

        match response {
            Ok(response) => {
                let status = response.status();

                if status == StatusCode::OK {
                    let response_text = response.text();
                    match response_text {
                        Ok(response_text) => {
                            let response: AuthenticationResponse =
                                serde_yaml::from_str(&response_text).unwrap();
                            let token = response.access_token;
                            Ok(token)
                        }
                        Err(_) => Err(ClientError::UnexpectedResponse(status)),
                    }
                } else {
                    Err(ClientError::UnexpectedResponse(status))
                }
            }
            Err(_) => Err(ClientError::FailedToObtainToken),
        }
    }
}

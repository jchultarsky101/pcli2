use std::time::Duration;

use crate::{
    configuration::TenantConfiguration,
    model::FolderList,
    security::{SecurityError, TenantSession},
};
use base64::{engine::general_purpose, Engine};
use log::trace;
use reqwest::{
    self,
    blocking::{
        multipart::{Form, Part},
        Client,
    },
    StatusCode,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

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
    #[error("parse error: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Folder {
    #[serde(rename = "id")]
    pub id: u32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "ownerId", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(rename = "name")]
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct FolderListResponse {
    #[serde(rename = "folders")]
    pub folders: Vec<Folder>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
struct AuthenticationResponse {
    token_type: String, //e.g. "Bearer"
    expires_in: u64,    //e.g. 36000
    access_token: String,
    scope: String, //e.g. "tenantApp"
}

pub struct PhysnaHttpClient {
    tenant_configuration: TenantConfiguration,
    client: Client,
}

impl PhysnaHttpClient {
    pub fn new(tenant_configuration: TenantConfiguration) -> Result<PhysnaHttpClient, ClientError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap();

        Ok(PhysnaHttpClient {
            tenant_configuration,
            client,
        })
    }

    fn evaluate_satus(&self, status: StatusCode) -> Result<(), ClientError> {
        if status.is_success() {
            ()
        }

        match status {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NON_AUTHORITATIVE_INFORMATION
            | StatusCode::NO_CONTENT
            | StatusCode::RESET_CONTENT => (), // Nothing to do, continue
            StatusCode::FORBIDDEN => return Err(ClientError::Forbidden),
            StatusCode::NOT_FOUND => return Err(ClientError::NotFound),
            StatusCode::UNAUTHORIZED => return Err(ClientError::Unauthorized),
            StatusCode::CONTINUE
            | StatusCode::SWITCHING_PROTOCOLS
            | StatusCode::PROCESSING
            | StatusCode::PARTIAL_CONTENT
            | StatusCode::MULTI_STATUS
            | StatusCode::ALREADY_REPORTED
            | StatusCode::IM_USED
            | StatusCode::MULTIPLE_CHOICES
            | StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::NOT_MODIFIED
            | StatusCode::USE_PROXY
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
            | StatusCode::BAD_REQUEST
            | StatusCode::PAYMENT_REQUIRED
            | StatusCode::METHOD_NOT_ALLOWED
            | StatusCode::NOT_ACCEPTABLE
            | StatusCode::PROXY_AUTHENTICATION_REQUIRED
            | StatusCode::REQUEST_TIMEOUT
            | StatusCode::CONFLICT
            | StatusCode::GONE
            | StatusCode::LENGTH_REQUIRED
            | StatusCode::PRECONDITION_FAILED
            | StatusCode::PAYLOAD_TOO_LARGE
            | StatusCode::URI_TOO_LONG
            | StatusCode::UNSUPPORTED_MEDIA_TYPE
            | StatusCode::RANGE_NOT_SATISFIABLE
            | StatusCode::EXPECTATION_FAILED
            | StatusCode::IM_A_TEAPOT
            | StatusCode::MISDIRECTED_REQUEST
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::LOCKED
            | StatusCode::FAILED_DEPENDENCY
            | StatusCode::UPGRADE_REQUIRED
            | StatusCode::PRECONDITION_REQUIRED
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
            | StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::NOT_IMPLEMENTED
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::HTTP_VERSION_NOT_SUPPORTED
            | StatusCode::VARIANT_ALSO_NEGOTIATES
            | StatusCode::INSUFFICIENT_STORAGE
            | StatusCode::LOOP_DETECTED
            | StatusCode::NOT_EXTENDED
            | StatusCode::NETWORK_AUTHENTICATION_REQUIRED => {
                return Err(ClientError::Unsupported(format!("Status: {:?}", status)))
            }
            _ => {
                return Err(ClientError::Unsupported(
                    "Unexpected query status code".to_string(),
                ))
            }
        };

        Ok(())
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

    fn get(
        &self,
        url: &str,
        session: &mut TenantSession,
        query_parameters: Option<Vec<(String, String)>>,
    ) -> Result<String, ClientError> {
        let token = match session.token() {
            Some(token) => token,
            None => match TenantSession::login(self.tenant_configuration.clone()) {
                Ok(new_session) => match new_session.token() {
                    Some(token) => {
                        session.set_token(token.clone());
                        token
                    }
                    None => return Err(ClientError::FailedToObtainToken),
                },
                Err(_) => return Err(ClientError::FailedToObtainToken),
            },
        };

        let mut builder = self
            .client
            .request(reqwest::Method::GET, url)
            .timeout(Duration::from_secs(180))
            .header(reqwest::header::USER_AGENT, APP_USER_AGENT)
            .header("X-PHYSNA-TENANTID", self.tenant_configuration.tenant_id());

        match query_parameters {
            Some(query_parametes) => {
                for (key, value) in query_parametes {
                    builder = builder.query(&[(key.to_owned(), value.to_owned())]);
                }
            }
            None => (),
        }

        let request = builder.bearer_auth(token).build()?;
        trace!("GET {}", request.url());
        let response = self.client.execute(request)?;

        trace!("Status: {}", response.status());

        self.evaluate_satus(response.status())?;

        let content = response.text()?;
        trace!("{}", content);
        Ok(content)
    }

    pub fn get_list_of_folders(
        &self,
        session: &mut TenantSession,
    ) -> Result<Vec<Folder>, ClientError> {
        trace!("Reading list of folders...");
        let url = format!("{}/v2/folders", self.tenant_configuration.api_url());

        let json = self.get(url.as_str(), session, None)?;

        trace!("{}", json);
        let response: FolderListResponse = serde_json::from_str(&json)?;

        Ok(response.folders)
    }
}

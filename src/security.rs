use super::browser;
use super::configuration::TenantConfiguration;
use anyhow::Result;
use log::trace;
use reqwest::{
    blocking::Client,
    header::{HeaderMap, ACCEPT, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct DeviceVerificationCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: Url,
    verification_uri_complete: Url,
    expires_in: usize,
    interval: usize,
}

impl DeviceVerificationCodeResponse {
    fn qrcode_url(&self) -> Url {
        self.verification_uri_complete.to_owned()
    }

    fn user_code(&self) -> String {
        self.user_code.to_owned()
    }

    fn verification_uri(&self) -> Url {
        self.verification_uri.to_owned()
    }
}

pub fn login(tenant: &TenantConfiguration) -> Result<()> {
    trace!("Logging in for tenant {}...", tenant.get_tenant_id());

    // let url = tenant.get_oidc_url();
    // let client_id = tenant.get_client_id();
    // let client_secret = tenant.get_client_secret();
    // let device_auth_url = DeviceAuthorizationUrl::from_url(url);

    let client_id = "0oa8105ceeNIB0RUT5d7";
    let okta_app_domain = "dev-11356524.okta.com";
    let device_auth_uri = format!(
        "https://{}/oauth2/default/v1/device/authorize",
        okta_app_domain
    );
    let device_auth_url = Url::parse(device_auth_uri.as_str())?;

    trace!("Creating HTTP client...");
    let client = Client::new();
    trace!("Client instance created.");

    // step 1: obtain a verification code
    /*
    Example:

    curl --request POST \
      --url https://dev-11356524.okta.com/oauth2/default/v1/device/authorize \
      --header 'Accept: application/json' \
      --header 'Content-Type: application/x-www-form-urlencoded' \
      --data-urlencode 'client_id=0oa8105ceeNIB0RUT5d7' \
      --data-urlencode 'scope=openid profile offline_access'
    */

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, "pcli2".parse().unwrap());
    headers.insert(ACCEPT, "application/json".parse().unwrap());

    let params = [
        ("client_id", client_id),
        ("scope", "openid profile offline_access"),
    ];

    trace!(
        "Sending a POST request to {}...",
        device_auth_url.to_string()
    );
    let response = client
        .post(device_auth_url)
        .headers(headers)
        .form(&params)
        .send()?;
    trace!("Response received.");

    trace!("Analyzing the response...");
    if response.status().is_success() {
        let device_verification: DeviceVerificationCodeResponse = response.json()?;
        let qrcode_url = device_verification.qrcode_url();
        let user_code = device_verification.user_code();
        let verification_uri = device_verification.verification_uri();

        trace!("Verification URI: {}", verification_uri.to_string());
        trace!("QRCode: {}", qrcode_url.to_string());
        trace!("User Code: {}", user_code);

        // step 2: navigate to the verification URL and enter the user code there
        browser::open(&qrcode_url);
        //browser::display_url_as_qrcode(&verification_uri);
    } else {
        let status_code = response.status().as_u16();
        trace!("Status: {}", status_code);
        let text = response.text()?;
        trace!("Response text: {}", text);
    }

    trace!("Done.");

    Ok(())
}

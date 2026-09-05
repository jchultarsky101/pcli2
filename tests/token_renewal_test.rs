//! Token renewal against a mock API and a mock auth server.
//!
//! The API client renews its access token when a request comes back 401, when
//! it has credentials but no token yet, and it must do so exactly once when a
//! burst of concurrent requests all fail together. These are the behaviours a
//! multi-hour folder match depends on, and until now they were only covered by
//! reading the code.

use mockito::{Matcher, Mock, ServerGuard};
use pcli2::physna_v3::{ApiError, PhysnaApiClient};
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const FOLDER: &str = "11111111-1111-1111-1111-111111111111";

/// Keep the renewed token's persistence away from the developer's real
/// credentials file. Every test in this binary points at the same directory,
/// so the process-wide variable is set idempotently.
fn isolate() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pcli2-renewal-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("PCLI2_CONFIG_DIR", &dir);
    std::env::set_var("PCLI2_CACHE_DIR", dir.join("cache"));
    dir
}

fn folder_body() -> String {
    format!(
        r#"{{"folder":{{"id":"{FOLDER}","tenantId":"{TENANT}","name":"Existing","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","assetsCount":0,"foldersCount":0}}}}"#
    )
}

fn folder_path() -> String {
    format!("/tenants/{TENANT}/folders/{FOLDER}")
}

async fn folder_with_bearer(server: &mut ServerGuard, token: &str, status: usize) -> Mock {
    let body = if status == 200 {
        folder_body()
    } else {
        r#"{"message":"Unauthorized"}"#.to_string()
    };
    server
        .mock("GET", folder_path().as_str())
        .match_header("authorization", format!("Bearer {token}").as_str())
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect_at_least(1)
        .create_async()
        .await
}

async fn token_endpoint(server: &mut ServerGuard, status: usize, body: &str) -> Mock {
    server
        .mock("POST", "/token")
        .match_body(Matcher::Regex("grant_type=client_credentials".into()))
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await
}

const FRESH_TOKEN_BODY: &str =
    r#"{"access_token":"fresh-token","expires_in":3600,"token_type":"Bearer"}"#;

fn client(server: &ServerGuard) -> PhysnaApiClient {
    PhysnaApiClient::new()
        .with_base_url(server.url())
        .with_auth_url(format!("{}/token", server.url()))
        .with_client_credentials("client-id".into(), "client-secret".into())
}

#[tokio::test]
async fn a_401_renews_the_token_once_and_the_retry_carries_the_new_one() {
    let dir = isolate();
    let mut server = mockito::Server::new_async().await;
    let stale = folder_with_bearer(&mut server, "stale-token", 401).await;
    let fresh = folder_with_bearer(&mut server, "fresh-token", 200).await;
    let token = token_endpoint(&mut server, 200, FRESH_TOKEN_BODY)
        .await
        .expect(1);

    let mut client = client(&server).with_access_token("stale-token".into());
    let folder = client
        .get_folder(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(FOLDER).unwrap(),
        )
        .await
        .expect("the retry with the renewed token must succeed");
    assert_eq!(folder.name(), "Existing");

    stale.assert_async().await;
    fresh.assert_async().await;
    token.assert_async().await;
    assert_eq!(client.get_access_token().as_deref(), Some("fresh-token"));

    // The renewed token is persisted where PCLI2_CONFIG_DIR says, so the next
    // command starts with it instead of renewing again.
    let credentials = std::fs::read_to_string(dir.join("dev_credentials.json"))
        .expect("renewed token should be saved under PCLI2_CONFIG_DIR");
    assert!(credentials.contains("fresh-token"), "{credentials}");
}

#[tokio::test]
async fn a_burst_of_concurrent_401s_costs_one_renewal() {
    isolate();
    let mut server = mockito::Server::new_async().await;
    let _stale = folder_with_bearer(&mut server, "stale-token", 401).await;
    let fresh = folder_with_bearer(&mut server, "fresh-token", 200)
        .await
        .expect(8);
    let token = token_endpoint(&mut server, 200, FRESH_TOKEN_BODY)
        .await
        .expect(1);

    // Clones share the token slot and the renewal lock, exactly as the folder
    // match and batch upload tasks do.
    let template = client(&server).with_access_token("stale-token".into());
    let tenant = Uuid::parse_str(TENANT).unwrap();
    let folder = Uuid::parse_str(FOLDER).unwrap();
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let mut task_client = template.clone();
            tokio::spawn(async move { task_client.get_folder(&tenant, &folder).await })
        })
        .collect();
    for task in tasks {
        task.await.unwrap().expect("every task should recover");
    }

    fresh.assert_async().await;
    token.assert_async().await;
}

#[tokio::test]
async fn with_credentials_but_no_token_the_client_authenticates_before_the_first_request() {
    isolate();
    let mut server = mockito::Server::new_async().await;
    let fresh = folder_with_bearer(&mut server, "fresh-token", 200).await;
    let token = token_endpoint(&mut server, 200, FRESH_TOKEN_BODY)
        .await
        .expect(1);

    let mut client = client(&server);
    client
        .get_folder(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(FOLDER).unwrap(),
        )
        .await
        .expect("the first request should already carry a token");

    fresh.assert_async().await;
    token.assert_async().await;
}

#[tokio::test]
async fn a_failed_renewal_names_the_cause_and_keeps_the_old_token() {
    isolate();
    let mut server = mockito::Server::new_async().await;
    let _stale = folder_with_bearer(&mut server, "stale-token", 401).await;
    let token = token_endpoint(
        &mut server,
        400,
        r#"{"error":"invalid_client","error_description":"Client is disabled"}"#,
    )
    .await
    .expect(1);

    let mut client = client(&server).with_access_token("stale-token".into());
    let err = client
        .get_folder(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(FOLDER).unwrap(),
        )
        .await
        .expect_err("a rejected credential cannot be recovered from");

    match &err {
        ApiError::AuthError(message) => {
            assert!(
                message.contains("pcli2 auth login"),
                "should tell the user how to recover: {message}"
            );
            assert!(
                message.contains("Client is disabled"),
                "should carry the auth server's reason: {message}"
            );
        }
        other => panic!("expected AuthError, got {other:?}"),
    }
    assert!(err.is_authentication_failure());
    token.assert_async().await;
    // The stale token is not thrown away on a failed renewal: it may still be
    // good if the auth endpoint was the thing that hiccupped.
    assert_eq!(client.get_access_token().as_deref(), Some("stale-token"));
}

#[tokio::test]
async fn without_credentials_a_401_is_reported_and_no_renewal_is_attempted() {
    isolate();
    let mut server = mockito::Server::new_async().await;
    let _stale = folder_with_bearer(&mut server, "stale-token", 401).await;
    let token = token_endpoint(&mut server, 200, FRESH_TOKEN_BODY)
        .await
        .expect(0);

    let mut client = PhysnaApiClient::new()
        .with_base_url(server.url())
        .with_auth_url(format!("{}/token", server.url()))
        .with_access_token("stale-token".into());
    let err = client
        .get_folder(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(FOLDER).unwrap(),
        )
        .await
        .expect_err("nothing to renew with");
    assert!(err.is_authentication_failure(), "{err:?}");
    token.assert_async().await;
}

//! Resolving a missing dependency against a mock API.
//!
//! `asset resolve-dependency` sends `POST .../assets/{assetId}/resolve-dependency`
//! with the dependency path as the listing spelled it and the stand-in
//! asset's id, and expects an empty 204.

use mockito::Matcher;
use pcli2::physna_v3::{ApiError, PhysnaApiClient};
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ASSEMBLY: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const PART: &str = "aaaaaaaa-0000-0000-0000-000000000002";

#[tokio::test]
async fn the_request_carries_the_path_and_the_stand_in_and_succeeds_on_204() {
    let mut server = mockito::Server::new_async().await;
    let post = server
        .mock(
            "POST",
            format!("/tenants/{TENANT}/assets/{ASSEMBLY}/resolve-dependency").as_str(),
        )
        .match_body(Matcher::Json(serde_json::json!({
            "resolvedAssetId": PART,
            "dependencyPath": "Joye/ballvalve/Body01.par",
        })))
        .with_status(204)
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    client
        .resolve_asset_dependency(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ASSEMBLY).unwrap(),
            "Joye/ballvalve/Body01.par",
            &Uuid::parse_str(PART).unwrap(),
        )
        .await
        .unwrap();
    post.assert_async().await;
}

#[tokio::test]
async fn a_rejected_resolution_is_an_error_with_the_servers_message() {
    let mut server = mockito::Server::new_async().await;
    let _post = server
        .mock("POST", Matcher::Any)
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"dependencyPath is not a missing dependency"}"#)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let error = client
        .resolve_asset_dependency(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ASSEMBLY).unwrap(),
            "nope.par",
            &Uuid::parse_str(PART).unwrap(),
        )
        .await
        .unwrap_err();
    match error {
        ApiError::HttpStatus { status, message } => {
            assert_eq!(status, 400);
            assert!(message.contains("not a missing dependency"), "{message}");
        }
        other => panic!("expected an HTTP status error, got {other:?}"),
    }
}

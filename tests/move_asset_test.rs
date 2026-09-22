//! Moving an asset against a mock API.
//!
//! `asset move` sends `PATCH .../assets/{assetId}/folder` with the
//! destination folder's id, or `null` for the root, and gets the asset back
//! with its new path.

use mockito::Matcher;
use pcli2::physna_v3::PhysnaApiClient;
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ASSET: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const FOLDER: &str = "bbbbbbbb-0000-0000-0000-000000000001";

fn moved_body(path: &str) -> String {
    format!(
        r#"{{"asset":{{"id":"{ASSET}","tenantId":"{TENANT}","path":"{path}","type":"model",
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-09-22T00:00:00Z",
            "state":"finished","isAssembly":false,"metadata":{{"material":"steel"}}}}}}"#
    )
}

#[tokio::test]
async fn a_move_into_a_folder_sends_its_id_and_returns_the_new_path() {
    let mut server = mockito::Server::new_async().await;
    let patch = server
        .mock(
            "PATCH",
            format!("/tenants/{TENANT}/assets/{ASSET}/folder").as_str(),
        )
        .match_body(Matcher::Json(serde_json::json!({ "folderId": FOLDER })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(moved_body("Archive/part.stl"))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let moved = client
        .move_asset(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ASSET).unwrap(),
            Some(Uuid::parse_str(FOLDER).unwrap()),
        )
        .await
        .unwrap();

    assert_eq!(moved.uuid(), Uuid::parse_str(ASSET).unwrap());
    assert_eq!(moved.path(), "Archive/part.stl");
    patch.assert_async().await;
}

#[tokio::test]
async fn a_move_to_the_root_sends_null() {
    let mut server = mockito::Server::new_async().await;
    let patch = server
        .mock(
            "PATCH",
            format!("/tenants/{TENANT}/assets/{ASSET}/folder").as_str(),
        )
        .match_body(Matcher::Json(serde_json::json!({ "folderId": null })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(moved_body("part.stl"))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let moved = client
        .move_asset(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ASSET).unwrap(),
            None,
        )
        .await
        .unwrap();

    assert_eq!(moved.path(), "part.stl");
    patch.assert_async().await;
}

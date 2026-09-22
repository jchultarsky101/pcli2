//! Replacing an asset's file against a mock API.
//!
//! `asset create --override` sends the new file to
//! `PUT .../assets/{assetId}/file` when the asset exists. The request is a
//! multipart form with the file alone, the answer is the asset itself (same
//! UUID, state `indexing`), and a 409 keeps the server's own wording: it does
//! not mean "already exists" here, unlike on a new upload.

use mockito::Matcher;
use pcli2::physna_v3::{ApiError, PhysnaApiClient};
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ASSET: &str = "aaaaaaaa-0000-0000-0000-000000000001";

fn ids() -> (Uuid, Uuid) {
    (
        Uuid::parse_str(TENANT).unwrap(),
        Uuid::parse_str(ASSET).unwrap(),
    )
}

fn local_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"solid cube\nendsolid cube\n").unwrap();
    path
}

#[tokio::test]
async fn the_file_goes_out_as_a_multipart_put_and_the_asset_comes_back() {
    let mut server = mockito::Server::new_async().await;
    let (tenant, asset) = ids();
    let put = server
        .mock(
            "PUT",
            format!("/tenants/{TENANT}/assets/{ASSET}/file").as_str(),
        )
        .match_header(
            "content-type",
            Matcher::Regex("^multipart/form-data".into()),
        )
        .match_body(Matcher::Regex(r#"name="file"; filename="cube.stl""#.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"id":"{ASSET}","tenantId":"{TENANT}","path":"Parts/cube.stl","type":"model",
                "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-09-22T00:00:00Z",
                "state":"indexing","isAssembly":false,"metadata":{{"material":"steel"}}}}"#
        ))
        .expect(1)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let file = local_file(dir.path(), "cube.stl");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let replaced = client
        .replace_asset_file(&tenant, &asset, &file)
        .await
        .unwrap();

    assert_eq!(replaced.uuid(), asset);
    assert_eq!(replaced.path(), "Parts/cube.stl");
    assert_eq!(
        replaced.processing_status().map(String::as_str),
        Some("indexing")
    );
    put.assert_async().await;
}

#[tokio::test]
async fn a_conflict_keeps_the_servers_wording() {
    let mut server = mockito::Server::new_async().await;
    let (tenant, asset) = ids();
    let _put = server
        .mock("PUT", Matcher::Any)
        .with_status(409)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"Asset is still being processed"}"#)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let file = local_file(dir.path(), "cube.stl");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let error = client
        .replace_asset_file(&tenant, &asset, &file)
        .await
        .unwrap_err();

    match error {
        ApiError::ConflictError(message) => {
            assert!(
                !message.contains("Asset already exists"),
                "a replacement conflict must not be reported as a duplicate path: {message}"
            );
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[tokio::test]
async fn a_missing_local_file_is_refused_before_any_request() {
    let mut server = mockito::Server::new_async().await;
    let (tenant, asset) = ids();
    let never = server
        .mock("PUT", Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let error = client
        .replace_asset_file(&tenant, &asset, std::path::Path::new("/no/such/file.stl"))
        .await
        .unwrap_err();
    assert!(matches!(error, ApiError::PathNotFound(_)));
    never.assert_async().await;
}

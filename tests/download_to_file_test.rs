//! The streamed download writes through a temporary file and refuses an empty body.

use pcli2::physna_v3::{ApiError, PhysnaApiClient};
use uuid::Uuid;

#[tokio::test]
async fn whole_file_is_written_and_the_part_file_is_gone() {
    let mut server = mockito::Server::new_async().await;
    let tenant = Uuid::new_v4();
    let asset = Uuid::new_v4();
    let body: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    let _m = server
        .mock(
            "GET",
            format!("/tenants/{}/assets/{}/file", tenant, asset).as_str(),
        )
        .with_status(200)
        .with_body(body.clone())
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("nested").join("part.stl");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let written = client
        .download_asset_to_file(
            &tenant.to_string(),
            &asset.to_string(),
            Some("part.stl"),
            &dest,
        )
        .await
        .unwrap();

    assert_eq!(written, body.len() as u64);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert!(!dir.path().join("nested").join("part.stl.part").exists());
}

#[tokio::test]
async fn an_empty_body_is_an_error_and_leaves_nothing_behind() {
    let mut server = mockito::Server::new_async().await;
    let tenant = Uuid::new_v4();
    let asset = Uuid::new_v4();
    let _m = server
        .mock(
            "GET",
            format!("/tenants/{}/assets/{}/file", tenant, asset).as_str(),
        )
        .with_status(200)
        .with_body("")
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("empty.stl");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let result = client
        .download_asset_to_file(&tenant.to_string(), &asset.to_string(), None, &dest)
        .await;

    assert!(matches!(result, Err(ApiError::IoError(_))));
    assert!(!dest.exists());
    assert!(!dir.path().join("empty.stl.part").exists());
}

#[tokio::test]
async fn a_server_error_is_classified_and_nothing_is_written() {
    let mut server = mockito::Server::new_async().await;
    let tenant = Uuid::new_v4();
    let asset = Uuid::new_v4();
    let _m = server
        .mock(
            "GET",
            format!("/tenants/{}/assets/{}/file", tenant, asset).as_str(),
        )
        .with_status(500)
        .with_body(r#"{"message":"boom"}"#)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("x.stl");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let result = client
        .download_asset_to_file(&tenant.to_string(), &asset.to_string(), None, &dest)
        .await;

    match result {
        Err(ApiError::HttpStatus { status, message }) => {
            assert_eq!(status, 500);
            assert!(message.contains("boom"));
        }
        other => panic!("expected HttpStatus, got {:?}", other),
    }
    assert!(!dest.exists());
}

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

#[tokio::test]
async fn delete_asset_targets_the_api_base_url() {
    // Regression: the delete helper once sent the relative path as the whole URL.
    let mut server = mockito::Server::new_async().await;
    let tenant = Uuid::new_v4();
    let asset = Uuid::new_v4();
    let m = server
        .mock(
            "DELETE",
            format!("/tenants/{}/assets/{}", tenant, asset).as_str(),
        )
        .with_status(204)
        .expect(1)
        .create_async()
        .await;
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    client
        .delete_asset(&tenant.to_string(), &asset.to_string())
        .await
        .unwrap();
    m.assert_async().await;
}

/// A server that promises a body and drops the connection halfway through it on
/// the first request, then serves the whole file.
async fn flaky_file_server(
    body: Vec<u8>,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let n = counter.fetch_add(1, Ordering::SeqCst);
            let body = body.clone();
            tokio::spawn(async move {
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let cut = if n == 0 { body.len() / 3 } else { body.len() };
                let _ = socket.write_all(&body[..cut]).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (url, requests)
}

#[tokio::test]
async fn a_connection_dropped_mid_body_is_downloaded_again() {
    let body: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
    let (url, requests) = flaky_file_server(body.clone()).await;
    let tenant = Uuid::new_v4();
    let asset = Uuid::new_v4();

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("big.stl");
    let mut client = PhysnaApiClient::new().with_base_url(url);
    let written = client
        .download_asset_to_file(
            &tenant.to_string(),
            &asset.to_string(),
            Some("big.stl"),
            &dest,
        )
        .await
        .expect("the second attempt delivers the whole file");

    assert_eq!(written, body.len() as u64);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(!dir.path().join("big.stl.part").exists());
}

//! Paginated listings stop where they should and never skip or repeat records.

use mockito::Matcher;
use pcli2::physna_v3::PhysnaApiClient;
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";

fn asset(n: usize) -> serde_json::Value {
    serde_json::json!({
        "id": Uuid::from_u128(n as u128),
        "tenantId": TENANT,
        "path": format!("/a/{n}.stl"),
        "state": "finished",
        "isAssembly": false,
        "metadata": {}
    })
}

fn page(items: Vec<serde_json::Value>, current: usize, last: usize) -> String {
    serde_json::json!({
        "assets": items,
        "pageData": {"total": 0, "perPage": 0, "currentPage": current, "lastPage": last, "startIndex": 0, "endIndex": 0}
    })
    .to_string()
}

#[tokio::test]
async fn a_limit_keeps_one_page_size_for_the_whole_walk() {
    // --limit 1500 used to ask for page=2&perPage=500 after a first page of 1000:
    // records 501-1000 again, and never 1001-1500.
    let mut server = mockito::Server::new_async().await;
    let field = Uuid::from_u128(7);
    let path = format!("/tenants/{TENANT}/metadata-fields/{field}/assets");
    let first = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "1000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page((0..1000).map(asset).collect(), 1, 3))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "2".into()),
            Matcher::UrlEncoded("perPage".into(), "1000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page((1000..2000).map(asset).collect(), 2, 3))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let assets = client
        .list_assets_using_metadata_field(&Uuid::parse_str(TENANT).unwrap(), &field, Some(1500))
        .await
        .unwrap();
    assert_eq!(assets.len(), 1500);
    let paths: std::collections::HashSet<String> =
        assets.get_all_assets().iter().map(|a| a.path()).collect();
    assert_eq!(paths.len(), 1500, "no record may appear twice");
    assert!(paths.contains("/a/1499.stl"));
    first.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn a_folder_listing_that_repeats_a_page_stops() {
    // The folder tree walk had no guard: a server answering page 1 to every
    // request was asked for page 2, 3, ... forever.
    let cache = tempfile::tempdir().unwrap();
    std::env::set_var("PCLI2_CACHE_DIR", cache.path());
    let mut server = mockito::Server::new_async().await;
    let folders = server
        .mock("GET", format!("/tenants/{TENANT}/folders").as_str())
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({
                "folders": [{
                    "id": Uuid::from_u128(1), "tenantId": TENANT, "name": "A",
                    "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
                    "assetsCount": 0, "foldersCount": 0
                }],
                "pageData": {"total": 3, "perPage": 1, "currentPage": 1, "lastPage": 3, "startIndex": 0, "endIndex": 1}
            })
            .to_string(),
        )
        .expect(2)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.resolve_folder_uuid_by_path(&Uuid::parse_str(TENANT).unwrap(), "/A"),
    )
    .await
    .expect("the walk must stop");
    assert_eq!(result.unwrap(), Some(Uuid::from_u128(1)));
    folders.assert_async().await;
}

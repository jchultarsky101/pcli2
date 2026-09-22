//! Fetching several assets by id against a mock API.
//!
//! `asset get --uuid A --uuid B` sends one `POST .../assets/batch` request
//! (per 1000 ids), gets the assets back in the order asked, and can tell
//! which ids the tenant does not have.

use mockito::Matcher;
use pcli2::physna_v3::{missing_asset_ids, PhysnaApiClient};
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";

fn id(n: u32) -> Uuid {
    Uuid::parse_str(&format!("aaaaaaaa-0000-4000-8000-{:012}", n)).unwrap()
}

fn asset_json(uuid: Uuid, name: &str) -> String {
    format!(
        r#"{{"id":"{uuid}","tenantId":"{TENANT}","path":"Parts/{name}","type":"model",
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
            "state":"finished","isAssembly":false,"metadata":{{}}}}"#
    )
}

#[tokio::test]
async fn assets_come_back_in_request_order_and_absent_ids_are_reported() {
    let mut server = mockito::Server::new_async().await;
    // The server answers in its own order and knows nothing about id 3.
    let batch = server
        .mock("POST", format!("/tenants/{TENANT}/assets/batch").as_str())
        .match_body(Matcher::Regex(format!(
            r#""assetIds":\["{}","{}","{}"\]"#,
            id(1),
            id(2),
            id(3)
        )))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"assets":[{},{}]}}"#,
            asset_json(id(2), "b.stl"),
            asset_json(id(1), "a.stl")
        ))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let tenant = Uuid::parse_str(TENANT).unwrap();
    // id 1 twice: asked about once, returned once.
    let requested = [id(1), id(2), id(3), id(1)];
    let assets = client.get_assets_batch(&tenant, &requested).await.unwrap();

    let names: Vec<String> = assets.iter().map(|a| a.name().to_string()).collect();
    assert_eq!(names, ["a.stl", "b.stl"]);
    assert_eq!(missing_asset_ids(&requested, &assets), vec![id(3)]);
    batch.assert_async().await;
}

#[tokio::test]
async fn more_than_a_thousand_ids_go_out_in_chunks() {
    let mut server = mockito::Server::new_async().await;
    let path = format!("/tenants/{TENANT}/assets/batch");
    let first = server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex(format!(r#""{}""#, id(0))))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"assets":[{}]}}"#,
            asset_json(id(0), "first.stl")
        ))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex(format!(r#""{}""#, id(1000))))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"assets":[{}]}}"#,
            asset_json(id(1000), "last.stl")
        ))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let tenant = Uuid::parse_str(TENANT).unwrap();
    let requested: Vec<Uuid> = (0..1001).map(id).collect();
    let assets = client.get_assets_batch(&tenant, &requested).await.unwrap();

    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0].name(), "first.stl");
    assert_eq!(assets[1].name(), "last.stl");
    first.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn an_empty_list_makes_no_request() {
    let mut server = mockito::Server::new_async().await;
    let never = server
        .mock("POST", Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let assets = client
        .get_assets_batch(&Uuid::parse_str(TENANT).unwrap(), &[])
        .await
        .unwrap();
    assert!(assets.is_empty());
    never.assert_async().await;
}

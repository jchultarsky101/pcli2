//! The existing-paths check against a mock API.
//!
//! `find_existing_asset_paths` backs `--skip-existing`. It must send at most
//! 1000 paths per request (the specification's maximum), return the union of
//! what every request answered, and make no request at all for an empty batch.

use mockito::Matcher;
use pcli2::actions::utils::asset_path_for;
use pcli2::physna_v3::PhysnaApiClient;
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";

fn tenant() -> Uuid {
    Uuid::parse_str(TENANT).unwrap()
}

#[tokio::test]
async fn a_large_batch_is_sent_in_chunks_of_one_thousand_and_the_answers_are_merged() {
    let mut server = mockito::Server::new_async().await;
    let path = format!("/tenants/{TENANT}/assets/existing-paths");
    let first = server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex(r#""/Parts/file-0\.stl""#.to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"existingPaths":["/Parts/file-7.stl"]}"#)
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex(r#""/Parts/file-1000\.stl""#.to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"existingPaths":["/Parts/file-1000.stl"]}"#)
        .expect(1)
        .create_async()
        .await;

    let paths: Vec<String> = (0..1001).map(|i| format!("/Parts/file-{i}.stl")).collect();
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let existing = client
        .find_existing_asset_paths(&tenant(), &paths)
        .await
        .unwrap();

    assert_eq!(existing.len(), 2);
    assert!(existing.contains("/Parts/file-7.stl"));
    assert!(existing.contains("/Parts/file-1000.stl"));
    first.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn an_empty_batch_makes_no_request() {
    let mut server = mockito::Server::new_async().await;
    let never = server
        .mock("POST", Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let existing = client
        .find_existing_asset_paths(&tenant(), &[])
        .await
        .unwrap();
    assert!(existing.is_empty());
    never.assert_async().await;
}

#[tokio::test]
async fn a_failed_check_is_an_error_not_an_empty_answer() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("POST", Matcher::Any)
        .with_status(500)
        .with_body("boom")
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let result = client
        .find_existing_asset_paths(&tenant(), &["/Parts/a.stl".to_string()])
        .await;
    assert!(
        result.is_err(),
        "a server error must not read as 'nothing exists'"
    );
}

#[test]
fn asset_paths_are_built_in_one_spelling() {
    assert_eq!(asset_path_for("/", "a.stl"), "/a.stl");
    assert_eq!(asset_path_for("", "a.stl"), "/a.stl");
    assert_eq!(asset_path_for("/Home", "a.stl"), "/a.stl");
    assert_eq!(asset_path_for("/Parts", "a.stl"), "/Parts/a.stl");
    assert_eq!(asset_path_for("Parts/", "a.stl"), "/Parts/a.stl");
    assert_eq!(
        asset_path_for("/Home/Parts/Sub", "a.stl"),
        "/Parts/Sub/a.stl"
    );
    assert_eq!(
        asset_path_for("/Parts//Sub/", "a b.stl"),
        "/Parts/Sub/a b.stl"
    );
}

//! Metadata-field management against a mock API.
//!
//! `tenant metadata rename|delete|assets|coverage|missing` address a field by
//! the id the listing now carries, and read paged asset listings with the
//! same early stop as the other listings.

use mockito::Matcher;
use pcli2::model::MetadataCoverage;
use pcli2::physna_v3::PhysnaApiClient;
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const FIELD: &str = "7ac811ce-d4d2-4e26-95bd-59318edbad4d";

fn tenant() -> Uuid {
    Uuid::parse_str(TENANT).unwrap()
}

fn field() -> Uuid {
    Uuid::parse_str(FIELD).unwrap()
}

fn assets_page(names: &[&str], page: usize, last_page: usize) -> String {
    let items: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            format!(
                r#"{{"id":"aaaaaaaa-0000-4000-8000-{:012}","tenantId":"{TENANT}","path":"Parts/{n}","type":"model",
                    "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
                    "state":"finished","isAssembly":false,"metadata":{{}}}}"#,
                page * 100 + i
            )
        })
        .collect();
    format!(
        r#"{{"assets":[{}],"pageData":{{"total":3,"perPage":2,"currentPage":{page},"lastPage":{last_page},"startIndex":0,"endIndex":0}}}}"#,
        items.join(",")
    )
}

#[tokio::test]
async fn the_listing_carries_field_ids() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", format!("/tenants/{TENANT}/metadata-fields").as_str())
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"metadataFields":[{{"id":"{FIELD}","name":"Material","type":"text","tenantId":"{TENANT}",
                "createdAt":"2026-04-01T15:06:01.198Z","updatedAt":"2026-04-01T15:06:01.198Z",
                "creator":{{"id":"f2265d73-8cfc-4f07-9706-b7f1cf29046b","email":"x@example.com"}}}}],
                "pageData":{{"total":1,"perPage":1000,"currentPage":1,"lastPage":1,"startIndex":0,"endIndex":0}}}}"#
        ))
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let fields = client.get_metadata_fields(TENANT).await.unwrap();
    assert_eq!(fields.metadata_fields.len(), 1);
    assert_eq!(fields.metadata_fields[0].id, Some(field()));
    assert_eq!(fields.metadata_fields[0].name, "Material");
}

#[tokio::test]
async fn rename_patches_the_name_and_delete_sends_the_force_flag() {
    let mut server = mockito::Server::new_async().await;
    let patch = server
        .mock(
            "PATCH",
            format!("/tenants/{TENANT}/metadata-fields/{FIELD}").as_str(),
        )
        .match_body(Matcher::Json(
            serde_json::json!({ "name": "Material (new)" }),
        ))
        .with_status(204)
        .expect(1)
        .create_async()
        .await;
    let delete = server
        .mock(
            "DELETE",
            format!("/tenants/{TENANT}/metadata-fields/{FIELD}").as_str(),
        )
        .match_query(Matcher::UrlEncoded("force".into(), "true".into()))
        .with_status(204)
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    client
        .rename_metadata_field(&tenant(), &field(), "Material (new)")
        .await
        .unwrap();
    client
        .delete_metadata_field(&tenant(), &field(), true)
        .await
        .unwrap();
    patch.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn assets_using_a_field_are_paged_and_stop_at_the_limit() {
    let mut server = mockito::Server::new_async().await;
    let path = format!("/tenants/{TENANT}/metadata-fields/{FIELD}/assets");
    let _p1 = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "1000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(assets_page(&["a.stl", "b.stl"], 1, 2))
        .create_async()
        .await;
    let _p2 = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "2".into()),
            Matcher::UrlEncoded("perPage".into(), "1000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(assets_page(&["c.stl"], 2, 2))
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let all = client
        .list_assets_using_metadata_field(&tenant(), &field(), None)
        .await
        .unwrap();
    assert_eq!(all.get_all_assets().len(), 3);

    // With a limit of 1 only the first page is asked for, with perPage=1.
    let limited_page = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "1".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(assets_page(&["a.stl"], 1, 3))
        .expect(1)
        .create_async()
        .await;
    let one = client
        .list_assets_using_metadata_field(&tenant(), &field(), Some(1))
        .await
        .unwrap();
    assert_eq!(one.get_all_assets().len(), 1);
    limited_page.assert_async().await;
}

#[tokio::test]
async fn assets_without_metadata_send_the_filters_and_coverage_is_read() {
    let mut server = mockito::Server::new_async().await;
    let listing = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/without-metadata").as_str(),
        )
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("folders".into(), "Parts,Old Parts".into()),
            Matcher::UrlEncoded("extensions".into(), "stl,step".into()),
            Matcher::UrlEncoded("page".into(), "1".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(assets_page(&["bare.stl"], 1, 1))
        .expect(1)
        .create_async()
        .await;
    let _coverage = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/metadata-coverage").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"coveredAssets":16143,"totalAssets":23580}"#)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let missing = client
        .list_assets_without_metadata(
            &tenant(),
            &["Parts".to_string(), "Old Parts".to_string()],
            &["stl".to_string(), "step".to_string()],
            None,
        )
        .await
        .unwrap();
    assert_eq!(missing.get_all_assets().len(), 1);
    listing.assert_async().await;

    let coverage: MetadataCoverage = client
        .get_metadata_coverage(&tenant())
        .await
        .unwrap()
        .into();
    assert_eq!(coverage.covered_assets, 16143);
    assert_eq!(coverage.total_assets, 23580);
}

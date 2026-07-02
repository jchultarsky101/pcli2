//! Tests for metadata field listing pagination.
//!
//! The /tenants/{id}/metadata-fields endpoint is paginated (defaulting to 20
//! items per page). `get_metadata_fields` must walk every page; returning only
//! the first page previously made registered fields look unregistered, which
//! skipped type-mismatch checks and caused a doomed re-registration attempt
//! (HTTP 409) for every field beyond page one.

use mockito::Matcher;
use pcli2::physna_v3::PhysnaApiClient;

fn page_body(names: &[&str], current_page: usize, last_page: usize) -> String {
    let fields: Vec<String> = names
        .iter()
        .map(|n| format!(r#"{{"name":"{}","type":"text"}}"#, n))
        .collect();
    format!(
        r#"{{"metadataFields":[{}],"pageData":{{"total":3,"perPage":200,"currentPage":{},"lastPage":{},"startIndex":0,"endIndex":0}}}}"#,
        fields.join(","),
        current_page,
        last_page
    )
}

#[tokio::test]
async fn get_metadata_fields_walks_all_pages() {
    let mut server = mockito::Server::new_async().await;

    let page1 = server
        .mock("GET", "/tenants/t1/metadata-fields")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "200".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page_body(&["Material", "Weight"], 1, 2))
        .create_async()
        .await;

    let page2 = server
        .mock("GET", "/tenants/t1/metadata-fields")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "2".into()),
            Matcher::UrlEncoded("perPage".into(), "200".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page_body(&["Family Table Info"], 2, 2))
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let response = client.get_metadata_fields("t1").await.unwrap();

    page1.assert_async().await;
    page2.assert_async().await;

    let names: Vec<&str> = response
        .metadata_fields
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(names, vec!["Material", "Weight", "Family Table Info"]);
}

#[tokio::test]
async fn get_metadata_fields_handles_unpaginated_response() {
    let mut server = mockito::Server::new_async().await;

    // No pageData in the response: the endpoint returned everything at once,
    // so the client must not loop.
    let mock = server
        .mock("GET", "/tenants/t1/metadata-fields")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"metadataFields":[{"name":"Material","type":"text"}]}"#)
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let response = client.get_metadata_fields("t1").await.unwrap();

    mock.assert_async().await;
    assert_eq!(response.metadata_fields.len(), 1);
    assert_eq!(response.metadata_fields[0].name, "Material");
}

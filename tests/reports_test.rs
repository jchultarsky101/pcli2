//! The report client methods against a mock API.
//!
//! Listing sends the type and status filters and stops at `--limit`; a
//! create posts the request and reads the 201 answer; a download streams the
//! file through a `.part` file and refuses an empty body; delete and the
//! failure diagnostics use their endpoints.

use mockito::Matcher;
use pcli2::model::{
    CreateDuplicationReportRequest, FailureDiagnosticsStatus, JobStatus, ReportType,
};
use pcli2::physna_v3::{ApiError, PhysnaApiClient};
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const REPORT: &str = "f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1";

fn tenant() -> Uuid {
    Uuid::parse_str(TENANT).unwrap()
}

fn report() -> Uuid {
    Uuid::parse_str(REPORT).unwrap()
}

fn report_json(id: &str, status: &str) -> String {
    format!(
        r#"{{"id":"{id}","tenantId":"{TENANT}","status":"{status}","progress":100,"reportType":"DUPLICATION",
            "name":"dupes","minThreshold":80,"maxThreshold":100,"includeHomeFolderAssets":false,
            "creator":{{"id":"1b3cd922-6961-4aab-ac1c-82bb4c4816c3","email":"someone@example.com"}},
            "excludeAssemblies":false,"excludeExactDuplicates":false,
            "createdAt":"2026-09-21T18:33:10.382Z","updatedAt":"2026-09-21T18:33:11.254Z","groupCount":1}}"#
    )
}

#[tokio::test]
async fn the_listing_sends_the_filters_and_stops_at_the_limit() {
    let mut server = mockito::Server::new_async().await;
    let page = server
        .mock("GET", format!("/tenants/{TENANT}/reports").as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("type".into(), "DUPLICATION".into()),
            Matcher::UrlEncoded("status".into(), "COMPLETED".into()),
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "2".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"reports":[{},{}],"pageData":{{"total":5,"perPage":2,"currentPage":1,"lastPage":3,"startIndex":1,"endIndex":2}}}}"#,
            report_json(REPORT, "COMPLETED"),
            report_json("98e51614-3e5b-4eee-957b-fa19699535ce", "COMPLETED")
        ))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let reports = client
        .list_reports(&tenant(), Some("DUPLICATION"), Some("COMPLETED"), Some(2))
        .await
        .unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].id, REPORT);
    assert_eq!(reports[0].report_type, ReportType::Duplication);
    assert_eq!(reports[0].status, JobStatus::Completed);
    assert_eq!(reports[0].name.as_deref(), Some("dupes"));
    page.assert_async().await;
}

#[tokio::test]
async fn a_create_posts_the_request_and_reads_the_created_report() {
    let mut server = mockito::Server::new_async().await;
    let post = server
        .mock(
            "POST",
            format!("/tenants/{TENANT}/reports/duplication").as_str(),
        )
        .match_body(Matcher::Json(serde_json::json!({
            "name": "dupes",
            "minThreshold": 80.0,
            "maxThreshold": 100.0,
            "folderIds": ["3c91b897-0c8b-40f5-946d-6c3a867e7869"],
            "extensions": ["stl"],
            "excludeAssemblies": true,
            "excludeExactDuplicates": false,
            "includeHomeFolderAssets": false
        })))
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"report":{}}}"#,
            report_json(REPORT, "PENDING")
        ))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let created = client
        .create_duplication_report(
            &tenant(),
            &CreateDuplicationReportRequest {
                name: Some("dupes".to_string()),
                min_threshold: 80.0,
                max_threshold: 100.0,
                folder_ids: vec!["3c91b897-0c8b-40f5-946d-6c3a867e7869".to_string()],
                excluded_folder_ids: vec![],
                extensions: vec!["stl".to_string()],
                exclude_assemblies: true,
                exclude_exact_duplicates: false,
                include_home_folder_assets: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(created.id, REPORT);
    assert_eq!(created.status, JobStatus::Pending);
    post.assert_async().await;
}

#[tokio::test]
async fn a_download_is_written_through_a_part_file_and_an_empty_body_is_refused() {
    let mut server = mockito::Server::new_async().await;
    let body = b"ID,NAME\n1,a\n".to_vec();
    let _file = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/reports/{REPORT}/file").as_str(),
        )
        .match_query(Matcher::UrlEncoded("format".into(), "csv".into()))
        .with_status(200)
        .with_body(body.clone())
        .create_async()
        .await;
    let _empty = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/reports/{REPORT}/file").as_str(),
        )
        .match_query(Matcher::UrlEncoded("format".into(), "xlsx".into()))
        .with_status(200)
        .with_body("")
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("out").join("dupes.csv");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let written = client
        .download_report_to_file(&tenant(), &report(), "csv", &dest)
        .await
        .unwrap();
    assert_eq!(written, body.len() as u64);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert!(!dir.path().join("out").join("dupes.csv.part").exists());

    let empty_dest = dir.path().join("dupes.xlsx");
    let error = client
        .download_report_to_file(&tenant(), &report(), "xlsx", &empty_dest)
        .await
        .unwrap_err();
    assert!(matches!(error, ApiError::IoError(_)), "{error:?}");
    assert!(error.to_string().contains("empty file for report"));
    assert!(!empty_dest.exists());
    assert!(!dir.path().join("dupes.xlsx.part").exists());
}

#[tokio::test]
async fn get_delete_and_failure_diagnostics_use_their_endpoints() {
    let mut server = mockito::Server::new_async().await;
    let _get = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/reports/{REPORT}").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(r#"{{"report":{}}}"#, report_json(REPORT, "FAILED")))
        .create_async()
        .await;
    let delete = server
        .mock(
            "DELETE",
            format!("/tenants/{TENANT}/reports/{REPORT}").as_str(),
        )
        .with_status(204)
        .expect(1)
        .create_async()
        .await;
    let _diag = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/reports/{REPORT}/failure-diagnostics").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"found","kind":"internal","summary":"Processing failed inside Physna.","traceId":"t-1","occurredAt":"2026-09-22T14:16:24.033Z"}"#)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let fetched = client.get_report(&tenant(), &report()).await.unwrap();
    assert_eq!(fetched.status, JobStatus::Failed);

    let diagnostics = client
        .get_report_failure_diagnostics(&tenant(), &report())
        .await
        .unwrap();
    assert_eq!(diagnostics.status, FailureDiagnosticsStatus::Found);
    assert_eq!(diagnostics.trace_id.as_deref(), Some("t-1"));

    client.delete_report(&tenant(), &report()).await.unwrap();
    delete.assert_async().await;
}

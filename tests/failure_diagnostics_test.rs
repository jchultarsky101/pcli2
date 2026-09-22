//! The failure-diagnostics client calls against a mock API.
//!
//! `asset diagnose` turns the three `status` answers into three outcomes:
//! `found` and `not-found` are printed (exit 0), `unavailable` is exit 68 with
//! a message naming the cause. The exit-code mapping itself lives in
//! `CliError::exit_code` and is checked here without a binary, since the
//! command needs a configured tenant the tests do not have.

use mockito::Matcher;
use pcli2::error::CliError;
use pcli2::exit_codes::PcliExitCode;
use pcli2::model::{FailureDiagnosticsStatus, FailureKind, FailureSource};
use pcli2::physna_v3::PhysnaApiClient;
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ASSET: &str = "aaaaaaaa-0000-0000-0000-000000000001";

fn tenant() -> Uuid {
    Uuid::parse_str(TENANT).unwrap()
}

fn asset() -> Uuid {
    Uuid::parse_str(ASSET).unwrap()
}

#[tokio::test]
async fn a_found_failure_carries_every_field_the_spec_example_shows() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/{ASSET}/failure-diagnostics").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"kind":"user","occurredAt":"2026-08-26T19:03:16.000Z","status":"found",
                "summary":"The file format or version is not supported.",
                "traceId":"6f2b1c40-9f3a-4a1e-9a1b-2c7d4e5f6a7b"}"#,
        )
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let d = client
        .get_asset_failure_diagnostics(&tenant(), &asset())
        .await
        .unwrap();
    assert_eq!(d.status, FailureDiagnosticsStatus::Found);
    assert_eq!(d.kind, Some(FailureKind::User));
    assert_eq!(
        d.summary.as_deref(),
        Some("The file format or version is not supported.")
    );
    assert_eq!(
        d.trace_id.as_deref(),
        Some("6f2b1c40-9f3a-4a1e-9a1b-2c7d4e5f6a7b")
    );
    assert_eq!(d.occurred_at.as_deref(), Some("2026-08-26T19:03:16.000Z"));
}

#[tokio::test]
async fn a_status_only_answer_is_accepted() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/{ASSET}/failure-diagnostics").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"unavailable"}"#)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let d = client
        .get_asset_failure_diagnostics(&tenant(), &asset())
        .await
        .unwrap();
    assert_eq!(d.status, FailureDiagnosticsStatus::Unavailable);
    assert!(d.kind.is_none() && d.summary.is_none() && d.trace_id.is_none());
}

#[test]
fn an_unavailable_deployment_is_exit_68_and_names_the_cause() {
    let error = CliError::FeatureUnavailable(
        pcli2::actions::assets::diagnose::UNAVAILABLE_MESSAGE.to_string(),
    );
    assert_eq!(error.exit_code(), PcliExitCode::Unavailable);
    assert_eq!(error.exit_code().code(), 68);
    let message = error.to_string();
    assert!(message.contains("not available on this Physna deployment"));
    assert!(message.contains("log search"));
}

#[tokio::test]
async fn availability_is_a_plain_boolean() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/failure-diagnostics/availability").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"available":false}"#)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let a = client
        .get_failure_diagnostics_availability(&tenant())
        .await
        .unwrap();
    assert!(!a.available);
}

fn failures_page(page: usize, last_page: usize, names: &[&str]) -> String {
    let items: Vec<String> = names
        .iter()
        .map(|n| {
            format!(
                r#"{{"kind":"asset","id":"{ASSET}","name":"{n}","failedAt":"2026-09-20T10:00:00.000Z"}}"#
            )
        })
        .collect();
    format!(
        r#"{{"failures":[{}],"countsByKind":{{"asset":3,"report":0,"part-finder-report":0}},
            "pageData":{{"total":3,"perPage":2,"currentPage":{page},"lastPage":{last_page},"startIndex":0,"endIndex":0}}}}"#,
        items.join(",")
    )
}

#[tokio::test]
async fn the_failures_listing_walks_every_page_and_keeps_the_totals() {
    let mut server = mockito::Server::new_async().await;
    let path = format!("/tenants/{TENANT}/failures");
    let _p1 = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(failures_page(1, 2, &["a.stl", "b.stl"]))
        .create_async()
        .await;
    let _p2 = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "2".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(failures_page(2, 2, &["c.stl"]))
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let list = client
        .list_recent_failures(&tenant(), &[], None)
        .await
        .unwrap();
    let names: Vec<&str> = list.failures.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["a.stl", "b.stl", "c.stl"]);
    assert_eq!(list.counts_by_kind.asset, 3.0);
    assert_eq!(list.failures[0].kind, FailureSource::Asset);
}

#[tokio::test]
async fn the_failures_listing_sends_the_kinds_filter_and_stops_at_the_limit() {
    let mut server = mockito::Server::new_async().await;
    let path = format!("/tenants/{TENANT}/failures");
    // Only the first page is served: a limit of 1 must not ask for a second.
    let _p1 = server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "1".into()),
            Matcher::UrlEncoded("kinds".into(), "asset,report".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(failures_page(1, 3, &["a.stl"]))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let list = client
        .list_recent_failures(
            &tenant(),
            &[FailureSource::Asset, FailureSource::Report],
            Some(1),
        )
        .await
        .unwrap();
    assert_eq!(list.failures.len(), 1);
    _p1.assert_async().await;
}

//! `pcli2 api` end to end against a mock API.

mod common;

use common::{MockCli, TENANT};
use mockito::Matcher;

#[test]
fn a_get_prints_the_response_body() {
    let cli = MockCli::new();
    let output = cli.cmd().args(["api", "/users/me"]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["user"]["displayName"], "Test User");
}

#[test]
fn tenant_id_is_filled_in_and_paginate_merges_every_page() {
    let mut cli = MockCli::new();
    let path = format!("/tenants/{TENANT}/folders");
    let page = |n: usize, names: &[&str]| {
        serde_json::json!({
            "folders": names.iter().map(|n| serde_json::json!({"name": n})).collect::<Vec<_>>(),
            "pageData": {"total": 3, "perPage": 2, "currentPage": n, "lastPage": 2, "startIndex": 0, "endIndex": 0}
        })
        .to_string()
    };
    let first = cli
        .server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "1".into()),
            Matcher::UrlEncoded("perPage".into(), "2".into()),
        ]))
        .with_status(200)
        .with_body(page(1, &["A", "B"]))
        .create();
    let second = cli
        .server
        .mock("GET", path.as_str())
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("page".into(), "2".into()),
            Matcher::UrlEncoded("perPage".into(), "2".into()),
        ]))
        .with_status(200)
        .with_body(page(2, &["C"]))
        .create();

    let output = cli
        .cmd()
        .args(["api", "/tenants/{tenantId}/folders?perPage=2", "--paginate"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let names: Vec<&str> = body["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["A", "B", "C"]);
    assert!(body.get("pageData").is_none());
    first.assert();
    second.assert();
}

#[test]
fn fields_build_a_json_body_and_make_it_a_post() {
    let mut cli = MockCli::new();
    let endpoint = cli
        .server
        .mock(
            "POST",
            format!("/tenants/{TENANT}/assets/existing-paths").as_str(),
        )
        .match_body(Matcher::Json(serde_json::json!({
            "paths": ["/a.stl"],
            "limit": 5,
            "note": "5"
        })))
        .with_status(200)
        .with_body(r#"{"existingPaths":["/a.stl"]}"#)
        .create();
    let output = cli
        .cmd()
        .args([
            "api",
            "/tenants/{tenantId}/assets/existing-paths",
            "-F",
            "paths=[\"/a.stl\"]",
            "-F",
            "limit=5",
            "-f",
            "note=5",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    endpoint.assert();
}

#[test]
fn a_404_is_an_error_with_the_not_found_exit_code() {
    let mut cli = MockCli::new();
    let _missing = cli
        .server
        .mock("GET", "/nothing-here")
        .with_status(404)
        .with_body(r#"{"message":"No such route"}"#)
        .create();
    let output = cli.cmd().args(["api", "/nothing-here"]).output().unwrap();
    assert_eq!(output.status.code(), Some(67), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such route"));
}

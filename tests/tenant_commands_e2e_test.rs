//! Tenant commands end to end against a mock API.

mod common;

use common::{MockCli, TENANT};

#[test]
fn tenant_use_accepts_a_uuid_like_tenant_does() {
    let cli = MockCli::new();
    let output = cli
        .cmd()
        .args(["tenant", "use", "--name", TENANT])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = std::fs::read_to_string(cli.dir.path().join("config.yml")).unwrap();
    assert!(config.contains(TENANT), "{config}");
}

#[test]
fn tenant_use_with_an_unknown_name_fails() {
    let cli = MockCli::new();
    let output = cli
        .cmd()
        .args(["tenant", "use", "--name", "nope"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(67), "{output:?}");
}

#[test]
fn env_flag_targets_another_environment_for_one_run_without_saving_it() {
    // The saved active environment is `mock`; a second environment points at a
    // different server. --env (or PCLI2_ENV) uses it for this run only.
    let cli = MockCli::new();
    let mut other = mockito::Server::new();
    let other_user = other
        .mock("GET", "/users/me")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({"user": {"displayName": "Other", "settings": [{
                "tenantId": "33333333-3333-3333-3333-333333333333",
                "tenantRole": "author", "userEnabled": true,
                "tenantDisplayName": "Other Tenant", "tenantShortName": "other"
            }]}})
            .to_string(),
        )
        .expect(2)
        .create();
    let config_path = cli.dir.path().join("config.yml");
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(&format!(
        "  second:\n    api_base_url: {url}\n    ui_base_url: https://app.example.test\n    auth_base_url: {url}/token\n",
        url = other.url()
    ));
    std::fs::write(&config_path, &config).unwrap();
    // The second environment needs its own login.
    let credentials_path = cli.dir.path().join("dev_credentials.json");
    let mut credentials: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&credentials_path).unwrap()).unwrap();
    credentials["environments"]["second"] = credentials["environments"]["mock"].clone();
    std::fs::write(&credentials_path, credentials.to_string()).unwrap();

    let by_flag = cli
        .cmd()
        .args(["--env", "second", "tenant", "list", "--format", "csv"])
        .output()
        .unwrap();
    assert!(
        by_flag.status.success(),
        "{}",
        String::from_utf8_lossy(&by_flag.stderr)
    );
    assert!(String::from_utf8_lossy(&by_flag.stdout).contains("other"));

    let by_variable = cli
        .cmd()
        .env("PCLI2_ENV", "second")
        .env("PCLI2_CACHE_DIR", cli.dir.path().join("cache2"))
        .args(["tenant", "list", "--format", "csv"])
        .output()
        .unwrap();
    assert!(by_variable.status.success());
    assert!(String::from_utf8_lossy(&by_variable.stdout).contains("other"));
    other_user.assert();

    // Nothing was saved: the default is still `mock`.
    let saved = std::fs::read_to_string(&config_path).unwrap();
    assert!(saved.contains("active_environment: mock"), "{saved}");
}

#[test]
fn an_unknown_env_is_refused_before_anything_runs() {
    let cli = MockCli::new();
    let output = cli
        .cmd()
        .args(["--env", "nope", "tenant", "list"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(67), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("nope"));
}

#[test]
fn user_list_accepts_a_tenant_for_one_run() {
    let mut cli = MockCli::new();
    let users = cli
        .server
        .mock("GET", format!("/tenants/{}/users", common::TENANT).as_str())
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"users":[]}"#)
        .create();
    let output = cli
        .cmd()
        .args(["user", "list", "--tenant", "mock"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    users.assert();
}

const ACTIVITY: &str = r#"{"searches":3,"compares":1,"downloads":0,"uploads":2,"reports":0,
    "activeUsers":2,"internalActiveUsers":1,
    "searchesByType":{"text":1,"visual":0,"geometric":2,"part":0,"composite":0,"metadata":0},
    "reportsByType":{"DUPLICATION":0,"SIMPLIFICATION":0,"CUSTOM":0},
    "featureUsage":{"compare":1,"upload":2},
    "daily":[{"date":"2026-09-01","searches":3,"compares":1,"downloads":0,"uploads":0,"reports":0,"activeUsers":2},
             {"date":"2026-09-02","searches":0,"compares":0,"downloads":0,"uploads":2,"reports":0,"activeUsers":1}]}"#;

fn activity_mock(cli: &mut MockCli, from: &str, to: &str) -> mockito::Mock {
    cli.server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/activity-metrics").as_str(),
        )
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("from".into(), from.into()),
            mockito::Matcher::UrlEncoded("to".into(), to.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(ACTIVITY)
        .create()
}

#[test]
fn tenant_usage_prints_one_row_per_count_including_asset_types() {
    let mut cli = MockCli::new();
    let activity = activity_mock(&mut cli, "2026-09-01", "2026-09-02");
    let types = cli
        .server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/type-counts").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"model":10,"image":2}"#)
        .create();
    let output = cli
        .cmd()
        .args([
            "tenant",
            "usage",
            "--from",
            "2026-09-01",
            "--to",
            "2026-09-02",
            "--format",
            "csv",
            "--headers",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.starts_with("CATEGORY,NAME,COUNT\nactivity,searches,3\n"),
        "{stdout}"
    );
    assert!(stdout.contains("\nsearches,geometric,2\n"), "{stdout}");
    assert!(stdout.contains("\nfeatures,upload,2\n"), "{stdout}");
    assert!(
        stdout.ends_with("assets,model,10\nassets,image,2\n"),
        "{stdout}"
    );
    activity.assert();
    types.assert();
}

#[test]
fn tenant_usage_daily_needs_no_asset_counts() {
    let mut cli = MockCli::new();
    let activity = activity_mock(&mut cli, "2026-08-27", "2026-09-02");
    let types = cli
        .server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/type-counts").as_str(),
        )
        .expect(0)
        .create();
    let output = cli
        .cmd()
        .args([
            "tenant",
            "usage",
            "--to",
            "2026-09-02",
            "--days",
            "7",
            "--daily",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let days: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(days[1]["date"], "2026-09-02");
    assert_eq!(days[1]["uploads"], 2);
    activity.assert();
    types.assert();
}

#[test]
fn tenant_usage_rejects_a_bad_period_before_calling_the_api() {
    let mut cli = MockCli::new();
    let activity = cli
        .server
        .mock("GET", mockito::Matcher::Regex("activity-metrics".into()))
        .expect(0)
        .create();
    for args in [
        vec!["--from", "2026-09-10", "--to", "2026-09-01"],
        vec!["--from", "2025-01-01", "--to", "2026-01-02"],
        vec!["--from", "yesterday"],
    ] {
        let output = cli
            .cmd()
            .args(["tenant", "usage"])
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(64), "{args:?}: {output:?}");
    }
    activity.assert();
}

#[test]
fn env_use_brings_back_the_tenant_last_used_in_that_environment() {
    let cli = MockCli::new();
    let other_tenant = "33333333-3333-3333-3333-333333333333";
    let config_path = cli.dir.path().join("config.yml");
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(&format!(
        "  second:\n    api_base_url: https://second.example.test\n    active_tenant_uuid: {other_tenant}\n  fresh:\n    api_base_url: https://fresh.example.test\n"
    ));
    std::fs::write(&config_path, &config).unwrap();

    let env_use = |name: &str| {
        let output = cli
            .cmd()
            .args(["env", "use", "--name", name])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let saved: serde_json::Value =
            serde_norway::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        (String::from_utf8(output.stdout).unwrap(), saved)
    };

    let (stdout, saved) = env_use("second");
    assert!(
        stdout.contains(&format!("with tenant '{other_tenant}'")),
        "{stdout}"
    );
    assert_eq!(saved["active_tenant_uuid"], other_tenant);

    // Back to the first environment: its tenant is still there.
    let (_, saved) = env_use("mock");
    assert_eq!(saved["environments"]["mock"]["active_tenant_uuid"], TENANT);
    assert_eq!(saved["active_tenant_uuid"], TENANT);
    assert_eq!(
        saved["environments"]["second"]["active_tenant_uuid"],
        other_tenant
    );

    // An environment never given a tenant has none, not the previous one's.
    let (stdout, saved) = env_use("fresh");
    assert!(
        stdout.contains("Select a tenant with 'pcli2 tenant use'"),
        "{stdout}"
    );
    assert!(saved.get("active_tenant_uuid").is_none(), "{saved}");
}

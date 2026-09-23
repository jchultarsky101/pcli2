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

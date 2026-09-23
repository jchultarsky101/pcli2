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

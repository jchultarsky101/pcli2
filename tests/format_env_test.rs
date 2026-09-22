//! `PCLI2_FORMAT` sets the default `--format` for every command, including the
//! ones that used to parse `--format` by hand and never looked at it.

mod common;

use common::MockCli;

#[test]
fn tenant_list_honours_pcli2_format() {
    let cli = MockCli::new();
    let json = cli.cmd().args(["tenant", "list"]).output().unwrap();
    assert!(
        json.status.success(),
        "{}",
        String::from_utf8_lossy(&json.stderr)
    );
    assert!(String::from_utf8_lossy(&json.stdout)
        .trim_start()
        .starts_with('['));

    let csv = cli
        .cmd()
        .env("PCLI2_FORMAT", "csv")
        .args(["tenant", "list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&csv.stdout);
    assert!(
        csv.status.success(),
        "{}",
        String::from_utf8_lossy(&csv.stderr)
    );
    assert!(!stdout.trim_start().starts_with('['), "{stdout}");
    assert!(stdout.contains("mock"), "{stdout}");

    // An explicit --format still wins over the environment.
    let explicit = cli
        .cmd()
        .env("PCLI2_FORMAT", "csv")
        .args(["tenant", "list", "--format", "json"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&explicit.stdout)
        .trim_start()
        .starts_with('['));
}

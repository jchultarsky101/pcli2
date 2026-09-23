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

#[test]
fn table_output_lines_up_columns_under_a_rule() {
    let cli = MockCli::new();
    let output = cli
        .cmd()
        .args(["tenant", "list", "--format", "table"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 3, "{stdout}");
    assert!(lines[1].chars().all(|c| c == '─'), "{stdout}");
    assert!(lines[2].contains("mock"), "{stdout}");
    // The header and the row start their second column at the same place.
    let second_column = |line: &str| {
        line.find("  ")
            .map(|i| i + line[i..].len() - line[i..].trim_start().len())
    };
    assert_eq!(second_column(lines[0]), second_column(lines[2]), "{stdout}");
}

#[test]
fn columns_selects_csv_columns_by_header_name() {
    let cli = MockCli::new();
    let with_headers = cli
        .cmd()
        .args([
            "tenant",
            "list",
            "--format",
            "csv",
            "--headers",
            "--columns",
            "TENANT_NAME,TENANT_UUID",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&with_headers.stdout);
    assert!(
        with_headers.status.success(),
        "{}",
        String::from_utf8_lossy(&with_headers.stderr)
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert_eq!(lines[0].split(',').count(), 2, "{stdout}");
    assert!(lines[1].starts_with("mock,"), "{stdout}");

    // Without --headers the header row is still used to pick, but not printed.
    let without = cli
        .cmd()
        .args([
            "tenant",
            "list",
            "--format",
            "csv",
            "--columns",
            "tenant_name",
        ])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&without.stdout).trim(), "mock");
}

#[test]
fn piped_output_without_a_format_is_still_json() {
    // The terminal default is a table; a pipe, like this one, keeps JSON.
    let cli = MockCli::new();
    let output = cli.cmd().args(["tenant", "list"]).output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout)
        .trim_start()
        .starts_with('['));
}

#[test]
fn an_unknown_column_is_an_error_not_every_column() {
    let cli = MockCli::new();
    let output = cli
        .cmd()
        .args(["tenant", "list", "--format", "csv", "--columns", "nope"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TENANT_NAME"));
}

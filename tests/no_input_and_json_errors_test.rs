//! `--no-input` and `--error-format json`, driven through the built binary.
//!
//! None of these touch the network: every case fails, on purpose, before a
//! request would go out.

use assert_cmd::prelude::*;
use std::process::{Command, Stdio};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pcli2-noinput-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pcli2(name: &str) -> Command {
    let dir = scratch(name);
    let mut cmd = Command::cargo_bin("pcli2").unwrap();
    cmd.env("PCLI2_CONFIG_DIR", &dir)
        .env("PCLI2_CACHE_DIR", dir.join("cache"))
        .env("PCLI2_NO_UPDATE_CHECK", "1")
        .env_remove("PCLI2_ERROR_FORMAT")
        .env_remove("PCLI2_NO_INPUT")
        .stdin(Stdio::null());
    cmd
}

fn stderr_lines(output: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn last_json_line(output: &std::process::Output) -> serde_json::Value {
    let lines = stderr_lines(output);
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("stderr line is not JSON: {e}\n{line}\nall:\n{lines:?}"));
    }
    serde_json::from_str(lines.last().expect("stderr should not be empty")).unwrap()
}

#[test]
fn a_usage_error_is_one_json_object_with_exit_code_64() {
    let output = pcli2("usage")
        .args(["--error-format", "json", "no-such-command"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let lines = stderr_lines(&output);
    assert_eq!(lines.len(), 1, "one object expected, got:\n{lines:?}");
    let object = last_json_line(&output);
    assert_eq!(object["level"], "ERROR");
    assert_eq!(object["code"], 64);
    assert_eq!(object["kind"], "usage");
    assert!(
        object["message"]
            .as_str()
            .unwrap()
            .contains("no-such-command"),
        "{object}"
    );
}

#[test]
fn the_environment_variable_selects_json_errors_too() {
    let output = pcli2("env")
        .env("PCLI2_ERROR_FORMAT", "json")
        .arg("--bogus-flag")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let object = last_json_line(&output);
    assert_eq!(object["kind"], "usage");
    assert!(object["message"].as_str().unwrap().contains("--bogus-flag"));
}

#[test]
fn help_and_version_are_unaffected_by_json_errors() {
    pcli2("help")
        .args(["--error-format", "json", "--version"])
        .assert()
        .success()
        .stdout(predicates::str::contains("pcli2 "));
}

#[test]
fn a_confirmation_without_a_terminal_is_a_usage_error_not_a_silent_no() {
    // stdin is /dev/null here: the old code read end-of-file as "no" and
    // exited 0 as "cancelled", which a script took for success.
    let output = pcli2("tty").args(["cache", "clear"]).output().unwrap();
    assert_eq!(output.status.code(), Some(64), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--yes"), "{stderr}");
    assert!(stderr.contains("stdin is not a terminal"), "{stderr}");
}

#[test]
fn no_input_names_itself_as_the_reason() {
    let output = pcli2("noinput")
        .args(["--no-input", "cache", "clear"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--no-input was given"), "{stderr}");
    assert!(stderr.contains("--yes"), "{stderr}");
}

#[test]
fn no_input_via_environment_variable() {
    let output = pcli2("noinput-env")
        .env("PCLI2_NO_INPUT", "true")
        .args(["cache", "clear"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--no-input was given"));
}

#[test]
fn yes_satisfies_no_input() {
    pcli2("yes")
        .args(["--no-input", "cache", "clear", "--yes"])
        .assert()
        .success();
}

#[test]
fn json_errors_put_the_exit_code_in_the_last_line() {
    let output = pcli2("json-noinput")
        .args(["--error-format", "json", "--no-input", "cache", "clear"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let object = last_json_line(&output);
    assert_eq!(object["code"], 64);
    assert_eq!(object["kind"], "usage");
    assert!(object["message"].as_str().unwrap().contains("--yes"));
}

#[test]
fn json_stats_line_is_an_object() {
    let output = pcli2("stats")
        .args([
            "--error-format",
            "json",
            "--stats",
            "--no-input",
            "cache",
            "clear",
        ])
        .output()
        .unwrap();
    let lines = stderr_lines(&output);
    let stats = lines
        .iter()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["level"] == "INFO")
        .expect("a stats object");
    assert_eq!(stats["requests"], 0);
    assert!(stats["message"].as_str().unwrap().contains("API request"));
}

#[test]
fn selecting_a_tenant_without_a_name_needs_a_prompt() {
    // No configuration exists in the scratch directory, so this fails before
    // the menu whichever way; what matters is that --no-input does not hang
    // and exits non-zero with an explanation rather than an empty menu.
    let output = pcli2("tenant")
        .args(["--no-input", "--error-format", "json", "tenant", "use"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let object = last_json_line(&output);
    assert_eq!(object["level"], "ERROR");
    assert!(object["code"].as_i64().unwrap() > 0);
}

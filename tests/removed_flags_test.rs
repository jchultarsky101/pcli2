//! The flag spellings removed in 2.0 are refused with a pointer to the
//! replacement, in text and in JSON error mode, before anything else runs.

use assert_cmd::prelude::*;
use std::process::{Command, Stdio};

fn pcli2() -> Command {
    let dir = std::env::temp_dir().join(format!("pcli2-removed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cmd = Command::cargo_bin("pcli2").unwrap();
    cmd.env("PCLI2_CONFIG_DIR", &dir)
        .env("PCLI2_CACHE_DIR", dir.join("cache"))
        .env("PCLI2_NO_UPDATE_CHECK", "1")
        .env_remove("PCLI2_ERROR_FORMAT")
        .stdin(Stdio::null());
    cmd
}

fn refused(args: &[&str], old: &str, replacement: &str) {
    let output = pcli2().args(args).output().unwrap();
    assert_eq!(output.status.code(), Some(64), "{args:?}: {output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(old),
        "{args:?}: should name {old}: {stderr}"
    );
    assert!(
        stderr.contains(replacement),
        "{args:?}: should name {replacement}: {stderr}"
    );
    assert!(
        stderr.contains("removed in pcli2 2.0.0"),
        "{args:?}: should say it was removed: {stderr}"
    );
}

#[test]
fn every_removed_spelling_names_its_replacement() {
    refused(
        &["asset", "create", "--file", "x.stl", "--folder-path", "/a"],
        "--file",
        "--input",
    );
    refused(
        &[
            "asset",
            "create-batch",
            "--files",
            "*.stl",
            "--folder-path",
            "/a",
        ],
        "--files",
        "--input",
    );
    refused(
        &["asset", "metadata", "create-batch", "--csv-file", "m.csv"],
        "--csv-file",
        "--input",
    );
    refused(
        &[
            "folder",
            "upload",
            "--local-path",
            ".",
            "--folder-path",
            "/a",
        ],
        "--local-path",
        "--input",
    );
    refused(
        &["config", "import", "--file", "c.yml"],
        "--file",
        "--input",
    );
    refused(
        &[
            "asset",
            "thumbnail",
            "--path",
            "/a/b.stl",
            "--file",
            "t.png",
        ],
        "--file",
        "--output",
    );
    refused(
        &["asset", "download", "--path", "/a/b.stl", "out.stl"],
        "positional output path",
        "--output",
    );
}

#[test]
fn the_refusal_is_a_json_object_in_json_error_mode() {
    let output = pcli2()
        .args([
            "--error-format",
            "json",
            "asset",
            "create-batch",
            "--files",
            "*.stl",
            "--folder-path",
            "/a",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr.lines().find(|l| !l.trim().is_empty()).unwrap();
    let object: serde_json::Value =
        serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(object["code"], 64);
    assert_eq!(object["kind"], "usage");
    assert!(object["message"].as_str().unwrap().contains("--input"));
}

#[test]
fn the_new_spellings_still_parse() {
    // Dry run needs no server; it fails on the missing configuration only
    // after the arguments were accepted, so any exit other than 64 will do.
    for args in [
        vec![
            "asset",
            "create-batch",
            "--input",
            "*.stl",
            "--folder-path",
            "/a",
            "--dry-run",
        ],
        vec![
            "folder",
            "upload",
            "--input",
            ".",
            "--folder-path",
            "/a",
            "--dry-run",
        ],
    ] {
        let output = pcli2().args(&args).output().unwrap();
        assert_ne!(output.status.code(), Some(64), "{args:?}: {output:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("removed in pcli2"),
            "{args:?}"
        );
    }
}

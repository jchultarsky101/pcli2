//! `folder upload` and `folder thumbnail` end to end: the bulk-run rules they now
//! share with `folder download`.

mod common;

use common::{asset_json, MockCli};
use mockito::Matcher;
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn upload_dir(cli: &MockCli, names: &[&str]) -> std::path::PathBuf {
    let dir = cli.dir.path().join("to-upload");
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::write(dir.join(name), format!("solid {name}")).unwrap();
    }
    dir
}

fn existing_paths(cli: &mut MockCli, existing: &[&str]) -> mockito::Mock {
    cli.server
        .mock(
            "POST",
            format!("/tenants/{}/assets/existing-paths", cli.tenant).as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({ "existingPaths": existing }).to_string())
        .create()
}

#[test]
fn an_existing_file_stops_the_upload_before_anything_is_sent() {
    let mut cli = MockCli::new();
    let dir = upload_dir(&cli, &["a.stl", "b.stl", "c.stl"]);
    let _existing = existing_paths(&mut cli, &["/b.stl"]);
    let uploads = cli
        .server
        .mock("POST", format!("/tenants/{}/assets", cli.tenant).as_str())
        .expect(0)
        .create();

    let output = cli
        .cmd()
        .args(["folder", "upload", "--folder-path", "/", "--input"])
        .arg(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr: {stderr}");
    assert!(stderr.contains("b.stl"), "stderr: {stderr}");
    uploads.assert();
}

#[test]
fn a_failed_upload_stops_the_rest_and_says_what_was_not_attempted() {
    let mut cli = MockCli::new();
    let dir = upload_dir(&cli, &["a.stl", "b.stl", "c.stl"]);
    let _existing = existing_paths(&mut cli, &[]);
    let path = format!("/tenants/{}/assets", cli.tenant);
    let _a = cli
        .server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex("filename=\"a.stl\"".into()))
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({ "asset": asset_json(id(1), "a.stl", "indexing", false) })
                .to_string(),
        )
        .create();
    let _b = cli
        .server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex("filename=\"b.stl\"".into()))
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"bad file"}"#)
        .create();
    let c = cli
        .server
        .mock("POST", path.as_str())
        .match_body(Matcher::Regex("filename=\"c.stl\"".into()))
        .expect(0)
        .create();

    // One at a time, with a pause before each upload, so c is still waiting when
    // b fails.
    let output = cli
        .cmd()
        .args([
            "folder",
            "upload",
            "--folder-path",
            "/",
            "--concurrent",
            "1",
            "--delay",
            "1",
            "--input",
        ])
        .arg(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("Successfully uploaded: 1"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("Not attempted"), "stderr: {stderr}");
    assert!(stderr.contains("b.stl"), "stderr: {stderr}");
    c.assert();
}

#[test]
fn folder_thumbnail_counts_assets_without_a_thumbnail_separately() {
    let mut cli = MockCli::new();
    let (with, without) = (id(1), id(2));
    let _folders = cli.folders(&[]);
    let _root = cli.assets_in(
        None,
        &[
            asset_json(with, "/with.stl", "finished", false),
            asset_json(without, "/without.stl", "finished", false),
        ],
    );
    let _sub = cli.subfolders_of(None, &[]);
    let _png = cli
        .server
        .mock(
            "GET",
            format!("/tenants/{}/assets/{}/thumbnail.png", cli.tenant, with).as_str(),
        )
        .with_status(200)
        .with_body(b"\x89PNG fake")
        .create();
    let _none = cli
        .server
        .mock(
            "GET",
            format!("/tenants/{}/assets/{}/thumbnail.png", cli.tenant, without).as_str(),
        )
        .with_status(404)
        .with_body(r#"{"message":"not found"}"#)
        .create();

    let out = cli.dir.path().join("thumbs");
    let output = cli
        .cmd()
        .args(["folder", "thumbnail", "--folder-path", "/", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert_eq!(
        std::fs::read(out.join("with.png")).unwrap(),
        b"\x89PNG fake"
    );
    assert!(!out.join("without.png").exists());
    assert!(
        stderr.contains("No thumbnail available: 1"),
        "stderr: {stderr}"
    );
}

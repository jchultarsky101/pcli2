//! Folder descriptions end to end against a mock API: what `folder description
//! get|set|clear` and `folder create --description` send, what they print, and
//! that `folder get` shows a description without changing its CSV columns.

mod common;

use common::{folder_json, MockCli, TENANT};
use mockito::{Matcher, Mock};
use uuid::Uuid;

fn rail() -> Uuid {
    Uuid::from_bytes([7; 16])
}

fn rail_json(description: Option<&str>) -> serde_json::Value {
    let mut folder = folder_json(rail(), "Rail", None);
    if let Some(description) = description {
        folder["description"] = serde_json::json!(description);
    }
    folder
}

/// /Rail, as the listing and `GET folders/{id}` return it.
fn rail_folder(cli: &mut MockCli, description: Option<&str>) -> Vec<Mock> {
    vec![
        cli.folders(&[(rail(), "Rail", None)]),
        cli.server
            .mock(
                "GET",
                format!("/tenants/{TENANT}/folders/{}", rail()).as_str(),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::json!({"folder": rail_json(description)}).to_string())
            .create(),
    ]
}

fn description_path() -> String {
    format!("/tenants/{TENANT}/folders/{}/description", rail())
}

fn run(cli: &MockCli, args: &[&str]) -> std::process::Output {
    cli.cmd().args(args).output().unwrap()
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn get_prints_the_description_alone_or_nothing() {
    let mut cli = MockCli::new();
    let _rail = rail_folder(&mut cli, Some("Rail car parts — 2026"));
    let output = run(
        &cli,
        &["folder", "description", "get", "--folder-path", "/Rail"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Rail car parts — 2026\n");

    let mut cli = MockCli::new();
    let _rail = rail_folder(&mut cli, None);
    let output = run(
        &cli,
        &["folder", "description", "get", "--folder-path", "/Rail"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
}

#[test]
fn set_sends_the_trimmed_text_and_prints_nothing() {
    let mut cli = MockCli::new();
    let _rail = rail_folder(&mut cli, None);
    let patch = cli
        .server
        .mock("PATCH", description_path().as_str())
        .match_body(Matcher::Json(
            serde_json::json!({"description": "Rail car parts"}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({"folder": rail_json(Some("Rail car parts"))}).to_string())
        .expect(1)
        .create();
    let output = run(
        &cli,
        &[
            "folder",
            "description",
            "set",
            "--folder-path",
            "/Rail",
            "--text",
            "  Rail car parts  ",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    patch.assert();
}

#[test]
fn clear_sends_a_delete_and_prints_nothing() {
    let mut cli = MockCli::new();
    let _rail = rail_folder(&mut cli, Some("old"));
    let delete = cli
        .server
        .mock("DELETE", description_path().as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({"folder": rail_json(None)}).to_string())
        .expect(1)
        .create();
    let output = run(
        &cli,
        &[
            "folder",
            "description",
            "clear",
            "--folder-uuid",
            &rail().to_string(),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    delete.assert();
}

#[test]
fn an_empty_or_too_long_description_is_refused_before_any_request() {
    let mut cli = MockCli::new();
    let _rail = rail_folder(&mut cli, None);
    let never = [
        cli.server.mock("PATCH", Matcher::Any).expect(0).create(),
        cli.server.mock("POST", Matcher::Any).expect(0).create(),
    ];
    let too_long = "x".repeat(256);
    for args in [
        vec![
            "folder",
            "description",
            "set",
            "--folder-path",
            "/Rail",
            "--text",
            "   ",
        ],
        vec![
            "folder",
            "description",
            "set",
            "--folder-path",
            "/Rail",
            "--text",
            &too_long,
        ],
        vec![
            "folder",
            "create",
            "--name",
            "New",
            "--parent-folder-path",
            "/",
            "--description",
            "",
        ],
    ] {
        let output = run(&cli, &args);
        assert_eq!(output.status.code(), Some(64), "{args:?}: {output:?}");
    }
    for mock in never {
        mock.assert();
    }
}

#[test]
fn create_sends_a_description_only_when_one_is_given() {
    for (description, body) in [
        (
            Some("Rail car parts"),
            serde_json::json!({"name": "New", "description": "Rail car parts"}),
        ),
        (None, serde_json::json!({"name": "New"})),
    ] {
        let mut cli = MockCli::new();
        let created = cli
            .server
            .mock("POST", format!("/tenants/{TENANT}/folders").as_str())
            .match_body(Matcher::Json(body))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::json!({"folder": folder_json(rail(), "New", None)}).to_string())
            .expect(1)
            .create();
        let mut args = vec![
            "folder",
            "create",
            "--name",
            "New",
            "--parent-folder-path",
            "/",
        ];
        if let Some(description) = description {
            args.extend(["--description", description]);
        }
        let output = run(&cli, &args);
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(stdout(&output), format!("{}\n", rail()));
        created.assert();
    }
}

#[test]
fn folder_get_shows_the_description_in_json_but_keeps_its_csv_columns() {
    let mut cli = MockCli::new();
    let _rail = rail_folder(&mut cli, Some("Rail car parts"));
    let json = run(
        &cli,
        &[
            "folder",
            "get",
            "--folder-path",
            "/Rail",
            "--format",
            "json",
        ],
    );
    assert!(json.status.success(), "{}", stderr(&json));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["description"], "Rail car parts");

    let csv = run(
        &cli,
        &[
            "folder",
            "get",
            "--folder-path",
            "/Rail",
            "--format",
            "csv",
            "--headers",
        ],
    );
    assert!(csv.status.success(), "{}", stderr(&csv));
    assert_eq!(
        stdout(&csv),
        format!(
            "NAME,PATH,ASSETS_COUNT,FOLDERS_COUNT,UUID\nRail,/Rail,0,0,{}\n",
            rail()
        )
    );
}

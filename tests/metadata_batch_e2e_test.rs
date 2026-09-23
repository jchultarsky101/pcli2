//! `asset metadata create-batch` end to end against a mock API: rows are
//! resolved first (UUIDs in one batch request), new fields are registered once,
//! and nothing is written when a row cannot be resolved.

mod common;

use common::{asset_json, MockCli};
use mockito::Matcher;
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn empty_field_registry(cli: &mut MockCli) -> mockito::Mock {
    cli.server
        .mock("GET", format!("/tenants/{}/metadata-fields", cli.tenant).as_str())
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"metadataFields":[],"pageData":{"total":0,"perPage":1000,"currentPage":1,"lastPage":1,"startIndex":0,"endIndex":0}}"#)
        .create()
}

#[test]
fn uuid_rows_are_resolved_in_one_request_and_a_new_field_is_registered_once() {
    let mut cli = MockCli::new();
    let ids = [id(1), id(2), id(3)];
    let _registry = empty_field_registry(&mut cli);
    let batch = cli
        .server
        .mock(
            "POST",
            format!("/tenants/{}/assets/batch", cli.tenant).as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({"assets": ids.iter().enumerate()
                .map(|(i, id)| asset_json(*id, &format!("/Parts/{i}.stl"), "finished", false))
                .collect::<Vec<_>>()})
            .to_string(),
        )
        .expect(1)
        .create();
    let single_lookups = cli
        .server
        .mock(
            "GET",
            Matcher::Regex(format!("^/tenants/{}/assets/[0-9a-f-]+$", cli.tenant)),
        )
        .expect(0)
        .create();
    let registration = cli
        .server
        .mock(
            "POST",
            format!("/tenants/{}/metadata-fields", cli.tenant).as_str(),
        )
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body("{}")
        .expect(1)
        .create();
    let writes: Vec<_> = ids
        .iter()
        .map(|id| {
            cli.server
                .mock(
                    "PATCH",
                    format!("/tenants/{}/assets/{}", cli.tenant, id).as_str(),
                )
                .match_body(Matcher::PartialJson(
                    serde_json::json!({"metadata": {"Material": "Steel"}}),
                ))
                .with_status(200)
                .expect(1)
                .create()
        })
        .collect();

    let csv = cli.dir.path().join("metadata.csv");
    let mut content = String::from("path,id,metadata:Material\n");
    for (i, id) in ids.iter().enumerate() {
        content.push_str(&format!("/Parts/{i}.stl,{id},Steel\n"));
    }
    std::fs::write(&csv, content).unwrap();

    let output = cli
        .cmd()
        .args([
            "asset",
            "metadata",
            "create-batch",
            "--concurrent",
            "3",
            "--input",
        ])
        .arg(&csv)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    batch.assert();
    single_lookups.assert();
    registration.assert();
    for write in writes {
        write.assert();
    }
}

#[test]
fn an_unresolvable_row_stops_the_batch_before_anything_is_written() {
    let mut cli = MockCli::new();
    let known = id(1);
    let _registry = empty_field_registry(&mut cli);
    let _folders = cli.folders(&[]);
    let _batch = cli
        .server
        .mock(
            "POST",
            format!("/tenants/{}/assets/batch", cli.tenant).as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({"assets": [asset_json(known, "/Parts/a.stl", "finished", false)]})
                .to_string(),
        )
        .create();
    let writes = cli
        .server
        .mock(
            "PATCH",
            Matcher::Regex(format!("^/tenants/{}/assets/", cli.tenant)),
        )
        .expect(0)
        .create();

    let csv = cli.dir.path().join("metadata.csv");
    std::fs::write(
        &csv,
        format!("path,id,metadata:Material\n/Parts/a.stl,{known},Steel\n/Missing/b.stl,,Brass\n"),
    )
    .unwrap();

    let output = cli
        .cmd()
        .args(["asset", "metadata", "create-batch", "--input"])
        .arg(&csv)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Missing/b.stl"), "{stderr}");
    writes.assert();
}

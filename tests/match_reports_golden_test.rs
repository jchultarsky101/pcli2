//! The folder match reports, byte for byte.
//!
//! `folder geometric-match`, `part-match` and `visual-match` produce reports that
//! people diff and scripts parse. These tests run each against a mock API in
//! every output shape and compare stdout with the files in
//! `fixtures/golden/`. Set `UPDATE_GOLDEN=1` to rewrite the files after an
//! intended change, and review the diff.

mod common;

use common::{asset_json, MockCli};
use mockito::Matcher;
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

/// /Parts holds a, b and c, each with metadata.
fn parts_folder(cli: &mut MockCli) -> Vec<mockito::Mock> {
    let parts = id(9);
    let with_metadata = |uuid: Uuid, path: &str, material: &str, weight: &str| {
        let mut asset = asset_json(uuid, path, "finished", false);
        asset["metadata"] = serde_json::json!({"Material": material, "Weight": weight});
        asset
    };
    vec![
        cli.folders(&[(parts, "Parts", None)]),
        cli.assets_in(
            Some(parts),
            &[
                with_metadata(id(1), "/Parts/a.stl", "Steel", "12.5"),
                with_metadata(id(2), "/Parts/b.stl", "Steel", "12.7"),
                with_metadata(id(3), "/Parts/c.stl", "Brass", "3"),
            ],
        ),
    ]
}

fn candidate(uuid: Uuid, path: &str) -> serde_json::Value {
    let mut asset = asset_json(uuid, path, "finished", false);
    let material = if path.ends_with("c.stl") {
        "Brass"
    } else {
        "Steel"
    };
    asset["metadata"] = serde_json::json!({"Material": material});
    asset
}

fn page(matches: Vec<serde_json::Value>) -> String {
    serde_json::json!({
        "matches": matches,
        "pageData": {"total": matches.len(), "perPage": 500, "currentPage": 1, "lastPage": 1, "startIndex": 0, "endIndex": matches.len()}
    })
    .to_string()
}

/// Who matches whom: a~b strongly, b~c weakly.
fn neighbours(n: u8) -> Vec<(Uuid, &'static str, f64, f64)> {
    match n {
        1 => vec![(id(2), "/Parts/b.stl", 99.5, 98.25)],
        2 => vec![
            (id(1), "/Parts/a.stl", 99.5, 98.75),
            (id(3), "/Parts/c.stl", 91.0, 88.5),
        ],
        _ => vec![(id(2), "/Parts/b.stl", 91.0, 89.0)],
    }
}

fn search_mocks(cli: &mut MockCli, kind: &str) -> Vec<mockito::Mock> {
    (1..=3u8)
        .map(|n| {
            let matches: Vec<_> = neighbours(n)
                .into_iter()
                .map(|(uuid, path, forward, reverse)| match kind {
                    "geometric" => serde_json::json!({"asset": candidate(uuid, path), "matchPercentage": forward}),
                    "part" => serde_json::json!({
                        "asset": candidate(uuid, path),
                        "forwardMatchPercentage": forward,
                        "reverseMatchPercentage": reverse
                    }),
                    _ => serde_json::json!({"asset": candidate(uuid, path)}),
                })
                .collect();
            let mock = match kind {
                "visual" => cli
                    .server
                    .mock("POST", "/tenants/assets/visual-search")
                    .match_body(Matcher::PartialJson(serde_json::json!({"assetId": id(n)}))),
                other => cli.server.mock(
                    "POST",
                    format!("/tenants/{}/assets/{}/{}-search", cli.tenant, id(n), other).as_str(),
                ),
            };
            mock.with_status(200)
                .with_header("content-type", "application/json")
                .with_body(page(matches))
                .create()
        })
        .collect()
}

fn check(name: &str, command: &str, kind: &str, extra: &[&str]) {
    let mut cli = MockCli::new();
    let _folder = parts_folder(&mut cli);
    let _searches = search_mocks(&mut cli, kind);
    let mut args = vec![
        "folder",
        command,
        "--folder-path",
        "/Parts",
        "--threshold",
        "80",
        "--concurrent",
        "1",
    ];
    args.extend_from_slice(extra);
    let output = cli.cmd().args(&args).output().unwrap();
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &output.stdout).unwrap();
        return;
    }
    let expected = std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} is missing; run with UPDATE_GOLDEN=1", path.display()));
    assert!(
        output.stdout == expected,
        "{name} differs from {}:\n--- got ---\n{}\n--- expected ---\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&expected)
    );
}

const SHAPES: &[(&str, &[&str])] = &[
    ("json", &["--format", "json"]),
    ("json_metadata", &["--format", "json", "--metadata"]),
    ("csv", &["--format", "csv"]),
    ("csv_headers", &["--format", "csv", "--headers"]),
    (
        "csv_headers_metadata",
        &["--format", "csv", "--headers", "--metadata"],
    ),
];

#[test]
fn geometric_match_reports_are_unchanged() {
    for (shape, extra) in SHAPES {
        check(
            &format!("geometric_{shape}"),
            "geometric-match",
            "geometric",
            extra,
        );
    }
}

#[test]
fn part_match_reports_are_unchanged() {
    for (shape, extra) in SHAPES {
        check(&format!("part_{shape}"), "part-match", "part", extra);
    }
}

#[test]
fn visual_match_reports_are_unchanged() {
    for (shape, extra) in SHAPES {
        check(&format!("visual_{shape}"), "visual-match", "visual", extra);
    }
}

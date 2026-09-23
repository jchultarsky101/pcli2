//! `folder geometric-match --groups` end to end against a mock API.

mod common;

use common::{asset_json, MockCli};
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn search_answer(matches: &[(Uuid, &str, f64)]) -> String {
    let matches: Vec<_> = matches
        .iter()
        .map(|(uuid, path, score)| {
            serde_json::json!({"asset": asset_json(*uuid, path, "finished", false), "matchPercentage": score})
        })
        .collect();
    serde_json::json!({
        "matches": matches,
        "pageData": {"total": matches.len(), "perPage": 500, "currentPage": 1, "lastPage": 1, "startIndex": 0, "endIndex": matches.len()}
    })
    .to_string()
}

/// A folder /Parts where a matches b and b matches c; d matches nothing.
fn chain_fixture() -> (MockCli, Vec<mockito::Mock>) {
    let mut cli = MockCli::new();
    let parts = id(9);
    let (a, b, c, d) = (id(1), id(2), id(3), id(4));
    let _folders = cli.folders(&[(parts, "Parts", None)]);
    let _assets = cli.assets_in(
        Some(parts),
        &[
            asset_json(a, "/Parts/a.stl", "finished", false),
            asset_json(b, "/Parts/b.stl", "finished", false),
            asset_json(c, "/Parts/c.stl", "finished", false),
            asset_json(d, "/Parts/d.stl", "finished", false),
        ],
    );
    let answers = [
        (a, search_answer(&[(b, "/Parts/b.stl", 99.5)])),
        (
            b,
            search_answer(&[(a, "/Parts/a.stl", 99.5), (c, "/Parts/c.stl", 97.0)]),
        ),
        (c, search_answer(&[(b, "/Parts/b.stl", 97.0)])),
        (d, search_answer(&[])),
    ];
    let _searches: Vec<_> = answers
        .iter()
        .map(|(uuid, body)| {
            cli.server
                .mock(
                    "POST",
                    format!("/tenants/{}/assets/{}/geometric-search", cli.tenant, uuid).as_str(),
                )
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(body)
                .create()
        })
        .collect();
    let mut mocks = vec![_folders, _assets];
    mocks.extend(_searches);
    (cli, mocks)
}

#[test]
fn groups_join_assets_connected_by_a_chain_of_matches() {
    // a matches b and b matches c, so a, b and c are one part even though a and c
    // never matched directly; d matches nothing.
    let (cli, _mocks) = chain_fixture();
    let output = cli
        .cmd()
        .args([
            "folder",
            "geometric-match",
            "--folder-path",
            "/Parts",
            "--format",
            "csv",
            "--headers",
            "--groups",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut lines = stdout.lines();
    let header = lines.next().unwrap();
    assert!(header.ends_with("GROUP_ID,GROUP_SIZE"), "{header}");
    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), 2, "{stdout}");
    assert!(rows.iter().all(|row| row.ends_with(",1,3")), "{stdout}");

    // Without --groups the report is exactly what it was.
    let plain = cli
        .cmd()
        .args([
            "folder",
            "geometric-match",
            "--folder-path",
            "/Parts",
            "--format",
            "csv",
            "--headers",
        ])
        .output()
        .unwrap();
    let plain = String::from_utf8_lossy(&plain.stdout);
    assert!(!plain.contains("GROUP_ID"), "{plain}");
    for (with, without) in stdout.lines().skip(1).zip(plain.lines().skip(1)) {
        assert_eq!(with.trim_end_matches(",1,3"), without);
    }
}

#[test]
fn the_excel_report_carries_a_summary_sheet() {
    let (cli, _mocks) = chain_fixture();
    let path = cli.dir.path().join("report.xlsx");
    let output = cli
        .cmd()
        .args([
            "folder",
            "geometric-match",
            "--folder-path",
            "/Parts",
            "--format",
            "xls",
            "--threshold",
            "95",
            "-o",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut workbook = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("xl/workbook.xml").unwrap(),
        &mut workbook,
    )
    .unwrap();
    assert!(workbook.contains("name=\"Summary\""), "{workbook}");
    let mut strings = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("xl/sharedStrings.xml").unwrap(),
        &mut strings,
    )
    .unwrap();
    for expected in [
        "Folder geometric match",
        "/Parts",
        "95.00%",
        "Groups of matching assets",
        "Match percentage",
    ] {
        assert!(strings.contains(expected), "missing {expected}");
    }
}

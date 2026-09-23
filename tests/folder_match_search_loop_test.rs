//! The search loop behind `folder geometric-match`, `part-match` and
//! `visual-match`, end to end against a mock API.
//!
//! The golden tests pin what a successful report looks like. These pin what the
//! loop does around it: which failures a run shrugs off and which make it exit
//! non-zero, when it stops searching altogether, which matches it drops
//! (self-matches, `--exclusive`), that concurrency does not change the report,
//! and that `--checkpoint` resumes where a failed run left off. Every scenario
//! runs against all three commands.

mod common;

use common::{asset_json, MockCli};
use mockito::{Matcher, Mock};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
struct Kind {
    command: &'static str,
    search: &'static str,
}

const KINDS: [Kind; 3] = [
    Kind {
        command: "geometric-match",
        search: "geometric",
    },
    Kind {
        command: "part-match",
        search: "part",
    },
    Kind {
        command: "visual-match",
        search: "visual",
    },
];

const PARTS: u8 = 200;
const OUTSIDE: u8 = 50;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn path(n: u8) -> String {
    if n == OUTSIDE {
        "/Other/x.stl".to_string()
    } else {
        format!("/Parts/p{n}.stl")
    }
}

/// /Parts holding assets 1..=count.
fn parts_folder(cli: &mut MockCli, count: u8) -> Vec<Mock> {
    let assets: Vec<_> = (1..=count)
        .map(|n| asset_json(id(n), &path(n), "finished", false))
        .collect();
    vec![
        cli.folders(&[(id(PARTS), "Parts", None)]),
        cli.assets_in(Some(id(PARTS)), &assets),
    ]
}

fn match_json(kind: Kind, n: u8, score: f64) -> serde_json::Value {
    let asset = asset_json(id(n), &path(n), "finished", false);
    match kind.search {
        "geometric" => serde_json::json!({"asset": asset, "matchPercentage": score}),
        "part" => serde_json::json!({
            "asset": asset,
            "forwardMatchPercentage": score,
            "reverseMatchPercentage": score
        }),
        _ => serde_json::json!({"asset": asset}),
    }
}

fn page(kind: Kind, matches: &[(u8, f64)]) -> String {
    let matches: Vec<_> = matches
        .iter()
        .map(|(n, score)| match_json(kind, *n, *score))
        .collect();
    serde_json::json!({
        "matches": matches,
        "pageData": {"total": matches.len(), "perPage": 500, "currentPage": 1, "lastPage": 1, "startIndex": 0, "endIndex": matches.len()}
    })
    .to_string()
}

/// The search request for one asset. Visual search is tenant-less and names the
/// asset in its body; the other two name it in the path.
fn search_request(cli: &mut MockCli, kind: Kind, n: u8) -> Mock {
    match kind.search {
        "visual" => cli
            .server
            .mock("POST", "/tenants/assets/visual-search")
            .match_body(Matcher::PartialJson(serde_json::json!({"assetId": id(n)}))),
        search => cli.server.mock(
            "POST",
            format!("/tenants/{}/assets/{}/{}-search", cli.tenant, id(n), search).as_str(),
        ),
    }
}

/// Asset `n`'s search answers with these matches.
fn finds(cli: &mut MockCli, kind: Kind, n: u8, matches: &[(u8, f64)]) -> Mock {
    search_request(cli, kind, n)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page(kind, matches))
        .create()
}

/// Asset `n`'s search fails with this status.
fn fails(cli: &mut MockCli, kind: Kind, n: u8, status: usize, message: &str) -> Mock {
    search_request(cli, kind, n)
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({"message": message}).to_string())
        .create()
}

fn run(cli: &MockCli, kind: Kind, extra: &[&str]) -> std::process::Output {
    let mut args = vec![
        "folder",
        kind.command,
        "--folder-path",
        "/Parts",
        "--threshold",
        "80",
        "--format",
        "csv",
    ];
    args.extend_from_slice(extra);
    cli.cmd().args(&args).output().unwrap()
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// The (reference, candidate) path pairs of a CSV report, in order.
fn pairs(report: &str) -> Vec<(String, String)> {
    report
        .lines()
        .map(|line| {
            let mut cells = line.split(',');
            (
                cells.next().unwrap().to_string(),
                cells.next().unwrap().to_string(),
            )
        })
        .collect()
}

fn pair(a: u8, b: u8) -> (String, String) {
    (path(a), path(b))
}

#[test]
fn an_unsearchable_asset_is_reported_and_the_run_still_succeeds() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 3);
        let _searches = [
            finds(&mut cli, kind, 1, &[(2, 99.0)]),
            finds(&mut cli, kind, 2, &[(1, 99.0), (3, 91.0)]),
            fails(&mut cli, kind, 3, 409, "Asset not indexed yet"),
        ];
        let output = run(&cli, kind, &[]);
        assert!(output.status.success(), "{kind:?}: {}", stderr(&output));
        assert!(
            stderr(&output).contains("Searched 2 of 3 asset(s): 1 not searchable"),
            "{kind:?}: {}",
            stderr(&output)
        );
        assert_eq!(
            pairs(&stdout(&output)),
            vec![pair(1, 2), pair(2, 3)],
            "{kind:?}"
        );
    }
}

#[test]
fn a_run_missing_more_than_a_tenth_of_its_searches_fails_without_a_report() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 3);
        let _searches = [
            finds(&mut cli, kind, 1, &[(2, 99.0)]),
            finds(&mut cli, kind, 2, &[(1, 99.0)]),
            fails(&mut cli, kind, 3, 500, "boom"),
        ];
        let output = run(&cli, kind, &[]);
        assert_eq!(output.status.code(), Some(69), "{kind:?}: {output:?}");
        assert!(
            stderr(&output)
                .contains("Searched 2 of 3 asset(s): 1 failed - the report would be incomplete"),
            "{kind:?}: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), "", "{kind:?}");
    }
}

#[test]
fn a_scattered_failure_under_a_tenth_is_a_warning() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 11);
        let mut searches = vec![finds(&mut cli, kind, 1, &[(2, 99.0)])];
        for n in 2..=10 {
            searches.push(finds(&mut cli, kind, n, &[]));
        }
        searches.push(fails(&mut cli, kind, 11, 500, "boom"));
        let output = run(&cli, kind, &[]);
        assert!(output.status.success(), "{kind:?}: {}", stderr(&output));
        assert!(
            stderr(&output).contains("Searched 10 of 11 asset(s): 1 failed"),
            "{kind:?}: {}",
            stderr(&output)
        );
        assert_eq!(pairs(&stdout(&output)), vec![pair(1, 2)], "{kind:?}");
    }
}

#[test]
fn three_authentication_failures_in_a_row_stop_the_run() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 6);
        // Every search is refused and the token cannot be renewed (the test
        // login has no client secret), so the credentials are really gone.
        let refused = match kind.search {
            "visual" => cli.server.mock("POST", "/tenants/assets/visual-search"),
            search => cli.server.mock(
                "POST",
                Matcher::Regex(format!("^/tenants/.+/assets/.+/{search}-search$")),
            ),
        }
        .with_status(401)
        .with_body(r#"{"message":"Unauthorized"}"#)
        .expect(3)
        .create();
        let output = run(&cli, kind, &["--concurrent", "1"]);
        assert_eq!(output.status.code(), Some(69), "{kind:?}: {output:?}");
        let err = stderr(&output);
        assert!(
            err.contains("Stopping after 3 consecutive authentication failures"),
            "{kind:?}: {err}"
        );
        assert!(err.contains("3 not attempted"), "{kind:?}: {err}");
        // The three remaining assets were never sent to the server.
        refused.assert();
    }
}

#[test]
fn self_matches_are_left_out() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 2);
        let _searches = [
            finds(&mut cli, kind, 1, &[(1, 100.0), (2, 99.0)]),
            finds(&mut cli, kind, 2, &[(2, 100.0), (1, 99.0)]),
        ];
        let output = run(&cli, kind, &[]);
        assert!(output.status.success(), "{kind:?}: {}", stderr(&output));
        assert_eq!(pairs(&stdout(&output)), vec![pair(1, 2)], "{kind:?}");
    }
}

#[test]
fn exclusive_keeps_only_matches_inside_the_folders() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 2);
        let _searches = [
            finds(&mut cli, kind, 1, &[(2, 99.0), (OUTSIDE, 95.0)]),
            finds(&mut cli, kind, 2, &[(1, 99.0)]),
        ];

        let everything = run(&cli, kind, &[]);
        assert!(
            everything.status.success(),
            "{kind:?}: {}",
            stderr(&everything)
        );
        assert_eq!(
            pairs(&stdout(&everything)),
            vec![pair(1, 2), pair(1, OUTSIDE)],
            "{kind:?}"
        );

        let exclusive = run(&cli, kind, &["--exclusive"]);
        assert!(
            exclusive.status.success(),
            "{kind:?}: {}",
            stderr(&exclusive)
        );
        assert_eq!(pairs(&stdout(&exclusive)), vec![pair(1, 2)], "{kind:?}");
    }
}

#[test]
fn concurrency_does_not_change_the_report() {
    for kind in KINDS {
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 5);
        let _searches = [
            finds(&mut cli, kind, 1, &[(2, 99.0), (3, 90.0)]),
            finds(&mut cli, kind, 2, &[(1, 98.0), (4, 85.0)]),
            finds(&mut cli, kind, 3, &[(1, 91.0), (5, 88.0)]),
            finds(&mut cli, kind, 4, &[(2, 86.0)]),
            finds(&mut cli, kind, 5, &[(3, 87.0), (1, 81.0)]),
        ];
        let one = run(&cli, kind, &["--concurrent", "1", "--headers"]);
        assert!(one.status.success(), "{kind:?}: {}", stderr(&one));
        for _ in 0..3 {
            let four = run(&cli, kind, &["--concurrent", "4", "--headers"]);
            assert!(four.status.success(), "{kind:?}: {}", stderr(&four));
            assert_eq!(stdout(&one), stdout(&four), "{kind:?}");
        }
        // Symmetric pairs are reported once.
        // 1-2, 1-3, 2-4, 3-5 and 1-5, under the header.
        assert_eq!(pairs(&stdout(&one)).len(), 1 + 5, "{kind:?}");
    }
}

#[test]
fn checkpoint_resumes_after_a_failed_run_without_searching_again() {
    for kind in KINDS {
        let responses: [(u8, &[(u8, f64)]); 3] = [
            (1, &[(2, 99.0)]),
            (2, &[(1, 99.0), (3, 91.0)]),
            (3, &[(2, 90.0)]),
        ];

        // What an uninterrupted run reports.
        let mut clean = MockCli::new();
        let _folder = parts_folder(&mut clean, 3);
        let _searches: Vec<_> = responses
            .iter()
            .map(|(n, found)| finds(&mut clean, kind, *n, found))
            .collect();
        let expected = run(&clean, kind, &["--headers"]);
        assert!(expected.status.success(), "{kind:?}: {}", stderr(&expected));

        // A run where asset 3 fails leaves a checkpoint behind...
        let mut cli = MockCli::new();
        let _folder = parts_folder(&mut cli, 3);
        let checkpoint = cli.dir.path().join("run.checkpoint");
        let checkpoint_arg = checkpoint.to_str().unwrap().to_string();
        let first = [
            finds(&mut cli, kind, 1, responses[0].1),
            finds(&mut cli, kind, 2, responses[1].1),
            fails(&mut cli, kind, 3, 500, "boom"),
        ];
        let failed = run(&cli, kind, &["--headers", "--checkpoint", &checkpoint_arg]);
        assert_eq!(failed.status.code(), Some(69), "{kind:?}: {failed:?}");
        assert!(checkpoint.exists(), "{kind:?}");
        drop(first);

        // ...and the re-run only searches asset 3, then reports everything.
        let again = [
            search_request(&mut cli, kind, 1).expect(0).create(),
            search_request(&mut cli, kind, 2).expect(0).create(),
            finds(&mut cli, kind, 3, responses[2].1),
        ];
        let resumed = run(&cli, kind, &["--headers", "--checkpoint", &checkpoint_arg]);
        assert!(resumed.status.success(), "{kind:?}: {}", stderr(&resumed));
        assert!(
            stderr(&resumed).contains("2 of 3 asset(s) already searched"),
            "{kind:?}: {}",
            stderr(&resumed)
        );
        assert_eq!(stdout(&resumed), stdout(&expected), "{kind:?}");
        again[0].assert();
        again[1].assert();
        assert!(
            !checkpoint.exists(),
            "{kind:?}: finished checkpoint is removed"
        );
    }
}

#[test]
fn visual_search_warns_when_an_asset_reaches_the_limit() {
    let kind = KINDS[2];
    let mut cli = MockCli::new();
    let _folder = parts_folder(&mut cli, 3);
    let _searches = [
        finds(&mut cli, kind, 1, &[(2, 0.0)]),
        finds(&mut cli, kind, 2, &[(1, 0.0), (3, 0.0)]),
        finds(&mut cli, kind, 3, &[]),
    ];
    let output = run(&cli, kind, &["--limit", "2"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let err = stderr(&output);
    assert!(
        err.contains("Visual search for asset p2.stl returned the --limit of 2 matches"),
        "{err}"
    );
    assert!(!err.contains("asset p1.stl returned"), "{err}");
}

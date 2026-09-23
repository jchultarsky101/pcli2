//! `asset list --recursive` and `folder download` run end to end against a mock API.

mod common;

use common::{asset_json, MockCli};
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

#[test]
fn recursive_asset_list_of_a_nested_folder_lists_its_subtree_only() {
    // /A/B/C, plus a top-level folder that is also called B. The old walk looked
    // up `B` and `B/C` from the root, so it listed the decoy (or failed).
    let mut cli = MockCli::new();
    let (a, b, c, decoy) = (id(1), id(2), id(3), id(4));
    let _folders = cli.folders(&[
        (a, "A", None),
        (b, "B", Some(a)),
        (c, "C", Some(b)),
        (decoy, "B", None),
    ]);
    let _b = cli.assets_in(
        Some(b),
        &[asset_json(id(10), "/A/B/b.stl", "finished", false)],
    );
    let _c = cli.assets_in(
        Some(c),
        &[asset_json(id(11), "/A/B/C/c.stl", "finished", false)],
    );
    let decoy_listing = cli
        .assets_in(
            Some(decoy),
            &[asset_json(id(12), "/B/decoy.stl", "finished", false)],
        )
        .expect(0);

    let output = cli
        .cmd()
        .args([
            "asset",
            "list",
            "--folder-path",
            "/A/B",
            "--recursive",
            "--format",
            "csv",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stdout.contains("/A/B/b.stl"), "{stdout}");
    assert!(stdout.contains("/A/B/C/c.stl"), "{stdout}");
    assert!(!stdout.contains("decoy"), "{stdout}");
    decoy_listing.assert();
}

#[test]
fn recursive_asset_list_of_the_root_includes_root_level_assets() {
    let mut cli = MockCli::new();
    let a = id(1);
    let _folders = cli.folders(&[(a, "A", None)]);
    let _root = cli.assets_in(None, &[asset_json(id(10), "/root.stl", "finished", false)]);
    let _a = cli.assets_in(
        Some(a),
        &[asset_json(id(11), "/A/a.stl", "finished", false)],
    );

    let output = cli
        .cmd()
        .args([
            "asset",
            "list",
            "--folder-path",
            "/",
            "--recursive",
            "--format",
            "csv",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("/root.stl"), "{stdout}");
    assert!(stdout.contains("/A/a.stl"), "{stdout}");
}

#[test]
fn folder_download_of_the_root_keeps_raw_assemblies_and_names_what_it_skipped() {
    // `/` used to fail with "folder not found"; an assembly without a bundle
    // failed with a ZIP error; a failed asset vanished without a word.
    let mut cli = MockCli::new();
    let (part, valve, broken) = (id(10), id(11), id(12));
    let _folders = cli.folders(&[]);
    let _root = cli.assets_in(
        None,
        &[
            asset_json(part, "/part.stl", "finished", false),
            asset_json(valve, "/valve.asm", "missing-dependencies", true),
            asset_json(broken, "/broken.stl", "failed", false),
        ],
    );
    let _sub = cli.subfolders_of(None, &[]);
    let _part = cli.file(part, b"solid part");
    let _valve = cli.file(valve, b"#UGC:2 ASSEMBLY valve");
    let broken_file = cli.file(broken, b"never asked for").expect(0);

    let out = cli.dir.path().join("out");
    let output = cli
        .cmd()
        .args(["folder", "download", "--folder-path", "/", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert_eq!(std::fs::read(out.join("part.stl")).unwrap(), b"solid part");
    assert_eq!(
        std::fs::read(out.join("valve.asm")).unwrap(),
        b"#UGC:2 ASSEMBLY valve"
    );
    assert!(!out.join("valve.asm.zip").exists());
    assert!(stderr.contains("/broken.stl (failed)"), "stderr: {stderr}");
    broken_file.assert();
}

#[test]
fn folder_download_resume_skips_a_complete_assembly_bundle() {
    use std::io::Write;
    let mut bundle = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bundle);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        writer.start_file("body.prt", options).unwrap();
        writer.write_all(b"part").unwrap();
        writer.start_file("valve.asm", options).unwrap();
        writer.write_all(b"assembly").unwrap();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"outside").unwrap();
        writer.finish().unwrap();
    }
    let bundle = bundle.into_inner();

    let mut cli = MockCli::new();
    let valve = id(11);
    let _folders = cli.folders(&[]);
    let _root = cli.assets_in(None, &[asset_json(valve, "/valve.asm", "finished", true)]);
    let _sub = cli.subfolders_of(None, &[]);
    let file = cli.file(valve, &bundle).expect(1);

    let out = cli.dir.path().join("out");
    for _ in 0..2 {
        let output = cli
            .cmd()
            .args([
                "folder",
                "download",
                "--folder-path",
                "/Home",
                "--resume",
                "-o",
            ])
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(std::fs::read(out.join("valve.asm")).unwrap(), b"assembly");
    assert_eq!(std::fs::read(out.join("body.prt")).unwrap(), b"part");
    // An entry that points outside the directory is skipped, not written.
    assert!(!cli.dir.path().join("escape.txt").exists());
    assert!(!out.join("escape.txt").exists());
    // Nothing is left of the staging directory or the archive.
    let leftovers: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".pcli2-extract") || n.ends_with(".zip"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // The second run found the assembly complete and did not download it again.
    file.assert();
}

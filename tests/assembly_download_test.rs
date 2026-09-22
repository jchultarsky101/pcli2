//! An assembly downloads as its dependency bundle when Physna has one, and as
//! the raw source file when it does not (for example in the
//! `missing-dependencies` state). Both bodies must end up on disk under a
//! sensible name.

use pcli2::actions::assets::download::{download_assembly, AssemblyDownload};
use pcli2::error::CliError;
use pcli2::physna_v3::PhysnaApiClient;
use std::io::Write;
use uuid::Uuid;

/// A ZIP holding `ballvalve.asm` and one part, the way a bundle is served.
fn bundle_bytes() -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        writer.start_file("ballvalve.asm", options).unwrap();
        writer.write_all(b"assembly source").unwrap();
        writer.start_file("body.prt", options).unwrap();
        writer.write_all(b"part source").unwrap();
        writer.finish().unwrap();
    }
    cursor.into_inner()
}

async fn serve(body: Vec<u8>) -> (mockito::ServerGuard, String, String, mockito::Mock) {
    let mut server = mockito::Server::new_async().await;
    let tenant = Uuid::new_v4().to_string();
    let asset = Uuid::new_v4().to_string();
    let mock = server
        .mock(
            "GET",
            format!("/tenants/{}/assets/{}/file", tenant, asset).as_str(),
        )
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await;
    (server, tenant, asset, mock)
}

#[tokio::test]
async fn an_assembly_without_a_bundle_is_saved_as_the_raw_source_file() {
    // What Physna sends for an assembly in `missing-dependencies`: the .asm
    // itself, not a ZIP. This used to fail with "Could not find EOCD" and
    // leave the file behind as `ballvalve.asm.zip`.
    let raw: Vec<u8> = b"#UGC:2 ASSEMBLY ballvalve\n"
        .iter()
        .copied()
        .chain((0..50_000u32).map(|i| (i % 253) as u8))
        .collect();
    let (server, tenant, asset, mock) = serve(raw.clone()).await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("ballvalve.asm");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let outcome = download_assembly(&mut client, &tenant, &asset, "ballvalve.asm", &dest)
        .await
        .unwrap();

    mock.assert_async().await;
    assert_eq!(outcome, AssemblyDownload::RawFile { file: dest.clone() });
    assert_eq!(std::fs::read(&dest).unwrap(), raw);
    assert!(!dir.path().join("ballvalve.asm.zip").exists());
    assert!(!dir.path().join("ballvalve.asm.zip.part").exists());
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["ballvalve.asm".to_string()]);
}

#[tokio::test]
async fn a_dependency_bundle_is_still_extracted_and_the_archive_removed() {
    let (server, tenant, asset, mock) = serve(bundle_bytes()).await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("ballvalve.asm");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let outcome = download_assembly(&mut client, &tenant, &asset, "ballvalve.asm", &dest)
        .await
        .unwrap();

    mock.assert_async().await;
    let archive = dir.path().join("ballvalve.asm.zip");
    assert_eq!(
        outcome,
        AssemblyDownload::Extracted {
            archive: archive.clone()
        }
    );
    assert_eq!(std::fs::read(&dest).unwrap(), b"assembly source");
    assert_eq!(
        std::fs::read(dir.path().join("body.prt")).unwrap(),
        b"part source"
    );
    assert!(!archive.exists());
}

#[tokio::test]
async fn a_raw_file_lands_in_the_requested_output_directory() {
    // `-o some/dir/name.asm` for an assembly without a bundle: the raw file
    // takes the requested name, and the directory is created on the way.
    let raw = b"raw assembly".to_vec();
    let (server, tenant, asset, _mock) = serve(raw.clone()).await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("out").join("renamed.asm");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let outcome = download_assembly(&mut client, &tenant, &asset, "ballvalve.asm", &dest)
        .await
        .unwrap();

    assert_eq!(outcome, AssemblyDownload::RawFile { file: dest.clone() });
    assert_eq!(std::fs::read(&dest).unwrap(), raw);
    assert!(!dir.path().join("out").join("renamed.asm.zip").exists());
}

#[tokio::test]
async fn a_truncated_bundle_is_still_reported_as_a_zip_error() {
    // A body that claims to be a ZIP but is cut short is a broken bundle, not
    // a raw source file: the error must surface rather than be hidden by a
    // rename.
    let mut truncated = bundle_bytes();
    truncated.truncate(40);
    let (server, tenant, asset, _mock) = serve(truncated).await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("ballvalve.asm");
    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let result = download_assembly(&mut client, &tenant, &asset, "ballvalve.asm", &dest).await;

    assert!(
        matches!(
            result,
            Err(CliError::ActionError(
                pcli2::actions::CliActionError::ZipError(_)
            ))
        ),
        "expected a ZIP error, got {:?}",
        result.map(|_| ())
    );
    assert!(!dest.exists());
}

//! Output-shape checks for every formatter.
//!
//! Every type that can be printed is formatted in every output format it
//! supports, and the result is checked for the properties scripts rely on:
//!
//! - CSV parses, every row has as many fields as the header, `--headers`
//!   adds exactly one line, and there is no empty line and no trailing line
//!   break (the 1.23.x regression: `| wc -l` over-counted by one).
//! - JSON parses, compact output is one line, pretty output is the same value.
//! - Tree output is non-empty and has no trailing line break.
//! - A format a type does not support is a clean error, never a panic.
//!
//! The table of supported formats is the current behaviour; a change to it is
//! a change to the CLI's contract and should show up here.

use pcli2::dependency_diff::DependencyDiff;
use pcli2::format::{
    Formattable, FormattingError, OutputFormat, OutputFormatOptions, OutputFormatter,
};
use pcli2::model::*;
use serde_json::json;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Fmt {
    Csv,
    Json,
    Tree,
}

const ALL: [Fmt; 3] = [Fmt::Csv, Fmt::Json, Fmt::Tree];

fn options(with_headers: bool, pretty: bool) -> OutputFormatOptions {
    OutputFormatOptions {
        with_metadata: false,
        with_headers,
        pretty,
    }
}

fn output_format(fmt: Fmt, with_headers: bool, pretty: bool) -> OutputFormat {
    let options = options(with_headers, pretty);
    match fmt {
        Fmt::Csv => OutputFormat::Csv(options),
        Fmt::Json => OutputFormat::Json(options),
        Fmt::Tree => OutputFormat::Tree(options),
    }
}

/// Run `format` for every format in `ALL` and check the shape of what comes
/// back against `supported`.
fn check_shapes(
    label: &str,
    supported: &[Fmt],
    allow_empty: bool,
    format: impl Fn(OutputFormat) -> Result<String, FormattingError>,
) {
    for fmt in ALL {
        let plain = format(output_format(fmt, false, false));
        if !supported.contains(&fmt) {
            match plain {
                Err(FormattingError::UnsupportedOutputFormat(_)) => continue,
                Ok(out) => panic!("{label}: {fmt:?} is listed as unsupported but produced {out:?}"),
                Err(other) => panic!(
                    "{label}: {fmt:?} failed with {other:?} instead of UnsupportedOutputFormat"
                ),
            }
        }
        let plain = plain.unwrap_or_else(|e| panic!("{label}: {fmt:?} failed: {e:?}"));
        if plain.is_empty() {
            // An empty collection may legitimately format to nothing (the print
            // helper then prints nothing at all); a non-empty value may not.
            assert!(allow_empty, "{label}: {fmt:?} produced nothing");
            continue;
        }
        assert!(
            !plain.ends_with('\n'),
            "{label}: {fmt:?} ends with a line break, println! would add a second one: {plain:?}"
        );

        match fmt {
            Fmt::Csv => {
                let with_headers = format(output_format(fmt, true, false)).unwrap();
                check_csv(label, &plain, &with_headers);
            }
            Fmt::Json => {
                let pretty = format(output_format(fmt, false, true)).unwrap();
                check_json(label, &plain, &pretty);
            }
            Fmt::Tree => {
                assert!(
                    !plain.trim().is_empty(),
                    "{label}: tree output is blank: {plain:?}"
                );
            }
        }
    }
}

fn check_csv(label: &str, without_headers: &str, with_headers: &str) {
    for (variant, out) in [("no headers", without_headers), ("headers", with_headers)] {
        assert!(
            !out.lines().any(|line| line.trim().is_empty()),
            "{label}: CSV ({variant}) contains an empty line: {out:?}"
        );
        assert!(
            !out.ends_with('\n'),
            "{label}: CSV ({variant}) ends with a line break: {out:?}"
        );
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(false)
            .from_reader(out.as_bytes());
        for record in reader.records() {
            record.unwrap_or_else(|e| panic!("{label}: CSV ({variant}) is ragged: {e}\n{out}"));
        }
    }
    let rows = |out: &str| out.lines().count();
    assert_eq!(
        rows(with_headers),
        rows(without_headers) + 1,
        "{label}: --headers must add exactly one line\nwith:\n{with_headers}\nwithout:\n{without_headers}"
    );
}

fn check_json(label: &str, compact: &str, pretty: &str) {
    let compact_value: serde_json::Value = serde_json::from_str(compact)
        .unwrap_or_else(|e| panic!("{label}: compact JSON does not parse: {e}\n{compact}"));
    assert!(
        !compact.contains('\n'),
        "{label}: compact JSON spans several lines: {compact:?}"
    );
    let pretty_value: serde_json::Value = serde_json::from_str(pretty)
        .unwrap_or_else(|e| panic!("{label}: pretty JSON does not parse: {e}\n{pretty}"));
    assert_eq!(
        compact_value, pretty_value,
        "{label}: pretty and compact JSON differ in content"
    );
}

fn shapes<T: OutputFormatter>(label: &str, value: &T, supported: &[Fmt]) {
    check_shapes(label, supported, false, |f| value.format(f));
}

fn shapes_of_empty<T: OutputFormatter>(label: &str, value: &T, supported: &[Fmt]) {
    check_shapes(label, supported, true, |f| value.format(f));
}

fn formattable_shapes<T: Formattable>(label: &str, value: &T, supported: &[Fmt]) {
    check_shapes(label, supported, false, |f| value.format(&f));
}

fn from<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("fixture should deserialize")
}

// ---- fixtures ---------------------------------------------------------------

const A1: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const A2: &str = "aaaaaaaa-0000-0000-0000-000000000002";
const TENANT: &str = "22222222-2222-2222-2222-222222222222";

fn asset_json(uuid: &str, name: &str) -> serde_json::Value {
    json!({
        "uuid": uuid,
        "name": name,
        "path": format!("/Parts/{name}"),
        "file_size": 1024,
        "file_type": "STL",
        "processing_status": "finished",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-02T00:00:00Z",
        "metadata": {"material": "steel", "note": "has, a comma"},
        "is_assembly": false
    })
}

fn asset_response_json(uuid: &str, name: &str) -> serde_json::Value {
    json!({
        "id": uuid,
        "tenantId": TENANT,
        "path": format!("/Parts/{name}"),
        "type": "STL",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-02T00:00:00Z",
        "state": "finished",
        "isAssembly": false,
        "metadata": {"material": "steel"}
    })
}

#[test]
fn asset_and_asset_list() {
    let asset: Asset = from(asset_json(A1, "bracket.stl"));
    shapes("Asset", &asset, &[Fmt::Csv, Fmt::Json]);

    let list = AssetList::from(vec![
        from::<Asset>(asset_json(A1, "bracket.stl")),
        from::<Asset>(asset_json(A2, "housing.stl")),
    ]);
    shapes("AssetList", &list, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);
    shapes_of_empty(
        "AssetList (empty)",
        &AssetList::from(Vec::<Asset>::new()),
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );
}

#[test]
fn assets_with_thumbnails() {
    let mut one = asset_json(A1, "bracket.stl");
    one["thumbnail_url"] = json!("https://example.invalid/thumb/1.png");
    let with_thumbnail: AssetWithThumbnail = from(one.clone());
    shapes(
        "AssetWithThumbnail",
        &with_thumbnail,
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );

    let list: AssetListWithThumbnails = from(json!({"assets": [one]}));
    shapes(
        "AssetListWithThumbnails",
        &list,
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );
}

#[test]
fn asset_metadata() {
    let metadata: AssetMetadata = from(json!({"material": "steel", "note": "with \"quotes\""}));
    shapes("AssetMetadata", &metadata, &[Fmt::Csv, Fmt::Json]);
}

#[test]
fn folders() {
    let folder: Folder = from(json!({
        "id": A1, "name": "Parts", "path": "/Parts", "assetsCount": 2, "foldersCount": 1
    }));
    shapes("Folder", &folder, &[Fmt::Csv, Fmt::Json]);

    let list: FolderList = from(json!({"folders": [
        {"id": A1, "name": "Parts", "path": "/Parts", "assetsCount": 2, "foldersCount": 1},
        {"id": A2, "name": "Sub", "path": "/Parts/Sub", "assetsCount": 0, "foldersCount": 0}
    ]}));
    shapes("FolderList", &list, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);
}

#[test]
fn tenants() {
    let tenant: Tenant = from(json!({"id": TENANT, "name": "acme", "description": "Acme, Inc."}));
    formattable_shapes("Tenant", &tenant, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);

    let list: TenantList = from(json!({"tenants": [
        {"id": TENANT, "name": "acme", "description": "Acme, Inc."}
    ]}));
    shapes("TenantList", &list, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);
}

#[test]
fn metadata_fields() {
    let fields: MetadataFieldListResponse = from(json!({
        "metadataFields": [{"name": "material", "type": "string"}, {"name": "weight", "type": "number"}],
        "pageData": null
    }));
    shapes(
        "MetadataFieldListResponse",
        &fields,
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );
}

#[test]
fn geometric_match_types() {
    let enhanced: EnhancedGeometricSearchResponse = from(json!({
        "reference_asset": asset_response_json(A1, "bracket.stl"),
        "matches": [{
            "asset": asset_response_json(A2, "housing.stl"),
            "matchPercentage": 91.5,
            "transformation": null,
            "comparisonUrl": "https://example.invalid/compare"
        }]
    }));
    shapes(
        "EnhancedGeometricSearchResponse",
        &enhanced,
        &[Fmt::Csv, Fmt::Json],
    );

    let pair: GeometricMatchPair = from(json!({
        "referenceAsset": asset_response_json(A1, "bracket.stl"),
        "candidateAsset": asset_response_json(A2, "housing.stl"),
        "matchPercentage": 91.5,
        "transformation": null,
        "comparisonUrl": "https://example.invalid/compare"
    }));
    shapes("GeometricMatchPair", &pair, &[Fmt::Csv, Fmt::Json]);

    let folder_matches: FolderGeometricMatchResponse = from(json!([{
        "referenceAssetName": "bracket.stl",
        "candidateAssetName": "housing.stl",
        "matchPercentage": 91.5,
        "referenceAssetPath": "/Parts/bracket.stl",
        "candidateAssetPath": "/Parts/housing.stl",
        "referenceAssetUuid": A1,
        "candidateAssetUuid": A2,
        "comparisonUrl": "https://example.invalid/compare"
    }]));
    shapes(
        "FolderGeometricMatchResponse",
        &folder_matches,
        &[Fmt::Csv, Fmt::Json],
    );

    let similarity: AssetSimilarity = from(json!({
        "referenceAssetPath": "/Parts/bracket.stl",
        "referenceAssetUuid": A1,
        "candidateAssetPath": "/Parts/housing.stl",
        "candidateAssetUuid": A2,
        "geometric": {"matchPercentage": 91.5, "forwardMatchPercentage": 90.0, "reverseMatchPercentage": 93.0},
        "volumetric": {"matchPercentage": 88.0},
        "comparisonUrl": "https://example.invalid/compare"
    }));
    shapes("AssetSimilarity", &similarity, &[Fmt::Csv, Fmt::Json]);
}

#[test]
fn part_and_text_match_types() {
    let part: EnhancedPartSearchResponse = from(json!({
        "referenceAsset": asset_response_json(A1, "bracket.stl"),
        "matches": [{
            "asset": asset_response_json(A2, "housing.stl"),
            "forwardMatchPercentage": 80.0,
            "reverseMatchPercentage": 20.0,
            "transformation": null,
            "comparisonUrl": "https://example.invalid/compare"
        }]
    }));
    shapes("EnhancedPartSearchResponse", &part, &[Fmt::Csv, Fmt::Json]);

    let text: TextMatchPair = from(json!({
        "referenceAsset": asset_response_json(A1, "bracket.stl"),
        "candidateAsset": asset_response_json(A2, "housing.stl"),
        "relevanceScore": 0.75,
        "comparisonUrl": "https://example.invalid/compare"
    }));
    shapes("TextMatchPair", &text, &[Fmt::Csv, Fmt::Json]);
}

#[test]
fn dependency_types() {
    let dependencies: AssetDependencyList = from(json!({
        "path": "/Assemblies/top.asm",
        "dependencies": [
            {"path": "/Parts/bracket.stl", "asset": asset_response_json(A1, "bracket.stl"),
             "occurrences": 2, "hasDependencies": false, "assemblyPath": "/Assemblies/top.asm"},
            {"path": "/Parts/missing.stl", "occurrences": 1, "hasDependencies": false,
             "assemblyPath": "/Assemblies/top.asm"}
        ]
    }));
    shapes(
        "AssetDependencyList",
        &dependencies,
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );

    let node_json = json!({
        "asset": asset_json(A1, "top.asm"),
        "children": [
            {"asset": asset_json(A2, "bracket.stl"), "children": null}
        ]
    });
    let node: AssemblyNode = from(node_json.clone());
    shapes("AssemblyNode", &node, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);

    let tree: AssemblyTree = from(json!({"root": node_json}));
    shapes("AssemblyTree", &tree, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);
}

#[test]
fn dependency_diff() {
    let diff: DependencyDiff = from(json!({
        "reference": "/Assemblies/v1.asm",
        "candidate": "/Assemblies/v2.asm",
        "summary": {"common": 1, "only_in_reference": 1, "only_in_candidate": 0},
        "nodes": [
            {"filename": "bracket.stl", "status": "common", "children": []},
            {"filename": "old.stl", "status": "only_in_reference", "uuid": A2, "children": []}
        ]
    }));
    shapes("DependencyDiff", &diff, &[Fmt::Csv, Fmt::Json, Fmt::Tree]);
}

#[test]
fn health_and_state_reports() {
    let health: AssetHealthReport = from(json!({
        "total": 3, "finished": 2, "indexing": 0, "failed": 1, "unsupported": 0,
        "no_3d_data": 0, "missing_dependencies": 0, "assemblies": 1, "parts": 2,
        "file_types": {"STL": 2, "ASM": 1}
    }));
    formattable_shapes(
        "AssetHealthReport",
        &health,
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );

    let state: AssetStateCounts = from(json!({
        "indexing": 1, "finished": 10, "failed": 2, "unsupported": 0, "no-3d-data": 0
    }));
    formattable_shapes(
        "AssetStateCounts",
        &state,
        &[Fmt::Csv, Fmt::Json, Fmt::Tree],
    );
}

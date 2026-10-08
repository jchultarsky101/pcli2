//! Contract tests against Physna's OpenAPI specification.
//!
//! The model structs in `pcli2::model` are hand-written from the API's
//! behaviour. Nothing tied them to the specification, so a field Physna
//! renamed or stopped sending showed up as a deserialization error in the
//! field, in a command that had worked the day before.
//!
//! These tests read a snapshot of the specification
//! (`tests/fixtures/physna-openapi.json`, taken from the JSON embedded in
//! `https://app-api.physna.com/v3/docs/swagger-ui-init.js`) and, for every
//! endpoint the client calls, generate a response body from the spec's schema
//! and deserialize it into the struct the client uses:
//!
//! - with only the properties the spec marks `required`, which catches a Rust
//!   field that is mandatory while the API may omit it;
//! - with every property present, which catches a type the two sides
//!   disagree on.
//!
//! The enumerations the code hard-codes (asset states, metadata field types,
//! tenant roles) are compared with the spec's, and every endpoint the client
//! builds a URL for must exist.
//!
//! `live_spec_matches_the_snapshot` (ignored by default; the `spec-drift`
//! workflow runs it weekly) fetches the current specification and reports any
//! change to the schemas and endpoints these tests depend on, so drift is
//! noticed before a user meets it. When it reports drift, review the diff it
//! prints, refresh the fixture with the command it prints (the fixture is the
//! spec on one line, in Physna's key order, as Python's `json.dump` writes
//! it), and re-run this file to see whether the model still fits. When it
//! reports that the spec was unavailable, the docs endpoint was down: re-run
//! the workflow later.

use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/physna-openapi.json"
);
const LIVE_URL: &str = "https://app-api.physna.com/v3/docs/swagger-ui-init.js";
/// Rewrites the live spec in the fixture's format, keeping the diff to what changed.
const REFRESH: &str = "python3 -c 'import json; json.dump(json.load(open(\"target/physna-openapi.live.json\")), open(\"tests/fixtures/physna-openapi.json\", \"w\"))'";

fn spec() -> Value {
    let text = std::fs::read_to_string(FIXTURE).expect("spec fixture");
    serde_json::from_str(&text).expect("spec fixture is JSON")
}

// ---- sample generation ------------------------------------------------------

/// How a response body is generated from a schema.
#[derive(Clone, Copy)]
struct Shape<'a> {
    /// Include optional properties too (otherwise only `required` ones).
    everything: bool,
    /// Schema names to prefer when a property is `anyOf` several shapes: the
    /// folder-contents endpoint returns folders or assets, and the client asks
    /// for assets.
    prefer: &'a [&'a str],
}

fn resolve<'s>(spec: &'s Value, reference: &str) -> &'s Value {
    let name = reference.rsplit('/').next().unwrap();
    spec["components"]["schemas"]
        .get(name)
        .unwrap_or_else(|| panic!("unresolved $ref {reference}"))
}

fn ref_name(schema: &Value) -> Option<&str> {
    schema["$ref"]
        .as_str()
        .map(|r| r.rsplit('/').next().unwrap())
}

/// Does this alternative (possibly an allOf) mention one of the preferred schemas?
fn mentions(schema: &Value, prefer: &[&str]) -> bool {
    if let Some(name) = ref_name(schema) {
        return prefer.contains(&name);
    }
    schema["allOf"]
        .as_array()
        .map(|parts| parts.iter().any(|p| mentions(p, prefer)))
        .unwrap_or(false)
}

fn sample(spec: &Value, schema: &Value, shape: Shape, depth: usize) -> Value {
    if depth > 12 {
        return Value::Null;
    }
    if let Some(reference) = schema["$ref"].as_str() {
        return sample(spec, resolve(spec, reference), shape, depth + 1);
    }
    if let Some(parts) = schema["allOf"].as_array() {
        let mut merged = Map::new();
        for part in parts {
            if let Value::Object(fields) = sample(spec, part, shape, depth + 1) {
                merged.extend(fields);
            }
        }
        return Value::Object(merged);
    }
    if let Some(alternatives) = schema["anyOf"]
        .as_array()
        .or_else(|| schema["oneOf"].as_array())
    {
        let chosen = alternatives
            .iter()
            .find(|alt| mentions(alt, shape.prefer))
            .unwrap_or(&alternatives[0]);
        return sample(spec, chosen, shape, depth + 1);
    }
    if let Some(values) = schema["enum"].as_array() {
        return values[0].clone();
    }
    match schema["type"].as_str() {
        Some("string") => {
            let is_uuid = schema["format"].as_str() == Some("uuid")
                || schema["pattern"]
                    .as_str()
                    .map(|p| p.contains("[0-9A-Fa-f]{8}"))
                    .unwrap_or(false);
            if is_uuid {
                json!("2f9d0a2e-6a55-4d26-9a0a-1c9fd0e2c4b1")
            } else if schema["format"].as_str() == Some("date-time") {
                json!("2026-01-01T00:00:00.000Z")
            } else {
                json!("text")
            }
        }
        Some("integer") | Some("number") => json!(1),
        Some("boolean") => json!(true),
        Some("array") => json!([sample(spec, &schema["items"], shape, depth + 1)]),
        Some("object") | None => {
            let required: BTreeSet<&str> = schema["required"]
                .as_array()
                .map(|r| r.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let mut object = Map::new();
            if let Some(properties) = schema["properties"].as_object() {
                for (name, property) in properties {
                    if shape.everything || required.contains(name.as_str()) {
                        object.insert(name.clone(), sample(spec, property, shape, depth + 1));
                    }
                }
            }
            if let Some(additional) = schema["additionalProperties"].as_object() {
                object.insert(
                    "key".into(),
                    sample(spec, &Value::Object(additional.clone()), shape, depth + 1),
                );
            }
            Value::Object(object)
        }
        Some(other) => panic!("unhandled schema type {other}"),
    }
}

fn response_schema<'s>(spec: &'s Value, method: &str, path: &str) -> &'s Value {
    let operation = &spec["paths"][path][method];
    assert!(
        operation.is_object(),
        "{} {} is not in the specification",
        method.to_uppercase(),
        path
    );
    let responses = &operation["responses"];
    let ok = responses
        .get("200")
        .or_else(|| responses.get("201"))
        .unwrap_or_else(|| panic!("{method} {path} has no 200/201 response"));
    &ok["content"]["application/json"]["schema"]
}

// ---- the contract ----------------------------------------------------------

/// One endpoint the client calls, and the type it deserializes the body into.
struct Contract {
    method: &'static str,
    path: &'static str,
    prefer: &'static [&'static str],
    /// Values to substitute into the generated body, by JSON pointer, where
    /// the specification is looser than what the API actually sends. Each one
    /// is a documented, deliberate deviation.
    fixups: &'static [(&'static str, &'static str)],
    check: fn(&Value, Shape) -> Result<Value, String>,
}

/// The spec types `TenantUserSettings.tenantId` as a bare string; the API
/// sends a UUID, and the tenant cache and every `--tenant` lookup key on it.
const TENANT_ID_IS_A_UUID: &[(&str, &str)] = &[(
    "/user/settings/0/tenantId",
    "2f9d0a2e-6a55-4d26-9a0a-1c9fd0e2c4b1",
)];

fn deserializes<T: DeserializeOwned + serde::Serialize>(
    body: &Value,
    _shape: Shape,
) -> Result<Value, String> {
    let value: T = serde_json::from_value(body.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&value).map_err(|e| e.to_string())
}

macro_rules! contract {
    ($method:literal, $path:literal, $ty:ty) => {
        Contract {
            method: $method,
            path: $path,
            prefer: &[],
            fixups: &[],
            check: deserializes::<$ty>,
        }
    };
    ($method:literal, $path:literal, $ty:ty, prefer $prefer:expr) => {
        Contract {
            method: $method,
            path: $path,
            prefer: $prefer,
            fixups: &[],
            check: deserializes::<$ty>,
        }
    };
    ($method:literal, $path:literal, $ty:ty, fixups $fixups:expr) => {
        Contract {
            method: $method,
            path: $path,
            prefer: &[],
            fixups: $fixups,
            check: deserializes::<$ty>,
        }
    };
}

fn contracts() -> Vec<Contract> {
    use pcli2::actions::users::{SingleUserResponse, UserListResponse};
    use pcli2::model::*;
    vec![
        contract!(
            "get",
            "/users/me",
            CurrentUserResponse,
            fixups TENANT_ID_IS_A_UUID
        ),
        contract!("get", "/tenants/{tenantId}/folders", FolderListResponse),
        contract!("post", "/tenants/{tenantId}/folders", SingleFolderResponse),
        contract!(
            "get",
            "/tenants/{tenantId}/folders/{folderId}",
            SingleFolderResponse
        ),
        contract!(
            "patch",
            "/tenants/{tenantId}/folders/{folderId}/name",
            SingleFolderResponse
        ),
        contract!(
            "patch",
            "/tenants/{tenantId}/folders/{folderId}/parent",
            SingleFolderResponse
        ),
        contract!(
            "patch",
            "/tenants/{tenantId}/folders/{folderId}/description",
            SingleFolderResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/folders/{folderId}/contents",
            AssetListResponse,
            prefer & ["Asset"]
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/folders/root/contents",
            AssetListResponse,
            prefer & ["Asset"]
        ),
        contract!("get", "/tenants/{tenantId}/assets", AssetListResponse),
        contract!("post", "/tenants/{tenantId}/assets", AssetResponse),
        contract!(
            "post",
            "/tenants/{tenantId}/assets/batch",
            AssetListResponse
        ),
        contract!(
            "patch",
            "/tenants/{tenantId}/assets/{assetId}/folder",
            SingleAssetResponse
        ),
        contract!(
            "put",
            "/tenants/{tenantId}/assets/{assetId}/file",
            AssetResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/assets/{assetId}",
            SingleAssetResponse
        ),
        contract!("get", "/tenants/{tenantId}/assets/state", AssetStateCounts),
        contract!(
            "get",
            "/tenants/{tenantId}/assets/state/{assetState}",
            AssetListResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/metadata-fields",
            MetadataFieldListResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/metadata-fields/{fieldId}/assets",
            AssetListResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/assets/without-metadata",
            AssetListResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/metadata-coverage",
            MetadataCoverageResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/activity-metrics",
            ActivityMetrics
        ),
        contract!("get", "/tenants/{tenantId}/assets/type-counts", Counts),
        contract!(
            "post",
            "/tenants/{tenantId}/assets/{assetId}/geometric-search",
            GeometricSearchResponse
        ),
        contract!(
            "post",
            "/tenants/{tenantId}/assets/{assetId}/part-search",
            PartSearchResponse
        ),
        contract!("post", "/tenants/assets/visual-search", PartSearchResponse),
        // `pcli2 catalog`: one page type for all three cross-tenant searches.
        contract!(
            "post",
            "/tenants/assets/geometric-search",
            CatalogSearchPage
        ),
        contract!("post", "/tenants/assets/part-search", CatalogSearchPage),
        contract!("post", "/tenants/assets/visual-search", CatalogSearchPage),
        contract!(
            "post",
            "/tenants/{tenantId}/assets/text-search",
            TextSearchResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/assets/{assetId}/match-scores/{targetAssetId}",
            MatchScoresResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/assets/{assetId}/dependencies-by-id",
            AssetDependenciesResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/assets/{assetId}/failure-diagnostics",
            FailureDiagnostics
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/failure-diagnostics/availability",
            FailureDiagnosticsAvailability
        ),
        contract!("get", "/tenants/{tenantId}/failures", RecentFailuresPage),
        contract!("get", "/tenants/{tenantId}/reports", ReportListResponse),
        contract!(
            "get",
            "/tenants/{tenantId}/reports/{id}",
            SingleReportResponse
        ),
        contract!(
            "post",
            "/tenants/{tenantId}/reports/duplication",
            SingleReportResponse
        ),
        contract!(
            "get",
            "/tenants/{tenantId}/reports/{id}/failure-diagnostics",
            FailureDiagnostics
        ),
        contract!(
            "post",
            "/tenants/{tenantId}/assets/existing-paths",
            ExistingPathsResponse
        ),
        contract!("get", "/tenants/{tenantId}/users", UserListResponse),
        contract!(
            "get",
            "/tenants/{tenantId}/users/{userId}",
            SingleUserResponse
        ),
    ]
}

/// Endpoints the client calls whose body it does not deserialize (or that
/// return nothing). They still have to exist.
const OTHER_ENDPOINTS: &[(&str, &str)] = &[
    ("delete", "/tenants/{tenantId}/folders/{folderId}"),
    (
        "delete",
        "/tenants/{tenantId}/folders/{folderId}/description",
    ),
    ("delete", "/tenants/{tenantId}/assets/{assetId}"),
    ("patch", "/tenants/{tenantId}/assets/{assetId}"),
    ("delete", "/tenants/{tenantId}/assets/{assetId}/metadata"),
    ("get", "/tenants/{tenantId}/assets/{assetId}/file"),
    ("get", "/tenants/{tenantId}/assets/{assetId}/thumbnail.png"),
    ("post", "/tenants/{tenantId}/assets/{assetId}/reprocess"),
    (
        "post",
        "/tenants/{tenantId}/assets/{assetId}/resolve-dependency",
    ),
    ("post", "/tenants/{tenantId}/metadata-fields"),
    ("patch", "/tenants/{tenantId}/metadata-fields/{fieldId}"),
    ("delete", "/tenants/{tenantId}/metadata-fields/{fieldId}"),
    ("delete", "/tenants/{tenantId}/reports/{id}"),
    ("get", "/tenants/{tenantId}/reports/{id}/file"),
];

/// Every endpoint above, for the drift check.
fn used_paths(spec: &Value) -> BTreeSet<String> {
    let mut paths: BTreeSet<String> = contracts()
        .iter()
        .map(|c| c.path.to_string())
        .chain(OTHER_ENDPOINTS.iter().map(|(_, p)| p.to_string()))
        .collect();
    // Keep the set stable even if a path is dropped from the live spec: the
    // comparison itself reports it.
    paths.retain(|p| spec["paths"].get(p).is_some() || true);
    paths
}

#[test]
fn every_response_the_client_reads_fits_its_model() {
    let spec = spec();
    let mut failures = Vec::new();
    let mut extras = Vec::new();

    for contract in contracts() {
        let schema = response_schema(&spec, contract.method, contract.path);
        let label = format!("{} {}", contract.method.to_uppercase(), contract.path);
        for everything in [false, true] {
            let shape = Shape {
                everything,
                prefer: contract.prefer,
            };
            let mut body = sample(&spec, schema, shape, 0);
            for (pointer, value) in contract.fixups {
                if let Some(slot) = body.pointer_mut(pointer) {
                    *slot = json!(value);
                }
            }
            match (contract.check)(&body, shape) {
                Ok(echo) => {
                    if !everything {
                        // Keys the model always writes that the spec does not
                        // define: harmless for reading, listed for review.
                        if let (Some(ours), Some(theirs)) = (
                            echo.as_object(),
                            sample(
                                &spec,
                                schema,
                                Shape {
                                    everything: true,
                                    prefer: contract.prefer,
                                },
                                0,
                            )
                            .as_object(),
                        ) {
                            for key in ours.keys() {
                                if !theirs.contains_key(key) {
                                    extras.push(format!(
                                        "{label}: model writes `{key}`, not in spec"
                                    ));
                                }
                            }
                        }
                    }
                }
                Err(e) => failures.push(format!(
                    "{label} ({}): {e}\n  body: {}",
                    if everything {
                        "all properties"
                    } else {
                        "required only"
                    },
                    serde_json::to_string(&body).unwrap()
                )),
            }
        }
    }

    for (method, path) in OTHER_ENDPOINTS {
        if !spec["paths"][*path][*method].is_object() {
            failures.push(format!(
                "{} {} is not in the specification",
                method.to_uppercase(),
                path
            ));
        }
    }

    if !extras.is_empty() {
        eprintln!("Model fields with no counterpart in the spec (informational):");
        for line in &extras {
            eprintln!("  {line}");
        }
    }
    assert!(
        failures.is_empty(),
        "{} contract failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn hard_coded_enumerations_match_the_spec() {
    let spec = spec();
    let values = |name: &str| -> Vec<String> {
        spec["components"]["schemas"][name]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} is not an enum"))
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };

    // The states the CLI reports on and filters by (asset health, tenant state
    // counts, `normalized_processing_status`). A new state here means a new
    // row in those reports.
    assert_eq!(
        values("AssetState"),
        [
            "indexing",
            "finished",
            "failed",
            "unsupported",
            "no-3d-data",
            "missing-dependencies"
        ]
    );
    // The types the metadata CSV declares and the registry converts.
    assert_eq!(
        values("MetadataFieldType"),
        pcli2::actions::assets::metadata_batch_csv::DECLARED_TYPES
    );
    // The role the 403 hint tells the user to ask for.
    assert!(values("TenantRole").iter().any(|r| r == "author"));
    // Dependency statuses: `resolve-dependency` and the tree builder branch on them.
    assert_eq!(
        values("DependencyStatus"),
        pcli2::model::DependencyStatus::ALL
    );

    // Failure diagnostics: the statuses `asset diagnose` branches on and the
    // kinds it prints, and the sources `tenant failures --kind` accepts. These
    // are inline enums on the response schemas rather than named ones.
    let property_values = |schema: &str, property: &str| -> Vec<String> {
        spec["components"]["schemas"][schema]["properties"][property]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{schema}.{property} is not an enum"))
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        property_values("FailureDiagnosticsResponse", "status"),
        ["found", "not-found", "unavailable"]
    );
    assert_eq!(
        property_values("FailureDiagnosticsResponse", "kind"),
        ["user", "internal"]
    );
    assert_eq!(
        property_values("RecentFailureItem", "kind"),
        pcli2::model::FailureSource::ALL
    );
    // Reports: the statuses `report create --wait` stops on and the types
    // `report list --type` accepts.
    assert_eq!(values("JobStatus"), pcli2::model::JobStatus::ALL);
    assert_eq!(values("ReportType"), pcli2::model::ReportType::ALL);
    let mut counts: Vec<String> = spec["components"]["schemas"]["ListRecentFailuresResponse"]
        ["properties"]["countsByKind"]["required"]
        .as_array()
        .expect("countsByKind lists its required kinds")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    counts.sort();
    let mut ours = pcli2::model::FailureSource::ALL.to_vec();
    ours.sort();
    assert_eq!(
        counts, ours,
        "countsByKind has a kind the client does not read"
    );
    let mut most_recent: Vec<String> = spec["components"]["schemas"]["ListRecentFailuresResponse"]
        ["properties"]["mostRecentByKind"]["properties"]
        .as_object()
        .expect("mostRecentByKind names its kinds")
        .keys()
        .cloned()
        .collect();
    most_recent.sort();
    assert_eq!(
        most_recent, ours,
        "mostRecentByKind has a kind the client does not read"
    );
}

#[test]
fn page_sizes_the_client_uses_are_within_the_spec_maximum() {
    let spec = spec();
    // perPage=1000 on listings (the API maximum), 500 on searches.
    for (path, ours) in [
        ("/tenants/{tenantId}/folders", 1000),
        ("/tenants/{tenantId}/folders/{folderId}/contents", 1000),
        ("/tenants/{tenantId}/assets", 1000),
        ("/tenants/{tenantId}/metadata-fields", 1000),
        (
            "/tenants/{tenantId}/assets/{assetId}/dependencies-by-id",
            1000,
        ),
        ("/tenants/{tenantId}/users", 100),
        ("/tenants/{tenantId}/failures", 100),
        ("/tenants/{tenantId}/metadata-fields/{fieldId}/assets", 1000),
        ("/tenants/{tenantId}/assets/without-metadata", 1000),
        ("/tenants/{tenantId}/reports", 1000),
    ] {
        let parameters = spec["paths"][path]["get"]["parameters"]
            .as_array()
            .unwrap_or_else(|| panic!("{path} has no parameters"));
        let per_page = parameters
            .iter()
            .find(|p| p["name"] == "perPage")
            .unwrap_or_else(|| panic!("{path} has no perPage parameter"));
        if let Some(maximum) = per_page["schema"]["maximum"].as_f64() {
            assert!(
                ours as f64 <= maximum,
                "{path}: client sends perPage={ours}, spec maximum is {maximum}"
            );
        }
    }
}

// ---- drift -----------------------------------------------------------------

/// Fetch the swagger-ui bootstrap script.
///
/// Physna's CDN intermittently answers with HTTP 200 and an empty body, for
/// any user agent, sometimes for minutes at a time (on 2026-10-05 it lasted
/// longer than the old two-and-a-half-minute window). A browser user agent, a
/// cache-busting query on every attempt, and retries spread over ten minutes
/// get the real script. Network errors are retried the same way.
///
/// Giving up is reported as an outage, under its own annotation title, so it
/// is not mistaken for the API having changed.
async fn fetch_live_script() -> String {
    const PATIENCE: std::time::Duration = std::time::Duration::from_secs(10 * 60);
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh) pcli2-spec-drift")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("http client");
    let started = std::time::Instant::now();
    let mut attempt = 0u64;
    let last = loop {
        attempt += 1;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let body = async {
            client
                .get(format!("{LIVE_URL}?nocache={nonce}"))
                .header(reqwest::header::CACHE_CONTROL, "no-cache")
                .send()
                .await?
                .text()
                .await
        }
        .await;
        let last = match body {
            Ok(text) if text.contains("\"swaggerDoc\"") => return text,
            Ok(text) => format!("no swaggerDoc ({} bytes)", text.len()),
            Err(e) => format!("request failed: {e}"),
        };
        eprintln!("attempt {attempt}: swagger-ui-init.js: {last}");
        let delay = std::time::Duration::from_secs((5 * attempt).min(60));
        if started.elapsed() + delay > PATIENCE {
            break last;
        }
        tokio::time::sleep(delay).await;
    };
    println!(
        "::error title=Physna spec unavailable (not API drift)::{LIVE_URL} did not return the specification in {attempt} attempts over {} s; last result: {last}. Re-run the workflow later.",
        started.elapsed().as_secs()
    );
    panic!(
        "could not fetch the live specification after {attempt} attempts ({last}). \
         This is an outage of Physna's docs endpoint, not a change to the API: re-run the workflow later."
    );
}

/// Extract the `swaggerDoc` object from the swagger-ui bootstrap script.
fn extract_swagger_doc(js: &str) -> Value {
    serde_json::from_str(swagger_doc_text(js)).expect("swaggerDoc is JSON")
}

/// The `swaggerDoc` object's source text, in Physna's own key order.
fn swagger_doc_text(js: &str) -> &str {
    let start = js.find("\"swaggerDoc\"").expect("swaggerDoc in script");
    let open = start + js[start..].find('{').expect("object after swaggerDoc");
    let bytes = js.as_bytes();
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (offset, &b) in bytes[open..].iter().enumerate() {
        match (in_string, b) {
            (true, b'\\') if !escaped => escaped = true,
            (true, b'"') if !escaped => in_string = false,
            (true, _) => escaped = false,
            (false, b'"') => in_string = true,
            (false, b'{') => depth += 1,
            (false, b'}') => {
                depth -= 1;
                if depth == 0 {
                    return &js[open..=open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated swaggerDoc object");
}

/// Every schema reachable from a value, by `$ref` name.
fn referenced_schemas(spec: &Value, value: &Value, seen: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                let name = reference.rsplit('/').next().unwrap().to_string();
                if seen.insert(name.clone()) {
                    referenced_schemas(spec, &spec["components"]["schemas"][&name], seen);
                }
            }
            for child in map.values() {
                referenced_schemas(spec, child, seen);
            }
        }
        Value::Array(items) => {
            for item in items {
                referenced_schemas(spec, item, seen);
            }
        }
        _ => {}
    }
}

#[tokio::test]
#[ignore = "fetches the live specification; run by the spec-drift workflow"]
async fn live_spec_matches_the_snapshot() {
    let snapshot = spec();
    let script = fetch_live_script().await;
    let live = extract_swagger_doc(&script);
    let live_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/physna-openapi.live.json"
    );
    // The source text, not a re-serialization: serde_json would sort the keys
    // and the refreshed fixture would differ from the old one on every line.
    std::fs::write(live_path, swagger_doc_text(&script)).unwrap();

    let mut differences = Vec::new();
    if snapshot["info"]["version"] != live["info"]["version"] {
        differences.push(format!(
            "info.version: snapshot {} -> live {}",
            snapshot["info"]["version"], live["info"]["version"]
        ));
    }

    // Operations we call, compared whole (parameters, request body, responses).
    for path in used_paths(&snapshot) {
        for method in ["get", "post", "put", "patch", "delete"] {
            let (ours, theirs) = (
                &snapshot["paths"][&path][method],
                &live["paths"][&path][method],
            );
            if ours.is_null() && theirs.is_null() {
                continue;
            }
            if ours != theirs {
                differences.push(format!("{} {} changed", method.to_uppercase(), path));
            }
        }
    }

    // Every schema those operations reach, compared whole.
    let mut names = BTreeSet::new();
    for path in used_paths(&snapshot) {
        referenced_schemas(&snapshot, &snapshot["paths"][&path], &mut names);
    }
    for name in names {
        let (ours, theirs) = (
            &snapshot["components"]["schemas"][&name],
            &live["components"]["schemas"][&name],
        );
        if ours != theirs {
            let describe = |v: &Value| -> String {
                let props: Vec<String> = v["properties"]
                    .as_object()
                    .map(|m| {
                        let required: BTreeSet<&str> = v["required"]
                            .as_array()
                            .map(|r| r.iter().filter_map(Value::as_str).collect())
                            .unwrap_or_default();
                        m.keys()
                            .map(|k| {
                                if required.contains(k.as_str()) {
                                    format!("{k}*")
                                } else {
                                    k.clone()
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if props.is_empty() {
                    serde_json::to_string(v).unwrap()
                } else {
                    props.join(", ")
                }
            };
            differences.push(format!(
                "schema {name} changed\n    snapshot: {}\n    live:     {}",
                describe(ours),
                describe(theirs)
            ));
        }
    }

    if !differences.is_empty() {
        println!(
            "::error title=Physna API drift::the live specification differs from the snapshot in {} place(s); see the job log",
            differences.len()
        );
    }
    assert!(
        differences.is_empty(),
        "the live specification differs from the snapshot in {} place(s):\n  {}\n\nThe live spec was written to {live_path}. Review the changes, then refresh the fixture in its one-line format with\n\n  {REFRESH}\n\nand re-run `cargo test --test openapi_contract_test`.",
        differences.len(),
        differences.join("\n  ")
    );
}

#[test]
fn the_swagger_doc_extractor_handles_braces_inside_strings() {
    let js = r#"let options = { "swaggerDoc": {"info": {"title": "x { y } \" z"}, "paths": {}}, "customOptions": {} };"#;
    let doc = extract_swagger_doc(js);
    assert_eq!(doc["info"]["title"], "x { y } \" z");
    assert!(doc["paths"].is_object());
}

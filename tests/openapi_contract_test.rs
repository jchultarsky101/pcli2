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
//! noticed before a user meets it. When it fails, review the diff it prints,
//! copy `target/physna-openapi.live.json` over the fixture, and re-run this
//! file to see whether the model still fits.

use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/physna-openapi.json"
);
const LIVE_URL: &str = "https://app-api.physna.com/v3/docs/swagger-ui-init.js";

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
            "/tenants/{tenantId}/assets/{assetPath}/dependencies",
            AssetDependenciesResponse
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
    ("delete", "/tenants/{tenantId}/assets/{assetId}"),
    ("patch", "/tenants/{tenantId}/assets/{assetId}"),
    ("delete", "/tenants/{tenantId}/assets/{assetId}/metadata"),
    ("get", "/tenants/{tenantId}/assets/{assetId}/file"),
    ("get", "/tenants/{tenantId}/assets/{assetId}/thumbnail.png"),
    ("post", "/tenants/{tenantId}/assets/reprocess"),
    ("post", "/tenants/{tenantId}/metadata-fields"),
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
    // Dependency statuses the diff and print commands know about.
    assert_eq!(
        values("DependencyStatus"),
        ["matched", "resolved", "missing"]
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
        ("/tenants/{tenantId}/assets/{assetPath}/dependencies", 1000),
        ("/tenants/{tenantId}/users", 100),
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

/// Extract the `swaggerDoc` object from the swagger-ui bootstrap script.
fn extract_swagger_doc(js: &str) -> Value {
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
                    return serde_json::from_str(&js[open..=open + offset])
                        .expect("swaggerDoc is JSON");
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
    let js = reqwest::get(LIVE_URL)
        .await
        .expect("fetch swagger-ui-init.js")
        .text()
        .await
        .expect("read swagger-ui-init.js");
    let live = extract_swagger_doc(&js);
    let live_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/physna-openapi.live.json"
    );
    std::fs::write(live_path, serde_json::to_string_pretty(&live).unwrap()).unwrap();

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

    assert!(
        differences.is_empty(),
        "the live specification differs from the snapshot in {} place(s):\n  {}\n\nThe live spec was written to {live_path}. Review the changes, copy it over tests/fixtures/physna-openapi.json, and re-run `cargo test --test openapi_contract_test`.",
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

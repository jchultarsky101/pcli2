//! The dependency tree against a mock API, on the ID-based endpoint.
//!
//! Dependencies are read from `GET .../assets/{assetId}/dependencies-by-id`
//! (the path-based endpoint is deprecated). Two things must hold: a missing
//! dependency is never expanded (it has no asset, so the only UUID to ask
//! about would be the nil one), and a present dependency that has its own
//! dependencies is expanded through the same endpoint.

use mockito::Matcher;
use pcli2::model::{AssetDependencyApiResponse, DependencyStatus};
use pcli2::physna_v3::PhysnaApiClient;
use uuid::Uuid;

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ROOT: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const CHILD: &str = "aaaaaaaa-0000-0000-0000-000000000002";
const GRANDCHILD: &str = "aaaaaaaa-0000-0000-0000-000000000003";

fn asset(id: &str, path: &str, is_assembly: bool) -> String {
    format!(
        r#"{{"id":"{id}","tenantId":"{TENANT}","path":"{path}","type":"model",
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z",
            "state":"finished","isAssembly":{is_assembly},"metadata":{{}}}}"#
    )
}

fn page(dependencies: &[String]) -> String {
    format!(
        r#"{{"dependencies":[{}],"pageData":{{"total":{n},"perPage":1000,"currentPage":1,"lastPage":1,"startIndex":0,"endIndex":{n}}}}}"#,
        dependencies.join(","),
        n = dependencies.len()
    )
}

#[tokio::test]
async fn a_missing_dependency_is_listed_but_never_expanded() {
    let mut server = mockito::Server::new_async().await;

    let _root_asset = server
        .mock("GET", format!("/tenants/{TENANT}/assets/{ROOT}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"asset":{}}}"#,
            asset(ROOT, "Parts/top.asm", true)
        ))
        .create_async()
        .await;

    // The root has one missing child that (oddly) claims to have dependencies
    // of its own, and one real child that is a leaf.
    let root_deps = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/{ROOT}/dependencies-by-id").as_str(),
        )
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page(&[
            r#"{"path":"Parts/ghost.par","occurrences":1,"hasDependencies":true,"status":"missing"}"#.to_string(),
            format!(
                r#"{{"path":"Parts/leaf.par","asset":{},"occurrences":2,"hasDependencies":false,"status":"matched"}}"#,
                asset(CHILD, "Parts/leaf.par", false)
            ),
        ]))
        .expect(1)
        .create_async()
        .await;

    // Nothing may ever ask about the nil UUID, on either endpoint.
    let nil_by_id = server
        .mock(
            "GET",
            Matcher::Regex(format!(
                "/tenants/{TENANT}/assets/{}/dependencies",
                Uuid::nil()
            )),
        )
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let tree = client
        .get_asset_dependencies_by_uuid(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ROOT).unwrap(),
        )
        .await
        .unwrap();

    let children: Vec<_> = tree.root().children().collect();
    assert_eq!(children.len(), 2);
    let ghost = children[0];
    assert_eq!(ghost.asset().uuid(), Uuid::nil());
    assert_eq!(
        ghost.asset().processing_status().map(String::as_str),
        Some("missing")
    );
    assert_eq!(ghost.children().count(), 0);
    assert_eq!(children[1].asset().uuid(), Uuid::parse_str(CHILD).unwrap());
    // The API says the leaf is used twice; that used to be reported as 1.
    assert_eq!(children[1].occurrences(), 2);
    assert_eq!(ghost.occurrences(), 1);

    root_deps.assert_async().await;
    nil_by_id.assert_async().await;
}

#[tokio::test]
async fn an_assembly_that_contains_itself_does_not_recurse_forever() {
    let mut server = mockito::Server::new_async().await;
    let _root_asset = server
        .mock("GET", format!("/tenants/{TENANT}/assets/{ROOT}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"asset":{}}}"#,
            asset(ROOT, "Parts/top.asm", true)
        ))
        .create_async()
        .await;
    // top.asm -> sub.asm -> top.asm again.
    let _root_deps = server
        .mock("GET", format!("/tenants/{TENANT}/assets/{ROOT}/dependencies-by-id").as_str())
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page(&[format!(
            r#"{{"path":"Parts/sub.asm","asset":{},"occurrences":1,"hasDependencies":true,"status":"matched"}}"#,
            asset(CHILD, "Parts/sub.asm", true)
        )]))
        .create_async()
        .await;
    let _child_deps = server
        .mock("GET", format!("/tenants/{TENANT}/assets/{CHILD}/dependencies-by-id").as_str())
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page(&[format!(
            r#"{{"path":"Parts/top.asm","asset":{},"occurrences":1,"hasDependencies":true,"status":"matched"}}"#,
            asset(ROOT, "Parts/top.asm", true)
        )]))
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let tree = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get_asset_dependencies_by_uuid(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ROOT).unwrap(),
        ),
    )
    .await
    .expect("a cycle must not recurse forever")
    .unwrap();
    let sub = tree.root().children().next().unwrap();
    let back_to_top = sub.children().next().unwrap();
    assert_eq!(back_to_top.children().count(), 0, "the cycle is cut here");
}

#[tokio::test]
async fn a_present_dependency_with_its_own_dependencies_is_expanded() {
    let mut server = mockito::Server::new_async().await;

    let _root_asset = server
        .mock("GET", format!("/tenants/{TENANT}/assets/{ROOT}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{"asset":{}}}"#,
            asset(ROOT, "Parts/top.asm", true)
        ))
        .create_async()
        .await;
    let _root_deps = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/{ROOT}/dependencies-by-id").as_str(),
        )
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page(&[format!(
            r#"{{"path":"Parts/sub.asm","asset":{},"occurrences":1,"hasDependencies":true,"status":"resolved"}}"#,
            asset(CHILD, "Parts/sub.asm", true)
        )]))
        .create_async()
        .await;
    let child_deps = server
        .mock(
            "GET",
            format!("/tenants/{TENANT}/assets/{CHILD}/dependencies-by-id").as_str(),
        )
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(page(&[format!(
            r#"{{"path":"Parts/bolt.par","asset":{},"occurrences":4,"hasDependencies":false,"status":"matched"}}"#,
            asset(GRANDCHILD, "Parts/bolt.par", false)
        )]))
        .expect(1)
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());
    let tree = client
        .get_asset_dependencies_by_uuid(
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(ROOT).unwrap(),
        )
        .await
        .unwrap();

    let sub = tree.root().children().next().expect("one child");
    assert_eq!(sub.children().count(), 1);
    assert_eq!(
        sub.children().next().unwrap().asset().uuid(),
        Uuid::parse_str(GRANDCHILD).unwrap()
    );
    child_deps.assert_async().await;
}

#[test]
fn a_dependency_without_a_status_is_missing_only_when_it_has_no_asset() {
    let with_status: AssetDependencyApiResponse = serde_json::from_str(
        r#"{"path":"a.par","occurrences":1,"hasDependencies":false,"status":"missing"}"#,
    )
    .unwrap();
    assert_eq!(with_status.status, Some(DependencyStatus::Missing));
    assert!(with_status.is_missing());

    let older: AssetDependencyApiResponse =
        serde_json::from_str(r#"{"path":"a.par","occurrences":1,"hasDependencies":false}"#)
            .unwrap();
    assert_eq!(older.status, None);
    assert!(older.is_missing());

    let present: AssetDependencyApiResponse = serde_json::from_str(&format!(
        r#"{{"path":"a.par","asset":{},"occurrences":1,"hasDependencies":false,"status":"matched"}}"#,
        asset(CHILD, "a.par", false)
    ))
    .unwrap();
    assert!(!present.is_missing());
}

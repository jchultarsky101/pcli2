//! Tests for strict folder path resolution.
//!
//! `resolve_folder_uuid_by_path` must return `None` ONLY for the root path
//! "/". A non-root path that doesn't resolve to an existing folder must be an
//! error: it previously fell through as `None`, which downstream code treated
//! as "the root folder", so commands like `asset delete --path
//! /Typo/part.stl` could resolve — and delete — a same-named asset at the
//! root instead of failing.

use mockito::Matcher;
use pcli2::physna_v3::{ApiError, PhysnaApiClient};
use uuid::Uuid;

fn folders_body(tenant: &Uuid) -> String {
    format!(
        r#"{{
            "folders": [
                {{
                    "id": "11111111-1111-1111-1111-111111111111",
                    "tenantId": "{}",
                    "name": "Existing",
                    "createdAt": "2026-01-01T00:00:00Z",
                    "updatedAt": "2026-01-01T00:00:00Z",
                    "assetsCount": 0,
                    "foldersCount": 0
                }}
            ],
            "pageData": {{"total":1,"perPage":200,"currentPage":1,"lastPage":1,"startIndex":0,"endIndex":0}}
        }}"#,
        tenant
    )
}

#[tokio::test]
async fn nonexistent_folder_path_is_an_error_not_root() {
    // Isolate the folder cache to a temp dir for this test process.
    let cache_dir = std::env::temp_dir().join(format!("pcli2-test-cache-{}", std::process::id()));
    std::env::set_var("PCLI2_CACHE_DIR", &cache_dir);

    let mut server = mockito::Server::new_async().await;
    let tenant = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

    let _folders = server
        .mock("GET", format!("/tenants/{}/folders", tenant).as_str())
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(folders_body(&tenant))
        .create_async()
        .await;

    let mut client = PhysnaApiClient::new().with_base_url(server.url());

    // Root resolves to None (the root has no UUID).
    let root = client.resolve_folder_uuid_by_path(&tenant, "/").await;
    assert!(matches!(root, Ok(None)));

    // An existing folder resolves to its UUID.
    let existing = client
        .resolve_folder_uuid_by_path(&tenant, "/Existing")
        .await
        .unwrap();
    assert_eq!(
        existing,
        Some(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
    );

    // A nonexistent folder is an ERROR — never silently the root.
    let missing = client
        .resolve_folder_uuid_by_path(&tenant, "/DoesNotExist")
        .await;
    match missing {
        Err(ApiError::FolderNotFound(path)) => assert_eq!(path, "/DoesNotExist"),
        other => panic!("expected FolderNotFound error, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&cache_dir);
}

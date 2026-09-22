//! Run the real `pcli2` binary against a mock Physna API.
//!
//! Most tests call the client library directly, which leaves the command layer
//! (argument handling, folder walks, output files, exit codes) untested. This
//! harness writes a throwaway configuration that points an environment at a
//! mockito server, stores a long-lived token for it, and answers the tenant
//! lookup, so a test can run `pcli2 <command>` exactly as a user would.

#![allow(dead_code)]

use assert_cmd::prelude::*;
use base64::Engine;
use mockito::{Matcher, Mock, Server, ServerGuard};
use std::process::{Command, Stdio};
use uuid::Uuid;

pub const TENANT: &str = "22222222-2222-2222-2222-222222222222";

pub struct MockCli {
    pub server: ServerGuard,
    pub dir: tempfile::TempDir,
    pub tenant: Uuid,
    _user: Mock,
}

/// A JWT the client accepts without renewing: unsigned, expiring in 2100.
fn long_lived_token() -> String {
    let encode = |json: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
    format!(
        "{}.{}.signature",
        encode(r#"{"alg":"none","typ":"JWT"}"#),
        encode(r#"{"exp":4102444800}"#)
    )
}

impl MockCli {
    pub fn new() -> Self {
        let mut server = Server::new();
        let tenant = Uuid::parse_str(TENANT).unwrap();
        let dir = tempfile::tempdir().unwrap();

        let config = format!(
            "active_environment: mock\nenvironments:\n  mock:\n    api_base_url: {url}\n    ui_base_url: https://app.example.test\n    auth_base_url: {url}/token\n    active_tenant_uuid: {tenant}\n",
            url = server.url(),
            tenant = tenant
        );
        std::fs::write(dir.path().join("config.yml"), config).unwrap();
        let credentials = serde_json::json!({
            "environments": {
                "mock": {"client_id": "", "client_secret": "", "access_token": long_lived_token()}
            }
        });
        std::fs::write(
            dir.path().join("dev_credentials.json"),
            credentials.to_string(),
        )
        .unwrap();

        let user = server
            .mock("GET", "/users/me")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "user": {
                        "displayName": "Test User",
                        "settings": [{
                            "tenantId": TENANT,
                            "tenantRole": "author",
                            "userEnabled": true,
                            "tenantDisplayName": "Mock Tenant",
                            "tenantShortName": "mock"
                        }]
                    }
                })
                .to_string(),
            )
            .create();

        MockCli {
            server,
            dir,
            tenant,
            _user: user,
        }
    }

    /// `pcli2` with the configuration, caches and a scratch working directory all
    /// inside this test's temporary directory.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.env("PCLI2_CONFIG_DIR", self.dir.path())
            .env("PCLI2_CACHE_DIR", self.dir.path().join("cache"))
            .env("PCLI2_NO_UPDATE_CHECK", "1")
            .env("PCLI2_MAX_RETRIES", "0")
            .env_remove("PCLI2_FORMAT")
            .env_remove("PCLI2_ERROR_FORMAT")
            .env_remove("RUST_LOG")
            .current_dir(self.dir.path())
            .stdin(Stdio::null());
        cmd
    }

    /// The tenant's whole folder listing, which path resolution reads.
    /// `folders` is `(uuid, name, parent)`.
    pub fn folders(&mut self, folders: &[(Uuid, &str, Option<Uuid>)]) -> Mock {
        let items: Vec<_> = folders
            .iter()
            .map(|(id, name, parent)| folder_json(*id, name, *parent))
            .collect();
        self.server
            .mock("GET", format!("/tenants/{}/folders", self.tenant).as_str())
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({"folders": items, "pageData": page_data(items.len())})
                    .to_string(),
            )
            .create()
    }

    /// The assets directly inside a folder (`None` is the root).
    pub fn assets_in(&mut self, folder: Option<Uuid>, assets: &[serde_json::Value]) -> Mock {
        self.server
            .mock("GET", contents_path(self.tenant, folder).as_str())
            .match_query(Matcher::UrlEncoded("contentType".into(), "assets".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({"assets": assets, "pageData": page_data(assets.len())})
                    .to_string(),
            )
            .create()
    }

    /// The folders directly inside a folder (`None` is the root).
    pub fn subfolders_of(&mut self, folder: Option<Uuid>, children: &[(Uuid, &str)]) -> Mock {
        let items: Vec<_> = children
            .iter()
            .map(|(id, name)| folder_json(*id, name, folder))
            .collect();
        self.server
            .mock("GET", contents_path(self.tenant, folder).as_str())
            .match_query(Matcher::UrlEncoded("contentType".into(), "folders".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({"folders": items, "pageData": page_data(items.len())})
                    .to_string(),
            )
            .create()
    }

    /// The file body served for an asset.
    pub fn file(&mut self, asset: Uuid, body: &[u8]) -> Mock {
        self.server
            .mock(
                "GET",
                format!("/tenants/{}/assets/{}/file", self.tenant, asset).as_str(),
            )
            .with_status(200)
            .with_body(body)
            .create()
    }
}

fn contents_path(tenant: Uuid, folder: Option<Uuid>) -> String {
    match folder {
        Some(folder) => format!("/tenants/{}/folders/{}/contents", tenant, folder),
        None => format!("/tenants/{}/folders/root/contents", tenant),
    }
}

fn page_data(total: usize) -> serde_json::Value {
    serde_json::json!({"total": total, "perPage": 1000, "currentPage": 1, "lastPage": 1, "startIndex": 0, "endIndex": total})
}

pub fn folder_json(id: Uuid, name: &str, parent: Option<Uuid>) -> serde_json::Value {
    let mut folder = serde_json::json!({
        "id": id,
        "tenantId": TENANT,
        "name": name,
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-01T00:00:00Z",
        "assetsCount": 0,
        "foldersCount": 0
    });
    if let Some(parent) = parent {
        folder["parentFolderId"] = serde_json::json!(parent);
    }
    folder
}

/// An asset as the listing returns it.
pub fn asset_json(id: Uuid, path: &str, state: &str, is_assembly: bool) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "tenantId": TENANT,
        "path": path,
        "type": "file",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-01T00:00:00Z",
        "state": state,
        "isAssembly": is_assembly,
        "metadata": {}
    })
}

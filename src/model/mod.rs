//! Data models for the Physna CLI client.
//!
//! This module contains all the data structures used throughout the application,
//! including models for folders, assets, tenants, and API responses.
//!
//! The models follow a layered approach:
//! - API response models (e.g., AssetResponse, FolderResponse) - direct mapping from API JSON
//! - Internal models (e.g., Asset, Folder) - business logic models with additional functionality
//! - Collection models (e.g., AssetList, FolderList) - collections of individual models
//!
//! Formatting trait implementations (OutputFormatter, CsvRecordProducer, Formattable) are
//! located in the `crate::format::impls` module to separate formatting logic from data definitions.
//!
//! All models implement serialization/deserialization with serde and appropriate error handling.

use crate::format::{CsvRecordProducer, FormattingError, OutputFormat, OutputFormatter};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Error types that can occur when working with models
#[derive(Debug, Error)]
pub enum ModelError {
    /// Error when a required property value is missing
    #[error("missing property value {name:?}")]
    MissingPropertyValue { name: String },

    /// Error when serializing to JSON
    #[error("serialization error: {source}")]
    SerializationError {
        #[from]
        source: serde_json::Error,
    },

    /// Error when working with CSV
    #[error("CSV error: {msg}")]
    CsvError { msg: String },

    /// Error when working with IO
    #[error("IO error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}

/// Normalize a path with these rules:
/// 1) Remove leading "/HOME" if present
/// 2) Remove any trailing '/'
/// 3) Collapse multiple consecutive '/' into a single '/'
/// 4) Ensure the result starts with exactly one '/'
///
/// Examples:
///   "/myroot/mysub/more/"         -> "/myroot/mysub/more"
///   "myroot/mysub/more"           -> "/myroot/mysub/more"
///   "/HOME/myroot/mysub/more/"    -> "/myroot/mysub/more"
///   "/HOME"                       -> "/"
///   "////"                        -> "/"
///   "/myroot//mysub///more/"      -> "/myroot/mysub/more"
pub fn normalize_path(path: impl AsRef<str>) -> String {
    let mut s = path.as_ref().trim();

    // Case-insensitive check for prefix "/HOME/"
    if s.to_ascii_lowercase().starts_with("/home/") {
        // SAFETY: only slice the original string, not the lowercase temp
        s = &s[5..]; // remove `/HOME` (5 chars)
    } else if s.eq_ignore_ascii_case("/home") {
        return "/".into();
    }

    // Remove trailing '/'
    s = s.trim_end_matches('/');

    // Split by '/' and filter out empty parts to collapse multiple consecutive slashes
    let parts: Vec<&str> = s.split('/').filter(|part| !part.is_empty()).collect();
    let result = parts.join("/");

    // Handle the case where the original path was just slashes (e.g. "/", "//", "///")
    let without_leading = if !result.is_empty() {
        result.as_str()
    } else {
        ""
    };

    // Ensure exactly one leading '/'
    let mut out = String::with_capacity(without_leading.len() + 1);
    out.push('/');
    out.push_str(without_leading);

    out
}

/// True when `asset_path` refers to the folder itself or anything inside it.
///
/// Uses a path-component boundary rather than a bare string prefix:
/// `/proj-archive/part.stl` is NOT within `/proj`, even though the string
/// starts with it.
pub fn path_is_within_folder(asset_path: impl AsRef<str>, folder_path: impl AsRef<str>) -> bool {
    let folder = normalize_path(folder_path);
    let asset = normalize_path(asset_path);
    if folder == "/" {
        return true;
    }
    asset == folder || asset.starts_with(&format!("{}/", folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_within_folder_respects_component_boundaries() {
        // Regression: `--exclusive` match filtering used a bare string prefix,
        // so a `/proj` filter leaked matches from sibling `/proj-archive`.
        assert!(path_is_within_folder("/proj/part.stl", "/proj"));
        assert!(path_is_within_folder("/proj/sub/part.stl", "/proj"));
        assert!(path_is_within_folder("/proj", "/proj"));
        assert!(!path_is_within_folder("/proj-archive/part.stl", "/proj"));
        assert!(!path_is_within_folder("/proj2/part.stl", "/proj"));
        // Root contains everything.
        assert!(path_is_within_folder("/anything/part.stl", "/"));
        // Paths without leading slashes normalize consistently.
        assert!(path_is_within_folder("proj/part.stl", "/proj/"));
    }

    #[test]
    fn test_normalize_path_basic_cases() {
        assert_eq!(normalize_path("/myroot/mysub/more/"), "/myroot/mysub/more");
        assert_eq!(normalize_path("myroot/mysub/more"), "/myroot/mysub/more");
        assert_eq!(
            normalize_path("/HOME/myroot/mysub/more/"),
            "/myroot/mysub/more"
        );
        assert_eq!(normalize_path("/HOME"), "/");
        assert_eq!(normalize_path("////"), "/");
    }

    #[test]
    fn test_normalize_path_consecutive_slashes() {
        assert_eq!(
            normalize_path("/myroot//mysub///more/"),
            "/myroot/mysub/more"
        );
        assert_eq!(normalize_path("Root//Folder"), "/Root/Folder");
        assert_eq!(
            normalize_path("//double//slash//test"),
            "/double/slash/test"
        );
        assert_eq!(normalize_path("///"), "/");
        assert_eq!(normalize_path(""), "/");
    }

    #[test]
    fn test_normalize_path_home_handling() {
        assert_eq!(normalize_path("/HOME"), "/");
        assert_eq!(normalize_path("/home"), "/");
        assert_eq!(normalize_path("/HOME/"), "/");
        assert_eq!(normalize_path("/home/"), "/");
        assert_eq!(normalize_path("/HOME/test"), "/test");
        assert_eq!(normalize_path("/home/test"), "/test");
        assert_eq!(normalize_path("/HOME/test/"), "/test");
        assert_eq!(normalize_path("/home/test/"), "/test");

        // Ensure case insensitivity
        assert_eq!(normalize_path("/HoMe"), "/");
        assert_eq!(normalize_path("/hOmE/test"), "/test");
    }

    #[test]
    fn test_normalize_path_edge_cases() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("   "), "/");
        assert_eq!(normalize_path("   /   "), "/");
        assert_eq!(normalize_path("   /test/   "), "/test");
        assert_eq!(normalize_path("test"), "/test");
        assert_eq!(normalize_path("test/"), "/test");
        assert_eq!(normalize_path("/test"), "/test");
        assert_eq!(normalize_path("/////test"), "/test");
        assert_eq!(normalize_path("test/////"), "/test");
    }

    #[test]
    fn test_normalize_path_trailing_slashes() {
        assert_eq!(normalize_path("/test/"), "/test");
        assert_eq!(normalize_path("/test//"), "/test");
        assert_eq!(normalize_path("/test///"), "/test");
        assert_eq!(normalize_path("test/"), "/test");
        assert_eq!(normalize_path("test//"), "/test");
        assert_eq!(normalize_path("test///"), "/test");
    }

    #[test]
    fn test_normalize_path_leading_slashes() {
        assert_eq!(normalize_path("//test"), "/test");
        assert_eq!(normalize_path("///test"), "/test");
        assert_eq!(normalize_path("////test"), "/test");
        assert_eq!(normalize_path("/////test"), "/test");
    }
}

mod assets;
mod dependencies;
mod folders;
mod reports;
mod search;
mod text_search;

pub use assets::*;
pub use dependencies::*;
pub use folders::*;
pub use reports::*;
pub use search::*;
pub use text_search::*;

#[cfg(test)]
mod unknown_enum_value_tests {
    use super::*;

    #[test]
    fn a_value_added_by_the_api_reads_as_unknown_instead_of_failing() {
        assert_eq!(
            serde_json::from_str::<JobStatus>(r#""QUEUED""#).unwrap(),
            JobStatus::Unknown
        );
        assert_eq!(
            serde_json::from_str::<ReportType>(r#""MOLD""#).unwrap(),
            ReportType::Unknown
        );
        assert_eq!(
            serde_json::from_str::<DependencyStatus>(r#""ignored""#).unwrap(),
            DependencyStatus::Unknown
        );
        assert_eq!(
            serde_json::from_str::<FailureSource>(r#""viewer""#).unwrap(),
            FailureSource::Unknown
        );
        assert_eq!(
            serde_json::from_str::<FailureKind>(r#""partner""#).unwrap(),
            FailureKind::Unknown
        );
        assert_eq!(
            serde_json::from_str::<FailureDiagnosticsStatus>(r#""pending""#).unwrap(),
            FailureDiagnosticsStatus::Unknown
        );
        // Known values are unaffected.
        assert_eq!(
            serde_json::from_str::<JobStatus>(r#""RUNNING""#).unwrap(),
            JobStatus::Running
        );
        assert!(
            !JobStatus::Unknown.is_terminal(),
            "keep polling an unknown status"
        );
    }
}

#[cfg(test)]
mod deterministic_output_tests {
    use super::*;

    #[test]
    fn metadata_is_written_in_key_order() {
        let meta: HashMap<String, serde_json::Value> =
            ["Weight", "Material", "b", "A", "Finish", "material"]
                .iter()
                .map(|k| (k.to_string(), serde_json::json!(k)))
                .collect();
        let asset = AssetResponse {
            uuid: Uuid::nil(),
            tenant_id: Uuid::nil(),
            path: "/a.stl".into(),
            folder_id: None,
            asset_type: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            state: String::new(),
            is_assembly: false,
            metadata: meta,
            parent_folder_id: None,
            owner_id: None,
        };
        let json = serde_json::to_string(&asset).unwrap();
        let keys: Vec<&str> = [
            "\"A\"",
            "\"Finish\"",
            "\"Material\"",
            "\"Weight\"",
            "\"b\"",
            "\"material\"",
        ]
        .into_iter()
        .collect();
        let positions: Vec<usize> = keys.iter().map(|k| json.find(k).unwrap()).collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{json}");
    }
}

//! CSV parsing for the "asset metadata create-batch" command.
//!
//! Two input layouts are supported:
//!
//! - **Classic (vertical)**: `ASSET_PATH,NAME,VALUE` — one row per asset+field
//!   combination.
//! - **UI (horizontal)**: one row per asset with a `path` column, an optional
//!   `id` column (asset UUID, takes precedence over the path when present),
//!   and one `metadata:<field name>` column per metadata field, as exported by
//!   the Physna web UI's bulk metadata upload.
//!
//! In both layouts an empty value is skipped by default (the existing field
//! value, if any, is left untouched). With `--delete-if-empty` an empty value
//! instead means "delete this metadata field from the asset".
//!
//! The layout is auto-detected from the header row: if any column name starts
//! with the `metadata:` prefix the file is treated as UI format, otherwise as
//! classic. Detection can be overridden with the `--csv-format` argument.

use crate::actions::CliActionError;
use crate::model::normalize_path;
use std::collections::HashMap;
use std::io::Read;

/// Normalize an asset path from a batch CSV row to the form the API lookup
/// expects.
///
/// Applies the shared [`normalize_path`] rules — which, per Physna convention,
/// treat a leading `/Home` (case-insensitive) as the root and collapse
/// redundant slashes — then drops the single leading slash to match how batch
/// asset paths are resolved. So `/Home/NX/1.prt`, `/NX/1.prt`, and `NX/1.prt`
/// all resolve to the same asset (`NX/1.prt`).
fn normalize_batch_asset_path(raw: &str) -> String {
    let normalized = normalize_path(raw);
    normalized
        .strip_prefix('/')
        .unwrap_or(&normalized)
        .to_string()
}

/// Column-name prefix that marks a metadata column in the UI format.
pub const METADATA_COLUMN_PREFIX: &str = "metadata:";

/// Field types accepted in the classic layout's optional `TYPE` column. The
/// registered Physna type is authoritative for existing fields; this declared
/// type only governs how a *new* field is registered.
pub const DECLARED_TYPES: [&str; 4] = ["text", "number", "boolean", "url"];

/// How an asset is identified by a batch CSV row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BatchAssetRef {
    /// Asset UUID from the UI format's "id" column. Takes precedence over the
    /// path and is never resolved via path lookup.
    Uuid(uuid::Uuid),
    /// Asset path (leading slash already stripped).
    Path(String),
}

impl BatchAssetRef {
    /// Human-readable identifier used for progress display, caching, and
    /// error messages.
    pub fn display(&self) -> String {
        match self {
            BatchAssetRef::Uuid(uuid) => uuid.to_string(),
            BatchAssetRef::Path(path) => path.clone(),
        }
    }
}

/// The CSV layout requested on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchCsvFormat {
    /// Detect from the header row (default).
    Auto,
    /// Classic vertical ASSET_PATH,NAME,VALUE layout.
    Classic,
    /// Horizontal layout exported by the Physna UI.
    Ui,
}

impl BatchCsvFormat {
    pub fn from_arg(value: &str) -> Self {
        match value {
            "classic" => BatchCsvFormat::Classic,
            "ui" => BatchCsvFormat::Ui,
            _ => BatchCsvFormat::Auto,
        }
    }
}

/// Metadata assignments for a single asset.
#[derive(Debug)]
pub struct BatchEntry {
    pub asset: BatchAssetRef,
    /// Raw string values keyed by metadata field name. An empty string means
    /// "delete this field"; empty values are only produced when the file is
    /// parsed with `delete_if_empty`, otherwise they are skipped.
    pub metadata: HashMap<String, String>,
    /// Optional caller-declared field type keyed by metadata field name, from
    /// the classic layout's fourth `TYPE` column. Only used when *registering*
    /// a new field; for a field that already exists in Physna the registered
    /// type is authoritative. Absent entries default to `text`.
    pub types: HashMap<String, String>,
}

/// Result of parsing a batch CSV file.
#[derive(Debug)]
pub struct ParsedBatch {
    /// One entry per distinct asset, in file order. Rows referring to the
    /// same asset are merged (later rows win on field conflicts).
    pub entries: Vec<BatchEntry>,
    /// Non-fatal issues (e.g. ignored columns) to surface before processing.
    pub warnings: Vec<String>,
    /// The layout that was actually used after auto-detection.
    pub format: BatchCsvFormat,
}

/// Parse a batch metadata CSV from `reader`, detecting the layout if
/// `requested` is [`BatchCsvFormat::Auto`].
///
/// When `delete_if_empty` is true, empty values are kept in the entries as
/// delete markers; otherwise they are skipped with a warning.
#[allow(clippy::result_large_err)]
pub fn parse_batch_csv<R: Read>(
    reader: R,
    requested: BatchCsvFormat,
    delete_if_empty: bool,
) -> Result<ParsedBatch, CliActionError> {
    // Flexible mode allows rows to have a different field count than the header.
    // This makes the classic layout's fourth TYPE column optional per row: some
    // rows may supply it and others may omit it without a length error.
    let mut csv_reader = csv::ReaderBuilder::new().flexible(true).from_reader(reader);
    let headers = csv_reader.headers()?.clone();

    let has_metadata_columns = headers
        .iter()
        .any(|h| is_metadata_column(h.trim()).is_some());

    let format = match requested {
        BatchCsvFormat::Auto => {
            if has_metadata_columns {
                BatchCsvFormat::Ui
            } else {
                BatchCsvFormat::Classic
            }
        }
        explicit => explicit,
    };

    match format {
        BatchCsvFormat::Ui => parse_ui(csv_reader, &headers, delete_if_empty),
        _ => parse_classic(csv_reader, delete_if_empty),
    }
}

/// Case-insensitive check for the `metadata:` prefix; returns the field name
/// (trimmed, prefix stripped) when the header is a metadata column.
fn is_metadata_column(header: &str) -> Option<&str> {
    let prefix_len = METADATA_COLUMN_PREFIX.len();
    // `get` (rather than direct slicing) avoids a panic when the byte at
    // `prefix_len` falls inside a multi-byte UTF-8 character; such a header
    // cannot start with the all-ASCII prefix anyway.
    match (header.get(..prefix_len), header.get(prefix_len..)) {
        (Some(prefix), Some(rest)) if prefix.eq_ignore_ascii_case(METADATA_COLUMN_PREFIX) => {
            Some(rest.trim())
        }
        _ => None,
    }
}

/// Accumulates entries grouped by asset reference while preserving file order.
struct EntryAccumulator {
    entries: Vec<BatchEntry>,
    index_by_asset: HashMap<BatchAssetRef, usize>,
}

impl EntryAccumulator {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            index_by_asset: HashMap::new(),
        }
    }

    fn add(
        &mut self,
        asset: BatchAssetRef,
        field: String,
        value: String,
        declared_type: Option<String>,
    ) {
        let index = *self.index_by_asset.entry(asset.clone()).or_insert_with(|| {
            self.entries.push(BatchEntry {
                asset,
                metadata: HashMap::new(),
                types: HashMap::new(),
            });
            self.entries.len() - 1
        });
        if let Some(declared_type) = declared_type {
            self.entries[index]
                .types
                .insert(field.clone(), declared_type);
        }
        self.entries[index].metadata.insert(field, value);
    }
}

/// Parse the classic vertical ASSET_PATH,NAME,VALUE layout.
#[allow(clippy::result_large_err)]
fn parse_classic<R: Read>(
    mut csv_reader: csv::Reader<R>,
    delete_if_empty: bool,
) -> Result<ParsedBatch, CliActionError> {
    let mut accumulator = EntryAccumulator::new();
    let mut skipped_empty = 0usize;
    let mut warnings = Vec::new();
    let mut invalid_types: Vec<String> = Vec::new();

    for result in csv_reader.records() {
        let record = result?;
        if record.len() >= 3 {
            let asset_path = record[0].trim();
            let metadata_name = record[1].trim();
            let metadata_value = record[2].trim();
            // Optional fourth column declares the field type for registration.
            let declared_type_raw = record.get(3).map(str::trim).unwrap_or("");

            // Empty values either mark the field for deletion (with
            // --delete-if-empty) or are skipped, leaving the field untouched.
            if metadata_value.is_empty() && !delete_if_empty {
                skipped_empty += 1;
                continue;
            }

            // Normalize and validate the declared type. An unrecognized type is
            // not fatal: it is reported and the field falls back to the default
            // (text for a new field, or the registered type if it already
            // exists).
            let declared_type = if declared_type_raw.is_empty() {
                None
            } else {
                let normalized = declared_type_raw.to_ascii_lowercase();
                if DECLARED_TYPES.contains(&normalized.as_str()) {
                    Some(normalized)
                } else {
                    invalid_types.push(declared_type_raw.to_string());
                    None
                }
            };

            accumulator.add(
                BatchAssetRef::Path(normalize_batch_asset_path(asset_path)),
                metadata_name.to_string(),
                metadata_value.to_string(),
                declared_type,
            );
        }
    }

    if skipped_empty > 0 {
        warnings.push(format!(
            "{} row(s) with an empty VALUE were skipped; pass --delete-if-empty to delete those metadata fields instead",
            skipped_empty
        ));
    }
    if !invalid_types.is_empty() {
        invalid_types.sort();
        invalid_types.dedup();
        warnings.push(format!(
            "Ignoring unrecognized TYPE value(s): {} (expected one of: {}); those fields fall back to their existing or default (text) type",
            invalid_types.join(", "),
            DECLARED_TYPES.join(", ")
        ));
    }

    Ok(ParsedBatch {
        entries: accumulator.entries,
        warnings,
        format: BatchCsvFormat::Classic,
    })
}

/// Parse the horizontal layout exported by the Physna UI.
#[allow(clippy::result_large_err)]
fn parse_ui<R: Read>(
    mut csv_reader: csv::Reader<R>,
    headers: &csv::StringRecord,
    delete_if_empty: bool,
) -> Result<ParsedBatch, CliActionError> {
    let mut warnings: Vec<String> = Vec::new();
    let mut path_column: Option<usize> = None;
    let mut id_column: Option<usize> = None;
    let mut metadata_columns: Vec<(usize, String)> = Vec::new();
    let mut ignored_columns: Vec<String> = Vec::new();

    for (index, header) in headers.iter().enumerate() {
        let header = header.trim();
        if let Some(field_name) = is_metadata_column(header) {
            if field_name.is_empty() {
                warnings.push(format!(
                    "Column {} ('{}') has an empty metadata field name and will be ignored",
                    index + 1,
                    header
                ));
            } else {
                metadata_columns.push((index, field_name.to_string()));
            }
        } else if header.eq_ignore_ascii_case("path") {
            path_column = Some(index);
        } else if header.eq_ignore_ascii_case("id") {
            id_column = Some(index);
        } else {
            ignored_columns.push(header.to_string());
        }
    }

    if !ignored_columns.is_empty() {
        warnings.push(format!(
            "Ignoring unrecognized column(s): {} (metadata columns must be prefixed with '{}')",
            ignored_columns.join(", "),
            METADATA_COLUMN_PREFIX
        ));
    }

    if path_column.is_none() && id_column.is_none() {
        return Err(CliActionError::BusinessLogicError(
            "UI-format CSV must contain a 'path' or 'id' column identifying each asset".to_string(),
        ));
    }

    if metadata_columns.is_empty() {
        return Err(CliActionError::BusinessLogicError(format!(
            "UI-format CSV must contain at least one metadata column (a column name prefixed with '{}')",
            METADATA_COLUMN_PREFIX
        )));
    }

    let mut accumulator = EntryAccumulator::new();

    for result in csv_reader.records() {
        let record = result?;
        let line = record
            .position()
            .map(|p| p.line().to_string())
            .unwrap_or_else(|| "?".to_string());

        let id_value = id_column
            .and_then(|i| record.get(i))
            .map(str::trim)
            .unwrap_or("");
        let path_value = path_column
            .and_then(|i| record.get(i))
            .map(str::trim)
            .unwrap_or("");

        // UUID takes precedence over the path; an invalid UUID is an error
        // rather than a fallback to the path, because falling back could
        // silently target a different asset than the one intended.
        let asset = if !id_value.is_empty() {
            let uuid = uuid::Uuid::parse_str(id_value).map_err(|e| {
                CliActionError::BusinessLogicError(format!(
                    "Line {}: invalid UUID '{}' in 'id' column: {}",
                    line, id_value, e
                ))
            })?;
            BatchAssetRef::Uuid(uuid)
        } else if !path_value.is_empty() {
            BatchAssetRef::Path(normalize_batch_asset_path(path_value))
        } else {
            return Err(CliActionError::BusinessLogicError(format!(
                "Line {}: row has neither an 'id' nor a 'path' value to identify the asset",
                line
            )));
        };

        let mut row_has_values = false;
        for (index, field_name) in &metadata_columns {
            let value = record.get(*index).map(str::trim).unwrap_or("");
            // Empty cells mark the field for deletion (with --delete-if-empty)
            // or are skipped, leaving the existing field value untouched.
            if !value.is_empty() || delete_if_empty {
                // The UI layout has no type column; new fields register as text
                // and existing fields use their registered type.
                accumulator.add(asset.clone(), field_name.clone(), value.to_string(), None);
                row_has_values = true;
            }
        }

        if !row_has_values {
            warnings.push(format!(
                "Line {}: no metadata values for asset '{}'; row skipped",
                line,
                asset.display()
            ));
        }
    }

    Ok(ParsedBatch {
        entries: accumulator.entries,
        warnings,
        format: BatchCsvFormat::Ui,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::result_large_err)]
    fn parse(content: &str, format: BatchCsvFormat) -> Result<ParsedBatch, CliActionError> {
        parse_batch_csv(content.as_bytes(), format, false)
    }

    #[allow(clippy::result_large_err)]
    fn parse_delete_if_empty(
        content: &str,
        format: BatchCsvFormat,
    ) -> Result<ParsedBatch, CliActionError> {
        parse_batch_csv(content.as_bytes(), format, true)
    }

    #[test]
    fn auto_detects_classic_format() {
        let csv = "ASSET_PATH,NAME,VALUE\n\
                   folder/a.stl,Material,Steel\n\
                   folder/a.stl,Weight,15.5 kg\n\
                   /folder/b.stl,Material,Aluminum\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.format, BatchCsvFormat::Classic);
        assert_eq!(parsed.entries.len(), 2);
        let a = &parsed.entries[0];
        assert_eq!(a.asset, BatchAssetRef::Path("folder/a.stl".to_string()));
        assert_eq!(a.metadata.get("Material").unwrap(), "Steel");
        assert_eq!(a.metadata.get("Weight").unwrap(), "15.5 kg");
        // Leading slash is stripped
        let b = &parsed.entries[1];
        assert_eq!(b.asset, BatchAssetRef::Path("folder/b.stl".to_string()));
    }

    #[test]
    fn classic_home_prefix_normalizes_to_root() {
        // Physna shows the root folder as "Home"; a "/Home/..." path means the
        // same asset as the equivalent root-relative path. All three spellings
        // must resolve to the same asset reference.
        let csv = "ASSET_PATH,NAME,VALUE\n\
                   /Home/NX/1.prt,Qty,1\n\
                   /NX/1.prt,Color,Blue\n\
                   NX/1.prt,Material,Steel\n\
                   /home/NX/2.prt,Qty,2\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        // First three rows all target NX/1.prt and merge into one entry.
        assert_eq!(parsed.entries.len(), 2);
        let a = &parsed.entries[0];
        assert_eq!(a.asset, BatchAssetRef::Path("NX/1.prt".to_string()));
        assert_eq!(a.metadata.len(), 3);
        // Case-insensitive: "/home/..." also normalizes.
        let b = &parsed.entries[1];
        assert_eq!(b.asset, BatchAssetRef::Path("NX/2.prt".to_string()));
    }

    #[test]
    fn ui_home_prefix_normalizes_to_root() {
        let csv = "path,id,metadata:Material\n\
                   /Home/NX/1.prt,,Steel\n\
                   NX/1.prt,,Aluminum\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        // Both rows target NX/1.prt; the second row wins on the field conflict.
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(
            parsed.entries[0].asset,
            BatchAssetRef::Path("NX/1.prt".to_string())
        );
        assert_eq!(
            parsed.entries[0].metadata.get("Material").unwrap(),
            "Aluminum"
        );
    }

    #[test]
    fn classic_reads_optional_type_column() {
        let csv = "ASSET_PATH,NAME,VALUE,TYPE\n\
                   NX/1.prt,Inventory Qty,18,number\n\
                   NX/1.prt,Supplier Link,https://x.com/,url\n\
                   NX/1.prt,Description,A part,\n\
                   NX/1.prt,Material,Steel\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.format, BatchCsvFormat::Classic);
        let entry = &parsed.entries[0];
        assert_eq!(entry.metadata.get("Inventory Qty").unwrap(), "18");
        assert_eq!(entry.types.get("Inventory Qty").unwrap(), "number");
        assert_eq!(entry.types.get("Supplier Link").unwrap(), "url");
        // Empty TYPE cell and a missing TYPE column both mean "no declared type".
        assert!(!entry.types.contains_key("Description"));
        assert!(!entry.types.contains_key("Material"));
    }

    #[test]
    fn classic_type_column_is_case_insensitive() {
        let csv = "ASSET_PATH,NAME,VALUE,TYPE\n\
                   NX/1.prt,Qty,18,NUMBER\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.entries[0].types.get("Qty").unwrap(), "number");
    }

    #[test]
    fn classic_unknown_type_is_warned_and_ignored() {
        let csv = "ASSET_PATH,NAME,VALUE,TYPE\n\
                   NX/1.prt,Qty,18,integer\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        // Value is still captured; only the bad type declaration is dropped.
        assert_eq!(parsed.entries[0].metadata.get("Qty").unwrap(), "18");
        assert!(!parsed.entries[0].types.contains_key("Qty"));
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("unrecognized TYPE") && w.contains("integer")));
    }

    #[test]
    fn classic_skips_empty_values_by_default() {
        let csv = "ASSET_PATH,NAME,VALUE\n\
                   folder/a.stl,Material,\n\
                   folder/a.stl,Weight,15.5 kg\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        let entry = &parsed.entries[0];
        assert_eq!(entry.metadata.len(), 1);
        assert_eq!(entry.metadata.get("Weight").unwrap(), "15.5 kg");
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("--delete-if-empty")));
    }

    #[test]
    fn classic_row_with_only_empty_values_produces_no_entry() {
        let csv = "ASSET_PATH,NAME,VALUE\n\
                   folder/a.stl,Material,\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert!(parsed.entries.is_empty());
    }

    #[test]
    fn classic_keeps_empty_values_with_delete_if_empty() {
        let csv = "ASSET_PATH,NAME,VALUE\n\
                   folder/a.stl,Material,\n";
        let parsed = parse_delete_if_empty(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.entries[0].metadata.get("Material").unwrap(), "");
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn auto_detects_ui_format() {
        let csv = "\"path\",\"id\",\"metadata:Material\",\"metadata:Color\"\n\
                   \"/domain/part1.sldprt\",\"\",\"Steel\",\"Blue\"\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.format, BatchCsvFormat::Ui);
        assert_eq!(parsed.entries.len(), 1);
        let entry = &parsed.entries[0];
        assert_eq!(
            entry.asset,
            BatchAssetRef::Path("domain/part1.sldprt".to_string())
        );
        assert_eq!(entry.metadata.get("Material").unwrap(), "Steel");
        assert_eq!(entry.metadata.get("Color").unwrap(), "Blue");
    }

    #[test]
    fn ui_uuid_takes_precedence_over_path() {
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let csv = format!(
            "path,id,metadata:Material\n\
             /domain/part1.sldprt,{},Steel\n",
            uuid
        );
        let parsed = parse(&csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(
            parsed.entries[0].asset,
            BatchAssetRef::Uuid(uuid::Uuid::parse_str(uuid).unwrap())
        );
    }

    #[test]
    fn ui_invalid_uuid_is_an_error() {
        let csv = "path,id,metadata:Material\n\
                   /domain/part1.sldprt,12345-67890,Steel\n";
        let error = parse(csv, BatchCsvFormat::Auto).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("invalid UUID"), "got: {}", message);
        assert!(message.contains("Line 2"), "got: {}", message);
    }

    #[test]
    fn ui_empty_cells_are_skipped_by_default() {
        let csv = "path,id,metadata:Material,metadata:Color,metadata:Weight\n\
                   /domain/assembly.sldasm,,Mixed,,\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        let entry = &parsed.entries[0];
        assert_eq!(entry.metadata.len(), 1);
        assert_eq!(entry.metadata.get("Material").unwrap(), "Mixed");
    }

    #[test]
    fn ui_empty_cells_are_kept_with_delete_if_empty() {
        let csv = "path,id,metadata:Material,metadata:Color,metadata:Weight\n\
                   /domain/assembly.sldasm,,Mixed,,\n";
        let parsed = parse_delete_if_empty(csv, BatchCsvFormat::Auto).unwrap();
        let entry = &parsed.entries[0];
        assert_eq!(entry.metadata.len(), 3);
        assert_eq!(entry.metadata.get("Material").unwrap(), "Mixed");
        assert_eq!(entry.metadata.get("Color").unwrap(), "");
        assert_eq!(entry.metadata.get("Weight").unwrap(), "");
    }

    #[test]
    fn ui_row_with_all_empty_cells_is_kept_with_delete_if_empty() {
        let csv = "path,id,metadata:Material\n\
                   /domain/part1.sldprt,,\n";
        let parsed = parse_delete_if_empty(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].metadata.get("Material").unwrap(), "");
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn ui_row_with_no_identifier_is_an_error() {
        let csv = "path,id,metadata:Material\n\
                   ,,Steel\n";
        let error = parse(csv, BatchCsvFormat::Auto).unwrap_err();
        assert!(error.to_string().contains("neither an 'id' nor a 'path'"));
    }

    #[test]
    fn ui_unknown_columns_are_warned_and_ignored() {
        let csv = "path,id,name,metadata:Material\n\
                   /domain/part1.sldprt,,Part One,Steel\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("unrecognized column") && w.contains("name")));
        let entry = &parsed.entries[0];
        assert_eq!(entry.metadata.len(), 1);
        assert!(entry.metadata.contains_key("Material"));
    }

    #[test]
    fn ui_without_identifier_column_is_an_error() {
        let csv = "metadata:Material\nSteel\n";
        let error = parse(csv, BatchCsvFormat::Auto).unwrap_err();
        assert!(error.to_string().contains("'path' or 'id' column"));
    }

    #[test]
    fn forced_ui_without_metadata_columns_is_an_error() {
        let csv = "path,id\n/domain/part1.sldprt,\n";
        let error = parse(csv, BatchCsvFormat::Ui).unwrap_err();
        assert!(error.to_string().contains("at least one metadata column"));
    }

    #[test]
    fn forced_classic_ignores_metadata_prefix() {
        // A file that would auto-detect as UI is parsed positionally when
        // classic is forced.
        let csv = "ASSET_PATH,NAME,metadata:VALUE\n\
                   folder/a.stl,Material,Steel\n";
        let parsed = parse(csv, BatchCsvFormat::Classic).unwrap();
        assert_eq!(parsed.format, BatchCsvFormat::Classic);
        assert_eq!(parsed.entries[0].metadata.get("Material").unwrap(), "Steel");
    }

    #[test]
    fn ui_duplicate_asset_rows_are_merged() {
        let csv = "path,id,metadata:Material,metadata:Color\n\
                   /domain/part1.sldprt,,Steel,\n\
                   /domain/part1.sldprt,,,Blue\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        let entry = &parsed.entries[0];
        assert_eq!(entry.metadata.get("Material").unwrap(), "Steel");
        assert_eq!(entry.metadata.get("Color").unwrap(), "Blue");
    }

    #[test]
    fn multibyte_utf8_header_does_not_panic() {
        // Regression: byte-slicing the header at the prefix length panicked
        // when the boundary fell inside a multi-byte UTF-8 character.
        let csv = "ASSET_PATH,NAME,metadataé\n\
                   folder/a.stl,Material,Steel\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        // "metadataé" is not a metadata: column, so this is classic format.
        assert_eq!(parsed.format, BatchCsvFormat::Classic);
    }

    #[test]
    fn ui_metadata_prefix_is_case_insensitive() {
        let csv = "path,id,Metadata:Material\n\
                   /domain/part1.sldprt,,Steel\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.format, BatchCsvFormat::Ui);
        assert!(parsed.entries[0].metadata.contains_key("Material"));
    }

    #[test]
    fn ui_row_with_all_empty_metadata_warns_and_skips() {
        let csv = "path,id,metadata:Material\n\
                   /domain/part1.sldprt,,\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert!(parsed.entries.is_empty());
        assert!(parsed.warnings.iter().any(|w| w.contains("row skipped")));
    }

    #[test]
    fn sample_template_file_shape_parses() {
        // Mirrors data/horizontal-metadata/metadata-template.csv but with a
        // valid UUID in the id column.
        let csv = "\"path\",\"id\",\"metadata:Material\",\"metadata:Color\",\"metadata:Weight\"\n\
                   \"/domain/assets/part1.sldprt\",\"123e4567-e89b-12d3-a456-426614174000\",\"Steel\",\"Blue\",\"2.5kg\"\n\
                   \"/domain/assets/part2.step\",\"\",\"Aluminum\",\"Red\",\"1.2kg\"\n\
                   \"/domain/assets/assembly.sldasm\",\"\",\"Mixed\",\"\",\"\"\n";
        let parsed = parse(csv, BatchCsvFormat::Auto).unwrap();
        assert_eq!(parsed.entries.len(), 3);
        assert!(matches!(parsed.entries[0].asset, BatchAssetRef::Uuid(_)));
        assert_eq!(
            parsed.entries[1].asset,
            BatchAssetRef::Path("domain/assets/part2.step".to_string())
        );
        assert_eq!(parsed.entries[2].metadata.len(), 1);
    }
}

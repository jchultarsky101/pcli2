# Integration tests

Every file here runs against the built binary or against a `mockito` server; none
of them contacts Physna. Run them all with `cargo test`, or one file with
`cargo test --test <name>`.

| File | What it covers |
|------|----------------|
| `cli_help_test.rs` | Help and version output for every level of the command tree, as a guard against accidental interface changes. |
| `error_tests.rs` | Error types, their messages, and the exit-code contract (64 usage, 65 data, 66 missing input, 67 not found, 69 temporary failure, 78 configuration, 100 authentication, 101 network, 102 API). |
| `format_model_tests.rs` | JSON, CSV and tree formatting of the data model. |
| `asset_tests.rs` | Asset model behaviour. |
| `folder_resolution_test.rs` | Folder path resolution against a mock API: root, existing, and non-existent paths. |
| `metadata_fields_pagination_test.rs` | Metadata field listing across pages of a mock API. |
| `download_to_file_test.rs` | Streamed downloads: whole file written through a temporary file, empty body refused, server errors classified. |
| `token_renewal_test.rs` | Token renewal against a mock API and auth server: a 401 renews once and the retry carries the new token, a burst of concurrent 401s costs one renewal, credentials without a token authenticate before the first request, a rejected credential reports the cause and keeps the old token, no credentials means no renewal. |
| `output_shape_test.rs` | Every formatter in every format it supports: CSV parses with no ragged rows and `--headers` adds one line, nothing ends with a line break, compact JSON is one line and pretty JSON is the same value, unsupported formats are clean errors. |
| `csv_trailing_newline_test.rs` | The original regression case for the trailing-line-break bug. |
| `openapi_contract_test.rs` | Contract tests against the snapshot of Physna's OpenAPI spec in `fixtures/physna-openapi.json`: for every endpoint the client calls, a body generated from the spec (required properties only, then all of them) must deserialize into the model type; hard-coded enumerations and page sizes are checked against the spec; every URL the client builds must exist. The ignored `live_spec_matches_the_snapshot` fetches the current spec and reports drift (run weekly by the `spec-drift` workflow). |
| `removed_flags_test.rs` | The flag spellings removed in 2.0 (`--file`, `--files`, `--csv-file`, `--local-path`, the positional output on `asset download`) are refused with exit 64 and a message naming the replacement, in text and JSON error mode; the new spellings still parse. |
| `dependencies_by_id_test.rs` | The dependency tree against a mock API on the ID-based endpoint: a missing dependency is listed but never expanded (no request for the nil UUID), a present dependency with its own dependencies is expanded, and a dependency without a `status` counts as missing only when it has no asset. |
| `metadata_fields_test.rs` | Metadata-field management (`tenant metadata rename|delete|assets|coverage|missing`) against a mock API: the listing carries field ids, rename patches the name, delete sends `force`, the assets-using-a-field listing pages and stops at `--limit`, the without-metadata listing sends the folder and extension filters, coverage is read. |
| `resolve_dependency_test.rs` | Resolving a missing dependency (`asset resolve-dependency`) against a mock API: the request carries the dependency path and the stand-in asset's id and succeeds on 204; a rejection is an error with the server's message. |
| `move_asset_test.rs` | Moving an asset (`asset move`) against a mock API: a folder destination sends its id and the asset comes back with its new path; the root sends `null`. |
| `batch_get_test.rs` | Fetching several assets by id (`asset get --uuid A --uuid B`) against a mock API: one request per 1000 ids, results in request order, duplicates asked once, absent ids reported by `missing_asset_ids`, an empty list makes no request. |
| `existing_paths_test.rs` | The existing-paths check behind `--skip-existing` against a mock API: 1001 paths go out as two requests whose answers merge, an empty batch makes no request, a server error is an error (never "nothing exists"), and `asset_path_for` builds one spelling. |
| `replace_asset_file_test.rs` | Replacing an asset's file (`asset create --override`) against a mock API: the file goes out as a multipart PUT and the asset comes back with its UUID and `indexing` state, a 409 keeps the server's wording instead of "already exists", and a missing local file is refused before any request. |
| `failure_diagnostics_test.rs` | The failure-diagnostics endpoints against a mock API: a `found` answer carries every field of the spec's example, a status-only answer is accepted, `unavailable` maps to exit 68 with a message naming the cause, availability is a plain boolean, and the recent-failures listing walks every page, keeps the tenant-wide totals, sends the `kinds` filter and stops at `--limit`. |
| `no_input_and_json_errors_test.rs` | The built binary with `--no-input` and `--error-format json`: prompts refused with exit 64 and a named flag, usage errors and the final error as JSON objects, `--stats` as JSON. |

Unit tests live next to the code they test (`#[cfg(test)]` modules), including the
HTTP retry path in `src/http_utils.rs`, the checkpoint file format in
`src/checkpoint.rs`, the search-failure classifier in
`src/actions/assets/match_ops.rs`, and the metadata CSV parser in
`src/actions/assets/metadata_batch_csv.rs`.

Tests that need a configuration or cache directory point `PCLI2_CONFIG_DIR` and
`PCLI2_CACHE_DIR` at a temporary directory so they never touch the real ones.

# Contributing to PCLI2

## Development

```bash
cargo build
cargo test                                          # unit, integration and end-to-end tests
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings   # what CI runs
rustup run 1.88 cargo check --all-targets           # the minimum supported Rust (rust-version)
```

CI (`.github/workflows/rust.yml`) runs the same checks on Linux, macOS and
Windows, plus `cargo audit` and the MSRV build. A dependency update that needs a
newer Rust shows up first in the `msrv` job.

## Branches and releases

GitFlow: work happens on a branch off `develop` and is merged into `develop`
through a pull request. A release merges `develop` into `main`, tags `vX.Y.Z`,
and merges `main` back into `develop`. cargo-dist (`dist-workspace.toml`,
`.github/workflows/release.yml`) builds the binaries, the installers and the MSI
for the tag, publishes the GitHub Release, and pushes the Homebrew formula to
`jchultarsky101/homebrew-pcli2`.

Record every user-visible change in `CHANGELOG.md` under `[Unreleased]`
(Keep a Changelog). The release notes are generated from it; a file that does not
parse (for example two headings for the same version) makes them silently empty.

## Tests

- **Library tests** call the API client against a mockito server
  (`tests/*_test.rs`).
- **End-to-end tests** run the real binary against a mock API through the
  harness in `tests/common/mod.rs`: a throwaway configuration pointing an
  environment at the mock server, a long-lived token, and the tenant lookup.
  Use it for anything a command does beyond one client call.
- **Contract tests** (`tests/openapi_contract_test.rs`) check the client's models
  and URLs against a snapshot of Physna's OpenAPI specification in
  `tests/fixtures/`; the weekly `spec-drift` workflow compares the snapshot with
  the live specification.
- Every example in `--help` is parsed against the command tree
  (`tests/help_examples_test.rs`).

Scripts depend on pcli2's output: do not rename or reorder CSV columns or change
JSON field names in a minor release. Add columns at the end.

## Documentation

The user guide is `docs/src/` (mdBook structure, built by oranda with `README.md`
as the home page). `.github/workflows/documentation.yml` deploys it to GitHub
Pages after the Release workflow completes, so the install links point at the
new version; it can also be started by hand (workflow_dispatch). Preview locally:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/oranda/releases/latest/download/oranda-installer.sh | sh
oranda build && (cd public && python3 -m http.server 8000)
```

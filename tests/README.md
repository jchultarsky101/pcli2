# CLI Help Verification Tests

This directory contains comprehensive tests that verify the CLI help output remains consistent and hasn't been accidentally changed. These tests are important for maintaining a stable user interface across all command levels.

## Test Files

### `cli_help_test.rs`
Contains functional tests that verify:
- Main CLI help output (`pcli2 --help`)
- Top-level subcommand help outputs (`pcli2 tenant --help`, `pcli2 asset --help`, etc.)
- Second-level nested subcommand help outputs (`pcli2 tenant list --help`, `pcli2 asset create --help`, etc.)
- Deeply nested subcommand help outputs (`pcli2 asset metadata get --help`, `pcli2 config environment add --help`, etc.)
- Version output (`pcli2 --version`)

### `cli_help_snapshot_test.rs`
Contains snapshot-style tests that capture the current help output format and verify it remains consistent over time. These tests help detect unintended changes to the CLI interface across all command levels:
- Main command help snapshots
- Top-level subcommand help snapshots
- Second-level nested subcommand help snapshots
- Deeply nested subcommand help snapshots

## Comprehensive Coverage

The tests cover all levels of the CLI hierarchy:
- **Level 1**: Main commands (`tenant`, `folder`, `asset`, `auth`, `context`, `config`)
- **Level 2**: Direct subcommands (`asset list`, `asset create`, `config get`, etc.)
- **Level 3**: Deeply nested commands (`asset metadata get`, `context set tenant`, `config environment add`, etc.)

## Purpose

These tests serve to:
1. Verify that the CLI help output contains expected elements at all levels
2. Ensure that major command groups remain accessible
3. Detect accidental changes to the CLI interface across all command depths
4. Maintain consistency in the user experience
5. Prevent breaking changes to the command structure

## Running the Tests

```bash
# Run all tests
cargo test

# Run specific help tests
cargo test --test cli_help_test
cargo test --test cli_help_snapshot_test

# Run the manual verification script
bash test_help_output.sh
```

## Manual Verification

The `test_help_output.sh` script provides a quick way to manually verify all help outputs at once.
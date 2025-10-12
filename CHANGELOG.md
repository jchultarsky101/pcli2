# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Geometric matching functionality for single assets (`asset geometric-match`)
- Geometric matching functionality for folders with parallel processing (`asset geometric-match-folder`)
- Configurable concurrency for folder-based geometric matching
- Progress bar option for long-running operations
- Retry mechanism for HTTP 409 Conflict errors
- Enhanced output formats with full asset paths and consistent naming
- Comprehensive trace logging for debugging
- Test scripts for API endpoint verification
- Missing methods to CsvRecordProducer trait: `csv_header_with_metadata()` and `as_csv_records_with_metadata()`

### Changed
- Improved error handling with standardized logging
- Enhanced JSON and CSV output formats for geometric matching results
- Updated command-line interface with new options and flags
- Updated trait implementations to support metadata-aware CSV output

### Fixed
- Compilation errors related to missing trait methods in `CsvRecordProducer`
- Unused variable warnings throughout the codebase
- Mutability warnings by removing unnecessary `mut` keywords
- Documentation example for `Asset::new` to include all required parameters
- Duplicate error messages in geometric matching commands
- Asset name extraction in geometric matching results

## [0.1.0] - 2023-01-17

### Added
- Initial release of PCLI2
- Basic asset management commands (create, list, get, delete)
- Folder management commands (create, list, get, delete, update)
- Tenant management functionality
- Authentication with OAuth2 client credentials flow
- Configuration management
- Context management for multi-tenant support

[Unreleased]: https://github.com/physna/pcli2/compare/v0.1.0...HEAD
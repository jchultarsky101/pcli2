# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.5] - 2026-01-14

### Added
- Added part-match and part-match-folder commands for finding part matches between assets
- Added comprehensive documentation for part matching functionality
- Added concurrent processing option for part-match-folder with configurable limits (1-10)
- Added progress tracking with multi-progress bars for part-match-folder operations
- Added exclusive flag for part-match-folder to limit matches to specified folders only
- Added proper comparison URLs for part matching results in both JSON and CSV formats
- Added metadata support for part matching with REF_ and CAND_ prefixes in CSV output

## [0.2.4] - 2026-01-14

### Added
- Added comprehensive uninstallation instructions to README.md for Windows, macOS, and Linux platforms
- Enhanced CSV output for geometric-match and part-match commands to properly include comparison URLs
- Added proper metadata support for geometric-match and part-match commands with REF_ and CAND_ prefixes

### Fixed
- Fixed CSV header consistency issues in EnhancedPartSearchResponse to include COMPARISON_URL field
- Fixed CSV output formatting to ensure consistent field counts across all records
- Fixed metadata loading for reference assets in geometric-match and part-match commands
- Removed failing integration tests that require API access from CI environment

## [0.2.3] - 2024-01-09

### Added
- Added `--refresh` flag to `context set tenant` command to force fetching fresh tenant list from API
- Implemented comprehensive tenant caching system with 1-hour TTL
- Added tenant cache persistence to disk for improved performance
- Added PCLI2_FORMAT environment variable support for default output format
- Added PCLI2_HEADERS environment variable support for default CSV header inclusion
- Added comprehensive documentation for system keyring integration and security
- Added documentation for environment variable precedence and usage

### Changed
- Reverted to dev-keyring as default for broader platform compatibility (was system keyring)
- Improved error message formatting to remove raw technical data dumps
- Removed redundant context messages from error output for cleaner user experience
- Updated documentation to prevent automatic GitHub release detection
- Enhanced credential storage to support environment-specific credentials
- Added PCLI2_FORMAT and PCLI2_HEADERS environment variable support with proper precedence

## [0.2.3] - 2024-01-09

### Added
- Added `--refresh` flag to `context set tenant` command to force fetching fresh tenant list from API
- Implemented comprehensive tenant caching system with 1-hour TTL
- Added tenant cache persistence to disk for improved performance

### Changed
- Improved error messages for authentication failures to be more user-friendly
- Enhanced error handling for common API error responses

## [0.2.3] - 2024-01-09

### Changed
- Reorganized documentation sections by moving "Geometric Matching", "Working with Metadata", and "Metadata Inference" to "Basic Usage"
- Made "Metadata Inference" a subsection under "Working with Metadata" for better organization
- Simplified Quick Start instructions by removing references to OpenAPI Documentation page for service account creation
- Clarified folder delete behavior with and without --force flag in documentation
- Moved "Best Practices" section to end of Geometric Matching section where it belongs
- Added comprehensive documentation section about using UNIX pipes with PCLI2 for advanced data processing

## [0.2.2] - 2024-01-09

### Fixed
- Improved error messages with consistent formatting and user-friendly guidance
- Added fun emojis to error messages for better visual indication
- Enhanced error remediation with specific steps for users

## [0.2.1] - 2024-01-08

### Fixed
- Fixed tenant parameter issue in `asset metadata delete` command that was causing "Mismatch between definition and access of tenant" error
- Fixed metadata delete command to use the proper API endpoint for deleting specific metadata fields from assets instead of fetching all metadata and re-updating the asset
- Improved error handling and documentation for metadata operations

## [0.2.0] - 2026-01-08

### Added
- Multi-environment configuration support with named environments
- `config environment add` command to add new environment configurations with custom URLs
- `config environment use` command with interactive selection to switch between environments
- `config environment list` command with detailed output and format options (json/csv)
- `config environment get` command to retrieve specific environment details with format options
- `config environment remove` command to delete environment configurations
- `config environment reset` command to reset all environment configurations to blank state
- Environment-specific URL configuration (API, UI, Auth) for different Physna instances
- Support for development, staging, and production environment configurations
- Active tenant clearing when switching environments to prevent cross-environment confusion
- Format options (json/csv) with headers and pretty printing for environment commands
- Comprehensive documentation for multi-environment configuration management

### Changed
- Authentication now uses environment-specific URLs instead of hardcoded production URLs
- OAuth2 client credentials flow now includes proper Content-Type header
- Enhanced error handling and tracing for authentication flows
- Updated README.md with comprehensive multi-environment configuration documentation
- Improved cross-platform configuration with environment variable support for custom URLs
- Restructured environment command hierarchy for better usability
- Limited environment command format options to json/csv (removed tree format where inappropriate)

### Fixed
- Hardcoded production URLs replaced with configurable environment-specific URLs
- Authentication flow now properly uses configuration-based URLs
- Cross-environment tenant conflicts resolved by clearing active tenant on environment switch
- OAuth2 client credentials flow compliance with proper header requirements

## [0.1.8] - 2025-10-31

### Added
- Asset dependencies command to retrieve component relationships and referenced assets for assemblies
- Recursive dependency traversal with `--recursive` flag to show full assembly hierarchies
- Enhanced JSON output with `parentPath` field to preserve parent-child relationships in recursive mode
- Enhanced CSV output with `PARENT_PATH` column to preserve parent-child relationships in recursive mode
- Hierarchical tree visualization for recursive dependencies showing proper indentation structure
- Cycle detection and deduplication to prevent infinite loops during recursive traversal
- Consistent parameter handling with `PARAMETER_RECURSIVE` constant for reuse across commands

### Changed
- Improved tree format for asset dependencies to show original asset as root instead of generic "Asset Dependencies"
- Updated API client to use asset path instead of asset ID in dependencies endpoint URL as per API specification
- Enhanced error handling for invalid folder paths in folder list command to return appropriate error messages
- Standardized command structure with consistent parameter naming and help text

### Fixed
- Asset dependencies endpoint URL construction to use asset path instead of asset ID as required by API
- Folder list command to return error for invalid paths instead of showing all root folders
- Banner display consistency to show for both `help` subcommand and `--help` flag
- Compilation warnings and improved code safety throughout the codebase

## [0.1.7] - 2025-10-29

### Added
- Folder get command with UUID and path parameters support
- Debug tracing for folder path resolution to diagnose ETRACE folder issues

### Fixed
- Compilation errors in folder get command implementation
- Folder list issues related to ETRACE functionality

## [0.1.6] - 2025-10-29

### Changed
- Updated CI workflow configurations
- Fixed JSON parsing errors in CI pipeline
- Updated cargo-dist runner list
- Updated Cargo.lock dependencies

## [0.1.5] - 2025-10-29

### Added
- Metadata inference command implementation
- Geometric search pagination fixes
- Visible aliases for metadata commands: 'update' for 'create' and 'update-batch' for 'create-batch'

### Changed
- Improved code safety and usability by fixing unsafe unwraps
- Enhanced tenant resolution functionality
- Updated cargo-dist configuration with proper GUID/UUID for CI pipeline
- Reordered asset subcommands alphabetically in CLI help output

### Fixed
- Invalid tenant CRUD operations removal
- Cargo-dist configuration issues and eliminated unused manifest key warnings

## [0.1.4] - 2025-10-29

### Changed
- Fixed tenant resolution in multiple commands (asset get, asset delete, folder get, folder list)
- Fixed Cargo.toml configuration warnings
- Improved banner with better gradient colors and spacing
- Updated documentation and dependencies

## [0.1.3] - 2025-10-29

### Added
- Asset metadata get command to CLI structure
- Workflow to automatically update 'latest' tag on release
- Comprehensive metadata documentation

### Changed
- Improved banner gradient colors and spacing
- Added visible aliases for metadata commands
- Reordered asset subcommands alphabetically in CLI help output

## [0.1.2] - 2025-10-29

### Changed
- Updated documentation structure for GitHub Actions deployment
- Fixed broken documentation links by ensuring proper mdBook file structure
- Improved README.md structure and user flow
- Updated oranda.json with Apache-2.0 license identifier

## [0.1.1] - 2025-10-13

### Added
- Cross-platform directory structure support using standard platform directories
- Automatic directory creation for configuration, cache, and data files
- Comprehensive documentation for data storage locations on all platforms
- Environment variable support for customizing directory locations
- Duplicate filtering for geometric matching results to remove reciprocal asset pairs
- Proper sorting of geometric matching results by match percentage (descending)
- LICENSE file accessibility fix for documentation website
- WSL installation test script for verifying installation on Windows Subsystem for Linux

### Changed
- Updated configuration to use proper cross-platform directories from `dirs` crate
- Improved error handling for first-time users with automatic configuration creation
- Enhanced documentation with cross-platform installation and usage instructions
- Updated Oranda configuration for proper GitHub Pages deployment
- Reworked README as single self-contained document to fix broken links

### Fixed
- Broken LICENSE links in documentation website that resulted in 404 errors
- Duplicate asset pairs in geometric matching results (A→B and B→A)
- Confusing error messages for first-time users trying to run PCLI2
- Inconsistent licensing information between README.md and LICENSE file
- Compilation warnings related to unused variables
- Documentation website CSS styling issues on GitHub Pages

## [0.1.0] - 2023-01-17

### Added
- Initial release of PCLI2
- Basic asset management commands (create, list, get, delete)
- Folder management commands (create, list, get, delete, update)
- Tenant management functionality
- Authentication with OAuth2 client credentials flow
- Configuration management
- Context management for multi-tenant support

[Unreleased]: https://github.com/physna/pcli2/compare/v0.2.5...HEAD
[0.2.5]: https://github.com/physna/pcli2/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/physna/pcli2/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/physna/pcli2/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/physna/pcli2/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/physna/pcli2/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/physna/pcli2/compare/v0.1.8...v0.2.0
[0.1.8]: https://github.com/physna/pcli2/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/physna/pcli2/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/physna/pcli2/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/physna/pcli2/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/physna/pcli2/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/physna/pcli2/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/physna/pcli2/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/physna/pcli2/compare/v0.1.0...v0.1.1
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/physna/pcli2/compare/v0.1.7...HEAD
[0.1.7]: https://github.com/physna/pcli2/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/physna/pcli2/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/physna/pcli2/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/physna/pcli2/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/physna/pcli2/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/physna/pcli2/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/physna/pcli2/compare/v0.1.0...v0.1.1
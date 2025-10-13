# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/physna/pcli2/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/physna/pcli2/compare/v0.1.0...v0.1.1
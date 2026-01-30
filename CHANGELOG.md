# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.19] - 2026-01-29

### Added
- Added --resume flag to folder download command to skip files that already exist in the destination directory
- Added detailed statistics report at the end of folder download showing successful, skipped, and failed downloads
- Extended all timeout values to 30 minutes (1800 seconds) to accommodate large files and slow connections
- Added short form (-n) for --name parameter in config environment commands
- Added comprehensive NuShell examples for advanced pipeline operations
- Added update instructions explaining different update methods based on installation approach
- Added proper statistics tracking for download operations

### Changed
- Improved folder download command to check for existing files before downloading when --resume flag is used
- Enhanced documentation with more detailed examples and parameter explanations
- Optimized resume functionality to apply delays only when actually downloading, not when skipping files
- Changed skip message log level from INFO to DEBUG in resume functionality

## [0.2.18] - 2026-01-29

### Added
- Added --resume flag to folder download command to skip files that already exist in the destination directory
- Added detailed statistics report at the end of folder download showing successful, skipped, and failed downloads
- Extended all timeout values to 30 minutes (1800 seconds) to accommodate large files and slow connections

## [0.2.17] - 2026-01-28

### Added
- Implemented performance and efficiency optimizations across multiple components
- Added HTTP connection pooling and optimized timeout configuration
- Implemented lazy loading and caching for expensive computations
- Added streaming for large file operations and pagination
- Implemented better error handling for individual file failures in batch operations
- Added configurable concurrency limits for batch operations
- Implemented bulk operations to reduce API calls
- Added caching mechanisms to avoid redundant API calls

### Changed
- Optimized batch processing with improved progress tracking and detailed reporting
- Enhanced path resolution logic to reduce redundant API calls
- Improved memory usage with streaming implementations for large files
- Refactored code to reduce unnecessary cloning operations

## [0.2.16] - 2026-01-27

### Added
- Added detailed shell completion instructions for all supported shells (Zsh, Bash, Fish, PowerShell, Elvish) to README.md

### Changed
- Enhanced shell completion documentation with comprehensive setup instructions for each shell
- Updated README.md with step-by-step instructions for applying shell completions

## [0.2.15] - 2026-01-27

### Added
- Added --metadata flag to asset get command to include metadata in output for both CSV and tree formats
- Added --continue-on-error flag to folder download command to continue downloading other assets if one fails
- Added --concurrent flag to folder download command to allow multiple concurrent downloads (range: 1-10)
- Added --delay flag to folder download command to add delays between downloads (range: 0-180 seconds)
- Added enhanced error logging with asset UUID and Physna path information to folder download command
- Added improved progress indicators for concurrent downloads using MultiProgress
- Added --resume flag to folder download command to skip files that already exist in the destination directory
- Added detailed statistics report at the end of folder download showing successful, skipped, and failed downloads
- Extended all timeout values to 30 minutes (1800 seconds) to accommodate large files and slow connections

### Changed
- Improved folder download command to show separate progress bars for each concurrent download when using --progress
- Enhanced error messages to include more specific information about which asset failed during download
- Updated documentation in README.md with examples of new flags and features

## [0.2.14] - 2026-01-25

### Added
- Added folder dependencies command to get dependencies for all assembly assets in one or more folders
- Added --progress flag to folder dependencies command for visual feedback during processing
- Added ASSEMBLY_PATH column to CSV output showing relative path within assembly hierarchy
- Added original_asset_path field to AssetDependency struct to track which original asset each dependency belongs to
- Added consistent state normalization converting "missing-dependencies" to "missing" for all formats

### Changed
- Moved match-folder commands from asset namespace to folder namespace: folder geometric-match, folder part-match, folder visual-match
- Renamed folder-based commands from geometric-match-folder to geometric-match (and similar for part-match, visual-match)
- Updated asset dependencies command to always operate recursively by default (removed --recursive flag)
- Improved CSV output sorting by ASSET_PATH then ASSEMBLY_PATH for consistent ordering
- Changed command structure to be more intuitive: folder dependencies instead of asset dependencies with folder paths
- Updated tree format to show state information alongside asset names
- Modified CSV output to show "None" instead of nil UUID when UUID is not available
- Improved assembly path handling to show proper hierarchy in all output formats

### Fixed
- Fixed issue where only first level of dependencies was shown instead of full hierarchy
- Fixed CSV output to properly show assembly hierarchy paths
- Fixed state display consistency across all output formats (tree, JSON, CSV)

## [0.2.13] - 2026-01-24

### Added
- Added text-match command with text-search alias to perform text-based asset searches
- Added --fuzzy flag to control search behavior (default: false for exact search with quoted text)
- Added TextMatch, TextSearchResponse, EnhancedTextSearchResponse, and TextMatchPair data models
- Added text_search API method to PhysnaApiClient for calling the text-search endpoint
- Added ASSET_NAME, TYPE, STATE, IS_ASSEMBLY, RELEVANCE_SCORE, ASSET_UUID, and ASSET_URL columns to CSV output
- Added aliases for existing match commands: geometric-search, part-search, visual-search
- Added folder variants for new aliases: geometric-search-folder, part-search-folder, visual-search-folder

### Changed
- Updated README.md with comprehensive documentation for text-match command and new aliases
- Modified text search to wrap queries in quotes by default for exact matching
- Improved asset URL construction to avoid duplicate tenant paths in URLs
- Enhanced CSV output format with additional asset metadata fields

## [0.2.12] - 2026-01-24

### Fixed
- Fixed token refresh mechanism to save refreshed tokens to keyring immediately after refresh in all API request methods
- Ensured subsequent commands use fresh tokens instead of expired ones after automatic refresh
- Resolved compilation warnings related to unused functions and imports
- Fixed AssemblyTree OutputFormatter implementation to properly handle all output formats (JSON, CSV, Tree)

### Changed
- Added token saving calls to all API request execution methods after successful refresh
- Improved error handling for token refresh operations across all API methods
- Consolidated token refresh and save logic for better reliability

## [0.2.11] - 2026-01-24

### Fixed
- Fixed token refresh mechanism to save refreshed tokens to keyring immediately after refresh
- Ensured subsequent commands use fresh tokens instead of expired ones after automatic refresh
- Resolved compilation warnings related to unused functions and imports
- Fixed AssemblyTree OutputFormatter implementation to properly handle tree format output

### Changed
- Moved token saving logic to refresh_token method for immediate persistence of new tokens
- Added token saving calls to all API request methods after successful refresh
- Improved error handling for token refresh operations

### Added
- Added 'pcli2 auth expiration' command to check access token validity and show expiration time in local time zone
- Implemented JWT decoding functionality to extract expiration claims from access tokens
- Added human-readable time remaining display (e.g., "59m 14s") for token expiration

### Fixed
- Fixed asset dependencies command to properly handle assets with no dependencies (was showing authentication error)
- Fixed token refresh mechanism to skip failing refresh attempts and guide users to re-authenticate properly
- Improved error handling for 404 responses when assets have no dependencies
- Added proper handling of NotFoundError in API response processing

### Changed
- Modified refresh_token method to attempt automatic re-authentication using cached credentials
- Updated HTTP error handling to distinguish between authentication issues and missing resources
- Enhanced AssemblyTree and AssemblyNode formatting with proper tree characters for better visualization
- Improved tree format output using proper Unicode box-drawing characters (├──, └──, │)
- Modified 'pcli2 auth expiration' command to display expiration time in user's local time zone instead of UTC

## [0.2.9] - 2026-01-20

### Fixed
- Fixed creation of folders under root
- Fixed folder move operations to properly support moving to root level
- Improved path normalization for folder operations

## [0.2.8] - 2026-01-19

### Fixed
- Fixed AssetStateCounts deserialization to match actual API response format
- Made all AssetStateCounts fields optional to handle missing values gracefully
- Renamed AssetStateCounts fields to match API response (indexing, finished, failed, unsupported, no-3d-data)
- Updated CSV headers to match JSON field names for consistency in tenant state command
- Updated documentation for tenant state command with detailed usage examples

### Added
- Added comprehensive documentation for tenant state command in README.md
- Added detailed descriptions for each asset state field (indexing, finished, failed, unsupported, no-3d-data)

## [0.2.7] - 2026-01-17

### Added
- Added resolve-folder command to resolve a folder path to its UUID
- Added comprehensive test suite for path normalization with edge cases
- Added assets_count and folders_count fields to Folder model for richer folder information
- Added 'issues/' directory to .gitignore
- Added completions command to generate shell completions for bash, zsh, fish, powershell, and elvish
- Added documentation for shell completions setup

### Changed
- Improved normalize_path function to handle consecutive slashes and collapse them into single slashes
- Modified folder list command to show only direct children for non-tree formats (previously showed all descendants)
- Enhanced error handling in API client to extract meaningful error messages from response bodies
- Updated folder move command to properly handle root path cases
- Modified list_folders to use get_children_by_path for non-tree formats
- Enhanced CSV output for folders to include PATH, ASSETS_COUNT, and FOLDERS_COUNT fields
- Improved documentation for folder rename and move commands with comprehensive examples
- Updated README.md and quickstart guide to reflect changes in folder command behavior
- Made -p a consistent short form for --folder-path parameter across all commands
- Improved error messages to provide clearer guidance when no tenant is selected
- Updated environment switching message to be more informative about tenant selection
- Fixed tenant cache to be environment-specific, resolving issues when switching environments
- Added tenant state command to get asset state counts (processing, ready, failed, deleted)
- Added AssetStateCounts model to represent asset state counts
- Implemented API call to retrieve asset state counts from the API

## [0.2.6] - 2026-01-16

### Added
- Added visual-match and visual-match-folder commands for finding visually similar assets
- Added comprehensive documentation for visual matching functionality
- Added concurrent processing option for visual-match-folder with configurable limits (1-10)
- Added progress tracking with multi-progress bars for visual-match-folder operations
- Added exclusive flag for visual-match-folder to limit matches to specified folders only
- Added metadata support for visual matching with REF_ and CAND_ prefixes in CSV output

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

[Unreleased]: https://github.com/physna/pcli2/compare/v0.2.18...HEAD
[0.2.19]: https://github.com/physna/pcli2/compare/v0.2.18...v0.2.19
[0.2.18]: https://github.com/physna/pcli2/compare/v0.2.17...v0.2.18
[0.2.17]: https://github.com/physna/pcli2/compare/v0.2.16...v0.2.17
[0.2.16]: https://github.com/physna/pcli2/compare/v0.2.15...v0.2.16
[0.2.15]: https://github.com/physna/pcli2/compare/v0.2.14...v0.2.15
[0.2.14]: https://github.com/physna/pcli2/compare/v0.2.13...v0.2.14
[0.2.13]: https://github.com/physna/pcli2/compare/v0.2.12...v0.2.13
[0.2.12]: https://github.com/physna/pcli2/compare/v0.2.11...v0.2.12
[0.2.11]: https://github.com/physna/pcli2/compare/v0.2.9...v0.2.11
[0.2.9]: https://github.com/physna/pcli2/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/physna/pcli2/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/physna/pcli2/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/physna/pcli2/compare/v0.2.5...v0.2.6
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

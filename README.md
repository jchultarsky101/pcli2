# PCLI2 - Physna Command Line Interface v2

**Documentation**: [https://jchultarsky101.github.io/pcli2/](https://jchultarsky101.github.io/pcli2/)

The goal of this project is to create version 2 of the Physna Command Line Interface client (PCLI2).

Based on lessons learned from the previous version, we have developed a new and more ergonomic interface. It operates more like Git's excellent CLI, utilizing nested sub-commands, sensible defaults, and configuration.

## Features

- **Intuitive command structure** with nested sub-commands
- **Configuration management** for persistent settings
- **Asset operations** (create, list, get, delete, update)
- **Folder operations** (create, list, get, delete, update)
- **Tenant management** with multi-tenant support
- **Authentication** with OAuth2 client credentials flow
- **Batch operations** for processing multiple assets
- **Geometric matching** for finding similar assets
- **Export/Import** functionality for data migration
- **Context management** for working with multiple tenants
- **Cross-platform support** with environment variable configuration

## Getting Started

### Prerequisites

Before using PCLI2, you will need:
- Your Physna tenant's API client credentials (client ID and client secret)
- A system with Rust installed (for building from source) or the pre-built binary

### Initial Setup

1. **Get Your API Credentials**
   - Log in to your Physna account
   - Navigate to Settings → API Keys
   - Create a new API key pair or use an existing one
   - Note down your Client ID and Client Secret

2. **Installation**
   
   For detailed installation instructions, please refer to the [Installation Guide](https://jchultarsky101.github.io/pcli2/book/installation.html) in our documentation website.
   
   ```bash
   # Clone the repository
   git clone https://github.com/physna/pcli2.git
   cd pcli2
   
   # Build the project
   cargo build --release
   
   # The executable will be located at target/release/pcli2
   ```

3. **Authentication Setup**
   
   Authenticate with your Physna tenant:
   
   ```bash
   # Login with client credentials
   pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET
   ```
   
   You'll only need to do this once per session. The credentials are securely stored using your system's keychain.

4. **Verify Setup**
   
   Test that everything is working:
   
   ```bash
   # Verify authentication
   pcli2 auth get
   
   # List available tenants to confirm connection
   pcli2 tenant list --format json
   
   # Verify access to your tenant
   pcli2 folder list --format tree
   ```

### Basic Commands to Try

Once authenticated, try these basic commands to verify functionality:

```bash
# List all folders in your tenant (tree view)
pcli2 folder list --format tree

# List assets in the root folder
pcli2 asset list

# Upload a test file (replace with your own file)
pcli2 asset create --file /path/to/test/file.stl --path /Root/Test/

# Get details about your test asset
pcli2 asset get --path /Root/Test/file.stl

# Find geometrically similar assets (if you have multiple assets)
pcli2 asset geometric-match --path /Root/Test/file.stl --threshold 80.0
```

### Configuration Management

PCLI2 automatically manages configuration for you, but you can view and manage it:

```bash
# View current configuration
pcli2 config get

# View configuration file path
pcli2 config get path

# Set active tenant context
pcli2 context set tenant
```

### Data Storage Locations

PCLI2 stores data in platform-specific standard directories to ensure proper cross-platform compatibility:

#### Configuration Directory
- **Windows**: `%APPDATA%\pcli2\config.yml`
- **macOS**: `~/Library/Application Support/pcli2/config.yml`
- **Linux**: `~/.config/pcli2/config.yml`

#### Cache Directory
- **Windows**: `%LOCALAPPDATA%\pcli2\cache\`
- **macOS**: `~/Library/Caches/pcli2/cache/`
- **Linux**: `~/.cache/pcli2/cache/`

#### Data Directory
- **Windows**: `%LOCALAPPDATA%\pcli2\data\`
- **macOS**: `~/Library/Application Support/pcli2/data/`
- **Linux**: `~/.local/share/pcli2/data/`

#### Environment Variables

You can override these default locations using environment variables:

```bash
# Override configuration directory
export PCLI2_CONFIG_DIR="/custom/path/to/config"

# Override cache directory
export PCLI2_CACHE_DIR="/custom/path/to/cache"

# Override data directory
export PCLI2_DATA_DIR="/custom/path/to/data"
```

#### Cleaning Up

To completely remove PCLI2 data:

1. **Delete configuration file**:
   ```bash
   # Find configuration file location
   pcli2 config get path
   
   # Delete the file and directory
   rm -rf "$(dirname "$(pcli2 config get path)")"
   ```

2. **Delete cache directory**:
   ```bash
   # Find cache directory location (check config)
   pcli2 config get | grep cache_path
   
   # Delete cache directory
   rm -rf "/path/to/cache/directory/from/config"
   ```

3. **Manual cleanup** (if directories were created outside standard locations):
   ```bash
   # Check for any PCLI2 directories
   find ~ -type d -name "*pcli2*" 2>/dev/null
   
   # Remove any found directories
   rm -rf ~/pcli2.cache
   rm -rf ~/.pcli2
   ```

PCLI2 follows standard platform conventions for data storage, so all data can be easily located and removed using standard file system operations.

### Working with Verbose Output

For debugging purposes, you can enable verbose output:

```bash
# Enable verbose output for debugging
pcli2 --verbose asset list

# Or using the short form
pcli2 -v asset list
```

### Next Steps

Now that you're set up:

1. **Explore your data**: Use `pcli2 folder list --format tree` to explore your folder structure
2. **Upload assets**: Use `pcli2 asset create --file path/to/file --path /Destination/Folder/` to upload files
3. **Find similar items**: Use geometric matching to discover duplicates or similar assets
4. **Automate**: Combine PCLI2 commands in scripts for batch processing

## Installation Guide

For detailed installation instructions, please refer to the [Installation Guide](https://jchultarsky101.github.io/pcli2/book/installation.html) in our documentation website.

## Quick Start Guide

This guide will help you get started with PCLI2 quickly by walking through common tasks.

### Authentication

Before using most PCLI2 commands, you need to authenticate with your Physna tenant:

```bash
# Login with client credentials
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# Verify authentication
pcli2 auth get
```

### Basic Navigation

Learn to navigate your Physna tenant using PCLI2:

```bash
# List available tenants
pcli2 tenant list

# List folders in the root directory
pcli2 folder list

# List assets in a specific folder
pcli2 asset list --path /Root/MyFolder/
```

### Working with Assets

Common asset operations you'll perform regularly:

```bash
# Upload a single asset
pcli2 asset create --file path/to/my/model.stl --path /Root/MyFolder/

# View asset details
pcli2 asset get --path /Root/MyFolder/model.stl

# Delete an asset
pcli2 asset delete --path /Root/MyFolder/model.stl

# Upload multiple assets
pcli2 asset create-batch --files "models/*.stl" --path /Root/BatchUpload/
```

### Geometric Matching

Find similar assets using PCLI2's powerful geometric matching:

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0

# Find matches for all assets in a folder (parallel processing)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 90.0 --format csv --progress
```

### Configuration

Manage your PCLI2 configuration:

```bash
# View current configuration
pcli2 config show

# Set default tenant
pcli2 config set tenant.default YOUR_TENANT_ID

# Export configuration for backup
pcli2 config export --output my-config.yaml
```

### Context Management

Work with multiple tenants efficiently:

```bash
# Set active context (tenant)
pcli2 context set --tenant YOUR_TENANT_ID

# View current context
pcli2 context get

# Clear active context
pcli2 context clear tenant
```

### Getting Help

For help with any command, use the built-in help system:

```bash
# General help
pcli2 --help

# Help for a specific command group
pcli2 asset --help

# Help for a specific command
pcli2 asset create --help
```

You can also use the `-h` or `--help` flag with any command to get detailed usage information.

## Geometric Matching

PCLI2 provides powerful geometric matching capabilities to find similar assets in your Physna tenant. This feature leverages advanced algorithms to identify assets with similar geometries, regardless of their orientation, scale, or position.

### Overview

Geometric matching helps you:

- Find duplicate or near-duplicate assets
- Identify variations of the same part
- Locate similar components across different projects
- Reduce storage costs by identifying redundant assets
- Improve design workflows by finding existing similar parts

### Single Asset Matching

Find geometrically similar assets for a specific reference asset.

#### Basic Usage

```bash
# Find matches for a specific asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 80.0

# Using asset UUID instead of path
pcli2 asset geometric-match --uuid 123e4567-e89b-12d3-a456-426614174000 --threshold 85.0
```

#### Output Formats

##### JSON Format (Default)

```json
[
  {
    "referenceAssetName": "ReferenceModel.stl",
    "candidateAssetName": "SimilarModel.stl",
    "matchPercentage": 95.75,
    "referenceAssetPath": "/Root/Folder/ReferenceModel.stl",
    "candidateAssetPath": "/Root/DifferentFolder/SimilarModel.stl",
    "referenceAssetUuid": "123e4567-e89b-12d3-a456-426614174000",
    "candidateAssetUuid": "987fc321-fedc-ba98-7654-43210fedcba9"
  }
]
```

##### CSV Format

```csv
REFERENCE_ASSET_NAME,CANDIDATE_ASSET_NAME,MATCH_PERCENTAGE,REFERENCE_ASSET_PATH,CANDIDATE_ASSET_PATH,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID
ReferenceModel.stl,SimilarModel.stl,95.75,/Root/Folder/ReferenceModel.stl,/Root/DifferentFolder/SimilarModel.stl,123e4567-e89b-12d3-a456-426614174000,987fc321-fedc-ba98-7654-43210fedcba9
```

### Threshold Settings

The threshold parameter controls the minimum similarity percentage required for a match:

- **0.0** - Return all possible matches (may include unrelated assets)
- **50.0** - Very loose matching (many potential matches)
- **80.0** - Default setting (good balance of accuracy and recall)
- **90.0** - Strict matching (high confidence matches)
- **95.0+** - Very strict matching (near duplicates only)

### Folder-Based Matching

Find geometrically similar assets for all assets in a specified folder. This command processes assets in parallel for improved performance.

#### Basic Usage

```bash
# Find matches for all assets in a folder
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 85.0
```

#### Performance Options

##### Concurrency Control

Control how many simultaneous operations are performed:

```bash
# Process 5 assets simultaneously (default)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --concurrent 5

# Process 10 assets simultaneously for faster results
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --concurrent 10

# Process 1 asset at a time for slower but more stable results
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --concurrent 1
```

##### Progress Monitoring

Monitor the progress of long-running folder-based operations:

```bash
# Show progress bar during folder matching
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --progress
```

### Output Formatting

The geometric matching commands support multiple output formats for integration with other tools:

#### JSON Format (Default)

JSON provides structured output that's easy to parse programmatically:

```bash
# JSON is the default format
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0

# Explicitly specify JSON format
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 --format json
```

#### CSV Format

CSV is ideal for importing results into spreadsheets or databases:

```bash
# Output in CSV format
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 --format csv
```

#### Advanced Usage

##### Filtering Results

Focus on the most relevant matches by adjusting thresholds and filtering:

```bash
# Find only very high-confidence matches (95%+)
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 95.0

# Find loose matches for broader similarity searches
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 50.0
```

##### Combining with Other Commands

Chain commands together for powerful workflows:

```bash
# Find matches and save to a file
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 --format csv > matches.csv

# Find matches and filter with grep
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 | grep "HighConfidencePart"
```

### Best Practices

1. **Start with moderate thresholds** (80-85%) for balanced results
2. **Use folder-based matching for bulk operations** to leverage parallel processing
3. **Monitor progress** for long-running operations using the `--progress` flag
4. **Adjust concurrency** based on your system's capabilities and API rate limits
5. **Save results to files** when performing extensive matching operations
6. **Use appropriate output formats** for your intended use case (JSON for scripting, CSV for spreadsheets)

### Troubleshooting

#### Common Issues

1. **API Rate Limiting**: Reduce concurrency if you encounter rate limiting errors
2. **Large Folder Processing**: Break large folders into smaller batches for better performance
3. **Timeout Errors**: Use lower thresholds to reduce processing time per match
4. **Memory Issues**: Reduce concurrency for systems with limited RAM

#### Error Messages

- **"Threshold must be between 0.00 and 100.00"**: Adjust threshold to a value between 0 and 100
- **"Asset not found"**: Verify the asset path exists in your tenant
- **"API rate limit exceeded"**: Reduce concurrency or wait before retrying
- **"Connection timeout"**: Check your network connection or reduce threshold values

#### Getting Help

If you encounter persistent issues with geometric matching:

1. **Check your API credentials** are still valid
2. **Verify your tenant has geometric matching enabled**
3. **Review the error messages** for specific troubleshooting guidance
4. **Consult the GitHub Issues** page for known issues and solutions
5. **Contact Physna support** for assistance with tenant-specific configuration

## Commands

The application uses a hierarchy of commands:

```
pcli2
├── asset
│   ├── create              Create a new asset by uploading a file
│   ├── create-batch        Create multiple assets by uploading files matching a glob pattern
│   ├── create-metadata-batch  Create metadata for multiple assets from a CSV file
│   ├── list                List all assets in a folder
│   ├── get                 Get asset details
│   ├── update              Update an asset's metadata
│   ├── delete              Delete an asset
│   ├── geometric-match     Find geometrically similar assets for a single asset
│   └── geometric-match-folder  Find geometrically similar assets for all assets in a folder
├── folder
│   ├── create           Create a new folder
│   ├── list             List all folders in a parent folder
│   ├── get              Get folder details
│   ├── update           Update folder details
│   └── delete           Delete a folder
├── tenant
│   ├── create           Create a new tenant (not supported via API)
│   ├── list             List all available tenants
│   ├── get              Get specific tenant details
│   ├── update           Update tenant configuration (not supported via API)
│   └── delete           Delete a tenant (not supported via API)
├── auth
│   ├── login            Authenticate with client credentials
│   ├── logout           Clear authentication tokens
│   └── get              Get current authentication status
├── config
│   ├── get              Get configuration details
│   ├── list             List configuration
│   ├── export           Export configuration to a file
│   └── import           Import configuration from a file
├── context
│   ├── set              Set active context (tenant)
│   ├── get              Get current context
│   └── clear            Clear active context
└── help                 Show help information
```

## Usage Examples

### Authentication
```bash
# Login with client credentials
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# Verify authentication
pcli2 auth get --format json

# Logout
pcli2 auth logout
```

### Tenant Management
```bash
# List available tenants
pcli2 tenant list --format json

# Get details for a specific tenant
pcli2 tenant get --id YOUR_TENANT_ID --format csv
```

### Folder Management
```bash
# List all folders in the default tenant
pcli2 folder list --format tree

# List folders in a specific tenant
pcli2 folder list --tenant YOUR_TENANT_ID --format json

# List subfolders under a specific path
pcli2 folder list --path /Root/MyFolder --format csv

# Create a new folder
pcli2 folder create --name "New Folder" --tenant YOUR_TENANT_ID

# Create a subfolder
pcli2 folder create --name "Sub Folder" --parent-folder-id PARENT_FOLDER_UUID

# Create a subfolder using path
pcli2 folder create --name "Sub Folder" --path "/Root/Parent"

# Get folder details
pcli2 folder get --uuid FOLDER_UUID --format json

# Delete a folder
pcli2 folder delete --path "/Root/FolderToDelete"
```

### Asset Management
```bash
# List all assets in a folder
pcli2 asset list --path "/Root/MyFolder" --format json

# Upload a single asset
pcli2 asset create --file /path/to/model.stl --path "/Root/Uploads" --format json

# Upload multiple assets using glob pattern
pcli2 asset create-batch --files "models/*.stl" --path "/Root/BatchUpload" --concurrent 10 --progress

# Get asset details by path
pcli2 asset get --path "/Root/MyFolder/model.stl" --format json

# Get asset details by UUID
pcli2 asset get --uuid ASSET_UUID --format csv

# Update asset metadata
pcli2 asset update --path "/Root/MyFolder/model.stl" --name "New Name" --format json

# Delete an asset
pcli2 asset delete --path "/Root/MyFolder/model.stl"

# Delete an asset by UUID
pcli2 asset delete --uuid ASSET_UUID
```

### Geometric Matching
```bash
# Find similar assets for a single reference asset
pcli2 asset geometric-match --path "/Root/Reference/model.stl" --threshold 85.0 --format json

# Find similar assets by UUID with different threshold
pcli2 asset geometric-match --uuid ASSET_UUID --threshold 90.0 --format csv

# Find similar assets for all assets in a folder with progress tracking
pcli2 asset geometric-match-folder --path "/Root/SearchFolder/" --threshold 80.0 --concurrent 8 --progress --format json
```

### Context Management
```bash
# Set active tenant context interactively
pcli2 context set tenant

# Set active tenant context explicitly
pcli2 context set tenant --name "My Tenant"

# Get current context
pcli2 context get --format json

# Clear active context
pcli2 context clear tenant
```

### Configuration Management
```bash
# Show current configuration
pcli2 config get --format json

# Show configuration file path
pcli2 config get path

# Export configuration
pcli2 config export --output config.yaml

# Import configuration
pcli2 config import --input config.yaml
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt

# Check for linting issues
cargo clippy
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a pull request

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Support

For support, please contact the Physna development team or open an issue on GitHub.
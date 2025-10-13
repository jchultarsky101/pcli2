# PCLI2 - Physna Command Line Interface v2

**Documentation**: [https://jchultarsky101.github.io/pcli2/](https://jchultarsky101.github.io/pcli2/)

The goal of this project is to create version 2 of the Physna Command Line Interface client (PCLI2).

Based on lessons learned from the previous version, we have developed a new and more ergonomic interface. It operates more like Git's excellent CLI, utilizing nested sub-commands, sensible defaults, and configuration.

## Table of Contents
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Authentication](#authentication)
- [Basic Usage](#basic-usage)
- [Advanced Usage](#advanced-usage)
- [Commands Reference](#commands-reference)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)
- [Support](#support)

## Features

- **Intuitive command structure** with nested sub-commands
- **Configuration management** for persistent settings
- **Asset operations** (create, list, get, delete, metadata operations)
- **Folder operations** (create, list, get, delete)
- **Tenant management** with multi-tenant support
- **Authentication** with OAuth2 client credentials flow
- **Batch operations** for processing multiple assets
- **Geometric matching** for finding similar assets
- **Export/Import** functionality for data migration
- **Context management** for working with multiple tenants
- **Cross-platform support** with environment variable configuration

## Quick Start

Getting started with PCLI2 is straightforward. Follow these steps:

1. **Get your API credentials** by logging into the [Physna OpenAPI Documentation page](https://app-api.physna.com/v3/docs/) and creating a service account
2. **Install PCLI2** using one of the methods described in [Installation](#installation)
3. **Authenticate** with your Physna tenant:
   ```bash
   pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET
   ```
4. **Verify your setup**:
   ```bash
   pcli2 auth get
   ```
5. **Start using PCLI2** to manage your assets and folders

## Installation

### Prerequisites

Before installing PCLI2, you will need:
- Your Physna tenant's API client credentials (client ID and client secret)
- A compatible operating system (Windows, macOS, or Linux)

### Installation Methods

#### Method 1: Pre-built Installers (Recommended for Most Users)

PCLI2 provides pre-built installers for Windows, macOS, and Linux through GitHub Releases:

1. Visit the [Latest Release](https://github.com/physna/pcli2/releases/latest)
2. Download the appropriate installer for your platform:
   - **Windows**: `pcli2-x86_64-pc-windows-msvc.msi` (Installer)
   - **macOS**: `pcli2-installer.sh` (Universal script)
   - **Linux**: `pcli2-installer.sh` (Universal script)

##### Using the Universal Installer Script (macOS/Linux):
```bash
# Download and run the installer script
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/physna/pcli2/releases/latest/download/pcli2-installer.sh | sh
```

#### Method 2: Manual Installation
```bash
# Extract the archive (example for Linux)
tar -xf pcli2-x86_64-unknown-linux-gnu.tar.xz
sudo cp pcli2 /usr/local/bin/
```

#### Method 3: Building from Source (Advanced Users)

This method gives you the latest development version of PCLI2:

```bash
# Clone the repository
git clone https://github.com/physna/pcli2.git
cd pcli2

# Build the project (this may take a few minutes)
cargo build --release

# The executable will be located at target/release/pcli2
# You can copy it to a directory in your PATH
sudo cp target/release/pcli2 /usr/local/bin/

# Or add the target directory to your PATH in ~/.bashrc or ~/.zshrc
echo 'export PATH="$PATH:/path/to/pcli2/target/release"' >> ~/.bashrc
```

### Verifying the Installation

After installation, verify that PCLI2 is working correctly:

```bash
# Check the version
pcli2 --version
```
If successful, this should print the PCLI2 version.

### Updating PCLI2

To update PCLI2 when using pre-built installers:
```bash
pcli2-update
```

This command should check if a new version is available and automatically install it.

For manual update, simply execute the installer script:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/physna/pcli2/releases/latest/download/pcli2-installer.sh | sh
```

For source builds:
```bash
cd /path/to/pcli2
git pull
cargo build --release
sudo cp target/release/pcli2 /usr/local/bin/
```

### Installation Troubleshooting

1. **Permission denied when copying binary**: Use `sudo` or copy to a directory you own
2. **Command not found**: Ensure the binary directory is in your PATH
3. **Build failures**: Make sure you have the latest stable Rust version

## Authentication

Before using most PCLI2 commands, you need to authenticate with Physna.

### Getting API Credentials

1. Log in to the [Physna OpenAPI Documentation page](https://app-api.physna.com/v3/docs/)
2. Authenticate with your Physna credentials
3. Execute the POST /users/me/service-accounts endpoint
4. Note down your Client ID and Client Secret

### Login

```bash
# Login with client credentials (first time)
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# Login with cached credentials (after first time)
pcli2 auth login

# Verify authentication
pcli2 auth get

# Logout
pcli2 auth logout
```

You'll only need to login once per session, which is valid for several hours. The credentials are securely stored using your system's keychain. PCLI2 will automatically renew your access token if necessary.


## Basic Usage

### Tenants

Physna is a multi-tenant system. Your organization may have multiple instances of Physna, which are called "tenants". Each tenant is separate by default, though cross-tenant queries can be configured.

#### List available tenants
```bash
pcli2 tenant list --format csv
```

#### Select active tenant
```bash
pcli2 context set tenant
```

You can also override the tenant for a command by explicitly specifying it with the `--tenant` argument:
```bash
pcli2 asset list --tenant my_other_tenant_name
```

### Working with Folders

```bash
# List all folders in your tenant (tree view)
pcli2 folder list --format tree

# List folders under the root folder in your active tenant
pcli2 folder list --format json

# List subfolders under a specific path
pcli2 folder list --path /Root/MyFolder --format csv

# Create a subfolder
pcli2 folder create --name "Sub Folder" --parent-folder-id PARENT_FOLDER_UUID

# Create a subfolder using path
pcli2 folder create --name "Sub Folder" --path "/Root/Parent"

# Get folder details
pcli2 folder get --uuid FOLDER_UUID --format json

# Delete a folder
pcli2 folder delete --path "/Root/FolderToDelete"
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

# List all assets in a folder
pcli2 asset list --path "/Root/MyFolder" --format json

# Update/create asset metadata
pcli2 asset create-metadata-batch --csv-file "metadata.csv"

# Upload asset with metadata
pcli2 asset create --file path/to/my/model.stl --path /Root/MyFolder/ --metadata "metadata.json"

# Create metadata for multiple assets from a CSV file
pcli2 asset create-metadata-batch --csv-file "metadata.csv"
```

#### Working with Metadata

Metadata is essential for organizing and searching your assets effectively. PCLI2 supports adding metadata via JSON files:

1. **Create a metadata JSON file**:
   ```json
   {
     "part_number": "ABC123",
     "description": "Sample part description",
     "material": "Aluminum",
     "weight": 1.25,
     "created_by": "engineering-team"
   }
   ```

2. **Apply metadata to an asset**:
   ```bash
   pcli2 asset create --file model.stl --path /Parts/Model --metadata "metadata.json"
   ```

3. **Update/create existing asset metadata**:
   ```bash
   pcli2 asset create-metadata-batch --csv-file "updated_metadata.csv"
   ```

### Geometric Matching

Find similar assets using PCLI2's powerful geometric matching:

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0

# Find matches for all assets in a folder (parallel processing)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 90.0 --format csv --progress
```

## Advanced Usage

### Configuration Management

PCLI2 automatically manages configuration for you, but you can view and modify it:

```bash
# View current configuration
pcli2 config get

# View configuration file path
pcli2 config get path
```

#### Data Storage Locations

PCLI2 stores data in platform-specific standard directories to ensure proper cross-platform compatibility:

##### Configuration Directory
- **Windows**: `%APPDATA%\pcli2\config.yml`
- **macOS**: `~/Library/Application Support/pcli2/config.yml`
- **Linux**: `~/.config/pcli2/config.yml`

##### Cache Directory
- **Windows**: `%LOCALAPPDATA%\pcli2\cache\`
- **macOS**: `~/Library/Caches/pcli2/cache/`
- **Linux**: `~/.cache/pcli2/cache/`

##### Data Directory
- **Windows**: `%LOCALAPPDATA%\pcli2\data\`
- **macOS**: `~/Library/Application Support/pcli2/data/`
- **Linux**: `~/.local/share/pcli2/data/`

##### Environment Variables

You can override these default locations using environment variables:

```bash
# Override configuration directory
export PCLI2_CONFIG_DIR="/custom/path/to/config"

# Override cache directory
export PCLI2_CACHE_DIR="/custom/path/to/cache"

# Override data directory
export PCLI2_DATA_DIR="/custom/path/to/data"
```

### Output Formats

Most commands that produce output can use different data formats.

#### JSON Format (Default)
```bash
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 80.0
```
The above is equivalent to:
```bash
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 80.0 --format json
```

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

#### CSV Format
```bash
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 80.0 --format csv
```

```csv
REFERENCE_ASSET_NAME,CANDIDATE_ASSET_NAME,MATCH_PERCENTAGE,REFERENCE_ASSET_PATH,CANDIDATE_ASSET_PATH,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID
ReferenceModel.stl,SimilarModel.stl,95.75,/Root/Folder/ReferenceModel.stl,/Root/DifferentFolder/SimilarModel.stl,123e4567-e89b-12d3-a456-426614174000,987fc321-fedc-ba98-7654-43210fedcba9
```

#### Tree Format

Only the folder list command supports the tree format. It is used to show the hierarchy of folders.

```bash
pcli2 folder list --format tree
```

### Geometric Matching

PCLI2 provides powerful geometric matching capabilities to find similar assets in your Physna tenant. This feature leverages advanced algorithms to identify assets with similar geometries, regardless of their orientation, scale, or position.

#### Overview

Geometric matching helps you:

- Find duplicate or near-duplicate assets
- Identify variations of the same part
- Locate similar components across different projects
- Reduce storage costs by identifying redundant assets
- Improve design workflows by finding existing similar parts

#### Threshold Settings

The threshold parameter controls the minimum similarity percentage required for a match:

- **0.0** - Return all possible matches (may include unrelated assets)
- **50.0** - Very loose matching (many potential matches)
- **80.0** - Default setting (good balance of accuracy and recall)
- **90.0** - Strict matching (high confidence matches)
- **95.0+** - Very strict matching (near duplicates only)

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

#### Combining with Other Commands

Chain commands together for powerful workflows:

```bash
# Find matches and save to a file
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 --format csv > matches.csv

# Find matches and filter with grep
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 | grep "HighConfidencePart"
```

#### Best Practices

The following are just recommendations. You can use any threshold value you would like between 0%-100%:

1. **Start with moderate thresholds** (80-85%) for balanced results
2. **Use folder-based matching for bulk operations** to leverage parallel processing
3. **Monitor progress** for long-running operations using the `--progress` flag
4. **Adjust concurrency** based on your system's capabilities and API rate limits
5. **Save results to files** when performing extensive matching operations
6. **Use appropriate output formats** for your intended use case (JSON for scripting, CSV for spreadsheets)

### Command Aliases and Short Argument Names

Some commands have shorter aliases. For example `list` has an alias of `ls`. Similarly, some command arguments have short names too. For example `--path` can be provided as `-p`. See the help for details.

### Working with Verbose Output

For debugging purposes, you can enable verbose output:

```bash
# Enable verbose output for debugging
pcli2 --verbose asset list

# Or using the short form
pcli2 -v asset list
```

### Cleaning Up

To completely remove PCLI2 data:

#### Delete configuration file
```bash
# Find configuration file location
pcli2 config get path

# Delete the file and directory
rm -rf "$(dirname "$(pcli2 config get path)")"
```

#### Delete cache directory
```bash
# Find cache directory location (check config)
pcli2 config get | grep cache_path

# Delete cache directory
rm -rf "/path/to/cache/directory/from/config"
```

#### Manual cleanup (if directories were created outside standard locations)
```bash
# Check for any PCLI2 directories
find ~ -type d -name "*pcli2*" 2>/dev/null

# Remove any found directories
rm -rf ~/pcli2.cache
rm -rf ~/.pcli2
```

PCLI2 follows standard platform conventions for data storage, so all data can be easily located and removed using standard file system operations.

## Commands Reference

The application uses a hierarchy of commands:

```
pcli2
├── asset
│   ├── create              Create a new asset by uploading a file
│   ├── create-batch        Create multiple assets by uploading files matching a glob pattern
│   ├── create-metadata-batch  Create metadata for multiple assets from a CSV file
│   ├── list                List all assets in a folder
│   ├── get                 Get asset details
│   ├── delete              Delete an asset
│   ├── geometric-match     Find geometrically similar assets for a single asset
│   └── geometric-match-folder  Find geometrically similar assets for all assets in a folder
├── folder
│   ├── create           Create a new folder
│   ├── list             List all folders in a parent folder
│   ├── get              Get folder details
│   └── delete           Delete a folder
├── tenant
│   ├── create           Create a new tenant (not supported via API)
│   ├── list             List all available tenants
│   ├── get              Get specific tenant details
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

## Troubleshooting

### Common Issues

1. **API Rate Limiting**: Reduce concurrency if you encounter rate limiting errors
2. **Large Folder Processing**: Break large folders into smaller batches for better performance
3. **Timeout Errors**: Use lower thresholds to reduce processing time per match
4. **Memory Issues**: Reduce concurrency for systems with limited RAM
5. **Authentication Expired**: Run `pcli2 auth login` to refresh your credentials
6. **Path Not Found**: Verify that folder paths exist and you have permissions to access them
7. **File Upload Failures**: Check that the file exists and is less than the maximum allowed size
8. **Network Errors**: Verify your internet connection and try again

### Error Messages

- **"Threshold must be between 0.00 and 100.00"**: Adjust threshold to a value between 0 and 100
- **"Asset not found"**: Verify the asset path exists in your tenant
- **"API rate limit exceeded"**: Reduce concurrency or wait before retrying
- **"Connection timeout"**: Check your network connection or reduce threshold values
- **"Permission denied"**: Ensure you have the required permissions for the operation
- **"Invalid file format"**: Ensure the file format is supported by Physna
- **"Configuration file not found"**: Run `pcli2 config get path` to check config location
- **"Tenant not accessible"**: Verify the tenant name and your access permissions

### Debugging Tips

If you encounter unexpected behavior:

1. **Enable verbose output**: Add the `--verbose` or `-v` flag to your commands to see detailed logs:
   ```bash
   pcli2 --verbose asset list
   ```

2. **Check your authentication status**:
   ```bash
   pcli2 auth get
   ```

3. **Verify your current context**:
   ```bash
   pcli2 context get
   ```

4. **Review the configuration**:
   ```bash
   pcli2 config get
   ```

### Getting Help

If you continue to experience issues:

1. Check the [GitHub Issues](https://github.com/physna/pcli2/issues) page for known issues
2. Search for similar problems in the issue tracker
3. If you can't find a solution, create a new issue with detailed information:
   - Your OS and PCLI2 version
   - The command you're trying to execute
   - The error message received
   - Steps to reproduce the issue
   - Verbose output if relevant

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a pull request

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Support

If you encounter issues consult the [GitHub Issues](https://github.com/physna/pcli2/issues) page and submit an issue

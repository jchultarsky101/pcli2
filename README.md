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
- **Asset operations** (create, list, get, delete, dependencies, metadata operations)
- **Folder operations** (create, list, get, delete)
- **Tenant management** with multi-tenant support
- **Authentication** with OAuth2 client credentials flow
- **Batch operations** for processing multiple assets
- **Geometric matching** for finding similar assets
- **Metadata inference** for automatically propagating metadata to similar assets (`pcli2 asset metadata inference`)
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

1. Visit the [Latest Release](https://github.com/jchultarsky101/pcli2/releases/latest)
2. Download the appropriate installer for your platform:
   - **Windows**: `pcli2-x86_64-pc-windows-msvc.msi` (Installer)
   - **macOS**: `pcli2-installer.sh` (Universal script)
   - **Linux**: `pcli2-installer.sh` (Universal script)

##### Using the Universal Installer Script (macOS/Linux):

```bash
# Download and run the installer script
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
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
git clone https://github.com/jchultarsky101/pcli2.git
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
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
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

There are two methods to obtain your API credentials:

#### Method 1: Using the Physna Web Interface (Recommended)

This is the newer, more user-friendly approach available to administrators:

1. Log in to your Physna instance
2. (Optional) Select a tenant from the tenant selector
3. Click on **Settings** (the gear icon in the top-right corner)
4. Navigate to the **Users** tab
5. Create a new service account
6. Record the **Client ID** and **Client Secret** for use with PCLI2

#### Method 2: Using the API Documentation Page (Legacy)

This is the original approach using the API documentation interface:

1. Navigate to the [Physna OpenAPI Documentation page](https://app-api.physna.com/v3/docs/)
2. Log in with your Physna credentials
3. Locate and execute the `POST /users/me/service-accounts` endpoint
4. Record the **Client ID** and **Client Secret** from the response

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

Physna operates on a multi-tenant architecture, meaning your organization may have multiple separate Physna instances called "tenants". Each tenant is isolated by default, though cross-tenant queries can be configured by administrators.

#### Managing Tenants

Use the tenant command to list and view available tenants in your organization:

```bash
pcli2 tenant list --format csv
```

#### Setting an Active Tenant

To avoid specifying a tenant for every command, you can set an active tenant using the context command:

```bash
pcli2 context set tenant
```

This command will prompt you to select a tenant from your available options.

#### Overriding Tenant Selection

For specific commands, you can override the active tenant by explicitly specifying it with the `--tenant` argument. You can use either the tenant name or tenant ID:

```bash
# Using tenant name
pcli2 asset list --tenant "Demo Environment 1"

# Using tenant ID (UUID)
pcli2 asset list --tenant 123e4567-e89b-12d3-a456-426614174000
```

The tenant parameter accepts either the human-readable tenant name or the unique identifier (UUID) for precise targeting.

### Working with Folders

Folder management is essential for organizing your assets in Physna. These commands allow you to create, view, and manage the folder structure where your assets are stored.

#### Listing Folders

Use the folder list command to view your folder structure in various formats:

```bash
# List all folders in your tenant (tree view)
pcli2 folder list --format tree

# List folders under the root folder in your active tenant
pcli2 folder list --format json

# List subfolders under a specific path
pcli2 folder list --path /Root/MyFolder --format csv
```

#### Creating Folders

You can create new folders in two ways: by specifying a parent folder ID or by using a path:

```bash
# Create a subfolder
pcli2 folder create --name "Sub Folder" --parent-folder-id PARENT_FOLDER_UUID

# Create a subfolder using path
pcli2 folder create --name "Sub Folder" --path "/Root/Parent"
```

#### Viewing and Managing Folders

Get detailed information about specific folders or remove them when no longer needed:

```bash
# Get folder details
pcli2 folder get --uuid FOLDER_UUID --format json

# Delete a folder (only removes the folder, not its contents)
pcli2 folder delete --path "/Root/FolderToDelete"

# Delete a folder and all its contents (assets and subfolders)
pcli2 folder delete --path "/Root/FolderToDelete" --force
```

**Note**: By default, deleting a folder only removes the folder itself. To also delete all assets and subfolders within it, use the `--force` flag. This action cannot be undone.

### Working with Assets

Asset management is a core function of PCLI2. These commands allow you to upload, retrieve, organize, and maintain your 3D models and other assets in Physna.

#### Uploading Assets

The `asset create` command uploads individual files to your Physna tenant, placing them in the specified folder path. This is useful for adding single assets to your collection:

```bash
# Upload a single asset
pcli2 asset create --file path/to/my/model.stl --path /Root/MyFolder/
```

For bulk operations, `asset create-batch` allows you to upload multiple files at once using glob patterns:

```bash
# Upload multiple assets
pcli2 asset create-batch --files "models/*.stl" --path /Root/BatchUpload/
```

#### Viewing and Managing Assets

Use these commands to inspect and manage your assets:

```bash
# View asset details
pcli2 asset get --path /Root/MyFolder/model.stl

# List all assets in a folder
pcli2 asset list --path "/Root/MyFolder" --format json
# Create metadata for multiple assets from a CSV file
pcli2 asset metadata create-batch --csv-file "metadata.csv"

#### Deleting Assets

When you need to remove assets from your Physna tenant, use the delete command:

```bash
# Delete an asset
pcli2 asset delete --path /Root/MyFolder/model.stl
```

This permanently removes the asset from your tenant and cannot be undone.

### Working with Asset Dependencies

Asset dependencies represent the relationships between assemblies and their components. When you have an assembly (like a CAD model that consists of multiple parts), the dependencies show which parts make up that assembly.

#### Basic Dependency Queries

```bash
# Get direct dependencies for an asset
pcli2 asset dependencies --path "/Root/MyFolder/assembly.stl" --format json

# Get dependencies by UUID (path-based lookup is preferred)
pcli2 asset dependencies --uuid ASSET_UUID --format json
```

#### Recursive Dependency Queries

For complex assemblies with nested subassemblies, you can use the `--recursive` flag to traverse the entire dependency tree:

```bash
# Get all dependencies recursively, showing the full hierarchy
pcli2 asset dependencies --path "/Root/MyFolder/complex_assembly.asm" --recursive --format tree

# Get all dependencies recursively in machine-readable JSON format with parent-child relationships
pcli2 asset dependencies --path "/Root/MyFolder/complex_assembly.asm" --recursive --format json

# Get all dependencies recursively in CSV format with parent-child relationships
pcli2 asset dependencies --path "/Root/MyFolder/complex_assembly.asm" --recursive --format csv
```

The recursive mode preserves parent-child relationships in the output:
- **Tree format**: Shows proper hierarchical indentation structure
- **JSON format**: Includes `parentPath` field to show which asset is the parent of each dependency
- **CSV format**: Includes `PARENT_PATH` column to show parent-child relationships

This allows you to understand the complete assembly structure and perform bill-of-materials analysis.

### Geometric Matching

Geometric matching is a powerful feature that allows you to find assets with similar 3D geometry in your Physna tenant. This is particularly useful for identifying duplicate parts, finding design variations, or discovering similar components across different projects.

#### Single Asset Matching

Use the geometric-match command to find similar assets to a specific reference model:

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0
```

The threshold parameter controls the similarity requirement, where higher values (closer to 100) require closer matches.

#### Folder-Based Matching

For bulk operations, use geometric-match-folder to find similar assets for all models in a folder:

```bash
# Find matches for all assets in a folder (parallel processing)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 90.0 --format csv --progress
```

This command processes all assets in the specified folder simultaneously, making it efficient for large-scale similarity searches. The progress flag provides visual feedback during long-running operations.

## Advanced Usage

### Configuration Management

PCLI2 automatically manages configuration settings for you, storing them in platform-appropriate locations. You can view, export, import, and modify configuration settings as needed:

```bash
# View current configuration
pcli2 config get

# View configuration file path
pcli2 config get path
```

Configuration management allows you to customize PCLI2's behavior and store settings that persist across sessions. The configuration includes settings for API endpoints, authentication, caching, and other operational parameters.

#### Multi-Environment Configuration

PCLI2 supports multiple environment configurations, allowing you to easily switch between different Physna instances (e.g., development, staging, production):

```bash
# Add a new environment configuration
pcli2 config environment add --name "development" \
  --api-url "https://dev-api.physna.com/v3" \
  --ui-url "https://dev.physna.com" \
  --auth-url "https://dev-auth.physna.com/oauth2/token"

# Add a production environment
pcli2 config environment add --name "production" \
  --api-url "https://app-api.physna.com/v3" \
  --ui-url "https://app.physna.com" \
  --auth-url "https://physna-app.auth.us-east-2.amazoncognito.com/oauth2/token"

# List all environments
pcli2 config environment list

# Switch to an environment
pcli2 config environment use development

# Remove an environment
pcli2 config environment remove test-environment
```

Each environment can have its own:
- API base URL (for API calls)
- UI base URL (for comparison viewer links)
- Authentication URL (for OAuth2 token requests)

When no environment is active, PCLI2 uses the default URLs. You can also override environment settings with environment variables:

```bash
# Override URLs with environment variables (highest priority)
export PCLI2_API_BASE_URL="https://custom-api.example.com/v3"
export PCLI2_UI_BASE_URL="https://custom-ui.example.com"
export PCLI2_AUTH_BASE_URL="https://custom-auth.example.com/oauth2/token"
```

The priority order for URL configuration is:
1. Environment variables (highest priority)
2. Active environment configuration
3. Global configuration settings
4. Default hardcoded URLs (lowest priority)

#### Linux Environment Variable Setup

On Linux systems, you can set environment variables in your shell profile to configure PCLI2:

**For Bash** (`~/.bashrc` or `~/.bash_profile`):
```bash
# PCLI2 Environment Configuration
export PCLI2_API_BASE_URL="https://my-custom-api.example.com/v3"
export PCLI2_UI_BASE_URL="https://my-custom-ui.example.com"
export PCLI2_AUTH_BASE_URL="https://my-custom-auth.example.com/oauth2/token"

# Optional: Custom configuration and cache directories
export PCLI2_CONFIG_DIR="/home/$USER/.config/pcli2"
export PCLI2_CACHE_DIR="/home/$USER/.cache/pcli2"
```

**For Zsh** (`~/.zshrc`):
```bash
# PCLI2 Environment Configuration
export PCLI2_API_BASE_URL="https://my-custom-api.example.com/v3"
export PCLI2_UI_BASE_URL="https://my-custom-ui.example.com"
export PCLI2_AUTH_BASE_URL="https://my-custom-auth.example.com/oauth2/token"

# Optional: Custom configuration and cache directories
export PCLI2_CONFIG_DIR="/home/$USER/.config/pcli2"
export PCLI2_CACHE_DIR="/home/$USER/.cache/pcli2"
```

After adding to your profile, reload the shell:
```bash
# For Bash
source ~/.bashrc

# For Zsh
source ~/.zshrc
```

**Example for Different Environments:**

Development Environment:
```bash
export PCLI2_API_BASE_URL="https://dev-api.physna.com/v3"
export PCLI2_UI_BASE_URL="https://dev.physna.com"
export PCLI2_AUTH_BASE_URL="https://dev-auth.physna.com/oauth2/token"
```

Staging Environment:
```bash
export PCLI2_API_BASE_URL="https://staging-api.physna.com/v3"
export PCLI2_UI_BASE_URL="https://staging.physna.com"
export PCLI2_AUTH_BASE_URL="https://staging-auth.physna.com/oauth2/token"
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

The tenant commands do not support tree format as it doesn't make sense for tenant data.

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


### Metadata Inference

Automatically apply metadata from a reference asset to geometrically similar assets using PCLI2's metadata inference capability:

```bash
# Apply specific metadata fields from a reference asset to similar assets
pcli2 asset metadata inference --path /Root/Parts/Bolt-M8x20.stl --name "Material,Cost" --threshold 90.0

# Apply metadata recursively to create chains of similar assets
pcli2 asset metadata inference --path /Root/Parts/Bolt-M8x20.stl --name "Category" --threshold 85.0 --recursive

# Apply multiple metadata fields with different thresholds
pcli2 asset metadata inference --path /Root/Parts/Bolt-M8x20.stl --name "Material" --name "Finish" --name "Supplier" --threshold 80.0
```

The metadata inference command helps you efficiently propagate metadata across geometrically similar assets, reducing manual work and ensuring consistency in your asset database.

```bash
# Find matches and save to a file
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 --format csv > matches.csv

# Find matches and filter with grep
pcli2 asset geometric-match --path /Root/Reference.stl --threshold 80.0 | grep "HighConfidencePart"
```

### Working with Metadata

Metadata is essential for organizing and searching your assets effectively. PCLI2 supports comprehensive metadata operations including creating, retrieving, updating, and deleting asset metadata. Metadata helps you categorize, filter, and find assets based on custom properties like material, supplier, weight, or any other characteristic relevant to your workflow.

#### Metadata Operations

PCLI2 provides several commands for working with asset metadata:

1. **Create/Update Individual Asset Metadata**:

   The `metadata create` command adds or updates a single metadata field on an asset. This is useful for setting specific properties on individual assets:

   ```bash
   # Add or update a single metadata field on an asset
   pcli2 asset metadata create --path "/Root/Folder/Model.stl" --name "Material" --value "Steel" --type "text"

   # Add or update a single metadata field on an asset by UUID
   pcli2 asset metadata create --uuid "123e4567-e89b-12d3-a456-426614174000" --name "Weight" --value "15.5" --type "number"
   ```

2. **Retrieve Asset Metadata**:

   Use the `metadata get` command to view all metadata associated with an asset:

   ```bash
   # Get all metadata for an asset in JSON format (default)
   pcli2 asset metadata get --path "/Root/Folder/Model.stl"

   # Get all metadata for an asset in CSV format (suitable for batch operations)
   pcli2 asset metadata get --uuid "123e4567-e89b-12d3-a456-426614174000" --format csv
   ```

3. **Delete Asset Metadata**:

   The `metadata delete` command removes specific metadata fields from an asset without affecting other metadata on the same asset:

   ```bash
   # Delete specific metadata fields from an asset
   pcli2 asset metadata delete --path "/Root/Folder/Model.stl" --name "Material" --name "Weight"

   # Delete metadata fields using comma-separated list
   pcli2 asset metadata delete --uuid "123e4567-e89b-12d3-a456-426614174000" --name "Material,Weight,Description"
   ```

   The delete command now uses the dedicated API endpoint to properly remove metadata fields from assets, rather than fetching all metadata and re-updating the asset without the specified fields.

4. **Create/Update Metadata for Multiple Assets**:

   For bulk operations, use the `metadata create-batch` command to update multiple assets at once using a CSV file:

   ```bash
   # Create or update metadata for multiple assets from a CSV file
   pcli2 asset metadata create-batch --csv-file "metadata.csv"
   ```

#### CSV Format for Batch Metadata Operations

The CSV format used by `asset metadata get --format csv` and `asset metadata create-batch --csv-file` is designed for seamless round-trip operations:

```csv
ASSET_PATH,NAME,VALUE
/Root/Folder/Model1.stl,Material,Steel
/Root/Folder/Model1.stl,Weight,"15.5 kg"
/Root/Folder/Model2.ipt,Material,Aluminum
/Root/Folder/Model2.ipt,Supplier,Richardson Electronics
```

The CSV format specifications:
- **Header Row**: Must contain exactly `ASSET_PATH,NAME,VALUE` in that order
- **ASSET_PATH**: Full path to the asset in Physna (e.g., `/Root/Folder/Model.stl`)
- **NAME**: Name of the metadata field to set
- **VALUE**: Value to assign to the metadata field
- **File Encoding**: Must be UTF-8 encoded
- **Quoting**: Values containing commas, quotes, or newlines must be enclosed in double quotes
- **Escaping**: Double quotes within values must be escaped by doubling them (e.g., `"15.5"" diameter"`)
- **Empty Rows**: Will be ignored during processing
- **Multiple Fields**: If an asset has multiple metadata fields to update, include multiple rows with the same ASSET_PATH but different NAME and VALUE combinations

**Example Command:**
```bash
# Create/update metadata for multiple assets from a CSV file
pcli2 asset metadata create-batch --csv-file "metadata.csv"
```

**Note:** The `create-batch` command processes each row as a single metadata field assignment for an asset. Multiple rows with the same ASSET_PATH will update multiple metadata fields for that asset in a single API call.

#### Advanced Metadata Workflow: Export, Modify, Reimport

One of the most powerful features of PCLI2 is the ability to export metadata, modify it externally, and reimport it:

1. **Export Metadata**:
   ```bash
   # Export all metadata for an asset to a CSV file
   pcli2 asset metadata get --path "/Root/Folder/Model.stl" --format csv > model_metadata.csv
   
   # Export metadata for multiple assets in a folder
   pcli2 asset list --path "/Root/Folder/" --metadata --format csv > folder_metadata.csv
   ```

2. **Modify Metadata Externally**:
   - Open the CSV file in a spreadsheet application (Excel, Google Sheets, etc.)
   - Make the desired changes to metadata values
   - Save the file in CSV format

3. **Reimport Modified Metadata**:
   ```bash
   # Update assets with modified metadata
   pcli2 asset metadata create-batch --csv-file "modified_metadata.csv"
   ```

This workflow enables powerful bulk metadata operations while maintaining the flexibility to use familiar spreadsheet tools for data manipulation.

#### Metadata Field Types

PCLI2 supports three metadata field types:

1. **Text** (default): String values
   ```bash
   pcli2 asset metadata create --path "/Root/Model.stl" --name "Description" --value "Sample part description" --type "text"
   ```

2. **Number**: Numeric values
   ```bash
   pcli2 asset metadata create --path "/Root/Model.stl" --name "Weight" --value "15.5" --type "number"
   ```

3. **Boolean**: True/False values
   ```bash
   pcli2 asset metadata create --path "/Root/Model.stl" --name "Approved" --value "true" --type "boolean"
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
│   ├── dependencies        Get dependencies for an asset (components in assemblies, referenced assets) with --recursive flag for full hierarchy (alias: dep)
│   ├── list                List all assets in a folder
│   ├── get                 Get asset details
│   ├── delete              Delete an asset
│   ├── geometric-match     Find geometrically similar assets for a single asset
│   ├── geometric-match-folder  Find geometrically similar assets for all assets in a folder
│   └── metadata
│       ├── get             Get metadata for an asset
│       ├── create          Add metadata to an asset
│       ├── delete          Delete specific metadata fields from an asset
│       ├── create-batch    Create metadata for multiple assets from a CSV file
│       └── inference       Apply metadata from a reference asset to geometrically similar assets
├── folder
│   ├── create           Create a new folder
│   ├── list             List all folders in a parent folder
│   ├── get              Get folder details
│   └── delete           Delete a folder
├── tenant
│   ├── list             List all available tenants
│   └── get              Get specific tenant details
├── auth
│   ├── login            Authenticate with client credentials
│   ├── logout           Clear authentication tokens
│   └── get              Get current authentication status
├── config
│   ├── get              Get configuration details
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

1. **Check your authentication status**:
   ```bash
   pcli2 auth get
   ```

2. **Verify your current context**:
   ```bash
   pcli2 context get
   ```

3. **Review the configuration**:
   ```bash
   pcli2 config get
   ```

### Getting Help

If you continue to experience issues:

1. Check the [GitHub Issues](https://github.com/jchultarsky101/pcli2/issues) page for known issues
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

If you encounter issues consult the [GitHub Issues](https://github.com/jchultarsky101/pcli2/issues) page and submit an issue

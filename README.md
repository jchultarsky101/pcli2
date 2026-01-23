# PCLI2 - Physna Command Line Interface v2

**Documentation**: [https://jchultarsky101.github.io/pcli2/](https://jchultarsky101.github.io/pcli2/)

The goal of this project is to create version 2 of the Physna Command Line Interface client (PCLI2).

Current Version: 0.2.11 <!-- Updated: 2026-01-23T13:30:00Z -->

Based on lessons learned from the previous version, we have developed a new and more ergonomic interface. It operates more like Git's excellent CLI, utilizing nested sub-commands, sensible defaults, and configuration.

## Table of Contents
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Authentication](#authentication)
- [Basic Usage](#basic-usage)
  - [Tenants](#tenants)
  - [Working with Folders](#working-with-folders)
  - [Working with Assets](#working-with-assets)
  - [Working with Asset Dependencies](#working-with-asset-dependencies)
  - [Geometric Matching](#geometric-matching)
  - [Part Matching](#part-matching)
  - [Visual Matching](#visual-matching)
  - [Working with Metadata](#working-with-metadata)
    - [Metadata Inference](#metadata-inference)
- [Advanced Usage](#advanced-usage)
  - [Configuration Management](#configuration-management)
  - [Using UNIX Pipes with PCLI2](#using-unix-pipes-with-pcli2)
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

1. **Get your API credentials** by logging into your Physna tenant and creating a service account
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
   - **Windows**:
     - `pcli2-x86_64-pc-windows-msvc.msi` (Installer) - *Does NOT include `pcli2-update` command*
     - `pcli2-installer.ps1` (PowerShell script) - *Includes `pcli2-update` command*
   - **macOS**: `pcli2-installer.sh` (Universal script) - *Includes `pcli2-update` command*
   - **Linux**: `pcli2-installer.sh` (Universal script) - *Includes `pcli2-update` command*

**Important Note for Windows Users**: The Windows MSI installer does not include the `pcli2-update` command. If you want to be able to update PCLI2 using the `pcli2-update` command, use the PowerShell installer script instead of the MSI installer. Alternatively, you can use the universal installer script within WSL (Windows Subsystem for Linux).

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

### First-Time Authorization

When you run PCLI2 for the first time after installation, you may see authorization prompts requesting permission to access your system's secure credential storage (keyring). This is normal and required for secure credential storage:

- **macOS**: A dialog will appear asking you to allow PCLI2 to access the keychain
- **Windows**: You may see a User Account Control (UAC) prompt
- **Linux**: You may be prompted to unlock the keyring

The authorization is typically remembered for subsequent runs.

### Updating PCLI2

The update mechanism depends on how you installed PCLI2:

#### If Installed via Universal Installer Script (macOS/Linux) or Shell/PowerShell Scripts:
```bash
pcli2-update
```

This command checks if a new version is available and automatically installs it.

For macOS/Linux users:
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
```

For Windows users using PowerShell:
```powershell
irm https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.ps1 | iex
```

#### If Installed via Windows MSI Installer:
The Windows MSI installer does **not** include a `pcli2-update` executable. To upgrade to a new version, you must download and run the new version of the MSI installer from the [releases page](https://github.com/jchultarsky101/pcli2/releases/latest).

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

# Check token expiration
pcli2 auth expiration
```

You'll only need to login once per session, which is valid for several hours. The credentials are securely stored using your system's keychain. PCLI2 will automatically renew your access token if necessary.

The `pcli2 auth expiration` command shows the expiration time (in your local time zone) and remaining validity period of your current access token. This helps you understand how much time remains before you need to log in again.

### Security and Authentication

PCLI2 uses your operating system's secure credential storage to protect your API credentials:

#### System Keyring Integration

PCLI2 automatically integrates with your system's secure credential storage:

- **macOS**: Uses Keychain Services for secure credential storage
- **Windows**: Uses Windows Credential Manager
- **Linux**: Uses Secret Service API or KWallet

#### First-Time Authorization

When you run PCLI2 for the first time after installation, you may see authorization prompts requesting permission to access the system keyring:

- **macOS**: A dialog will appear asking you to allow PCLI2 to access the keychain
- **Windows**: You may see a User Account Control (UAC) prompt
- **Linux**: You may be prompted to unlock the keyring

This is normal and required for secure credential storage. The authorization is typically remembered for subsequent runs.

#### Authorization Prompts

- On **macOS**, you may see multiple authorization prompts during the first run as PCLI2 accesses different credential types (access token, client ID, client secret)
- The system will remember your authorization decision for future runs
- If you deny access, authentication commands will fail until you grant permission


## Basic Usage

### Tenants

Physna operates on a multi-tenant architecture, meaning your organization may have multiple separate Physna instances called "tenants". Each tenant is isolated by default, though cross-tenant queries can be configured by administrators.

#### Managing Tenants

Use the tenant command to list and view available tenants in your organization:

```bash
pcli2 tenant list --format csv
```

#### Setting an Active Tenant

To avoid specifying a tenant for every command, you can set an active tenant using the tenant command:

```bash
pcli2 tenant use
```

This command will prompt you to select a tenant from your available options.

#### Getting the Active Tenant

To check which tenant is currently active:

```bash
pcli2 tenant current
```

#### Clearing the Active Tenant

To clear the currently active tenant:

```bash
pcli2 tenant clear
```

#### Overriding Tenant Selection

For specific commands, you can override the active tenant by explicitly specifying it with the `--tenant` argument. You can use either the tenant name or tenant ID:

```bash
# Using tenant name
pcli2 asset list --tenant "Demo Environment 1"

# Using tenant ID (UUID)
pcli2 asset list --tenant 123e4567-e89b-12d3-a456-426614174000
```

The tenant parameter accepts either the human-readable tenant name or the unique identifier (UUID) for precise targeting.

#### Checking Tenant Asset State

Use the tenant state command to get counts of assets in different processing states within your tenant:

```bash
# Get asset state counts in JSON format (default)
pcli2 tenant state

# Get asset state counts in pretty-printed JSON
pcli2 tenant state --pretty

# Get asset state counts in CSV format
pcli2 tenant state --format csv

# Get asset state counts in CSV format with headers
pcli2 tenant state --format csv --headers
```

The command returns counts for the following asset states:
- **indexing**: Assets currently being processed/uploaded
- **finished**: Assets that have completed processing and are ready for use
- **failed**: Assets that failed during processing
- **unsupported**: Assets with file formats not supported by Physna
- **no-3d-data**: Assets that were uploaded but contain no 3D geometry data

All fields are optional and will default to 0 if not present in the API response, ensuring robust handling of varying API responses.

### Working with Folders

Folder management is essential for organizing your assets in Physna. These commands allow you to create, view, and manage the folder structure where your assets are stored.

#### Listing Folders

Use the folder list command to view your folder structure in various formats:

```bash
# List all folders in your tenant (tree view)
pcli2 folder list --format tree

# List folders under the root folder in your active tenant
pcli2 folder list --format json

# List subfolders under a specific path (shows only direct children, not entire subtree)
pcli2 folder list --path /Root/MyFolder --format csv
```

**Important Notes about Folder List**:
- The `--format tree` option displays the complete hierarchical structure of folders
- All other formats (`json`, `csv`) display only the direct children of the specified path, not the entire subtree
- When no path is specified, the command defaults to the root path (`/`)
- The CSV format includes columns for NAME, PATH, ASSETS_COUNT, FOLDERS_COUNT, and UUID

#### Creating Folders

You can create new folders in two ways: by specifying a parent folder ID or by using a path:

```bash
# Create a subfolder
pcli2 folder create --name "Sub Folder" --parent-folder-id PARENT_FOLDER_UUID

# Create a subfolder using path
pcli2 folder create --name "Sub Folder" --path "/Root/Parent"
```

#### Viewing and Managing Folders

Get detailed information about specific folders, rename, move, or remove them when no longer needed:

```bash
# Get folder details
pcli2 folder get --uuid FOLDER_UUID --format json

# Rename a folder
pcli2 folder rename --folder-path "/Root/OldFolderName" --name "NewFolderName"
pcli2 folder rename --folder-uuid FOLDER_UUID --name "NewFolderName"

**Important Notes about Folder Rename**:
- The command supports both path-based and UUID-based identification of the folder to rename
- When using path-based identification, the path is automatically normalized (consecutive slashes collapsed, leading "/HOME" removed)
- The rename operation updates only the folder name, not its path or parent location
- If the rename operation fails, it may be due to permissions, API issues, or invalid folder identifiers

# Move a folder to a new parent folder (between non-root folders)
pcli2 folder move --folder-path "/Root/FolderToMove" --parent-folder-path "/New/Parent/Path"
pcli2 folder move --folder-uuid FOLDER_UUID --parent-folder-uuid PARENT_FOLDER_UUID
# Or using the alias
pcli2 folder mv --folder-path "/Root/FolderToMove" --parent-folder-path "/New/Parent/Path"

# Move a folder to the root level (known issue: may cause folder to disappear temporarily)
# Note: Moving to root level has a known API issue where the folder may not appear correctly
# It's recommended to move to a specific subfolder instead
pcli2 folder move --folder-path "/Some/Subfolder/FolderToMove" --parent-folder-path "/"

# Delete a folder (only works if the folder is empty)
pcli2 folder delete --path "/Root/FolderToDelete"

# Delete a folder and all its contents (assets and subfolders) recursively
pcli2 folder delete --path "/Root/FolderToDelete" --force
```

**Note**: By default, the folder delete command will only work if the folder is empty. If the folder contains any subfolders or assets, the command will throw an error. To delete a folder that is not empty, you need to specify the `--force` option, which will make PCLI2 recursively delete all assets and subfolders before deleting the base folder. This action cannot be undone.

**Important Notes about Folder Move**:
- The `folder move` command works reliably when moving between non-root folders
- Moving folders to the root level (`--parent-folder-path /`) is now properly supported
- When using path-based parameters, the paths are automatically normalized (consecutive slashes collapsed, leading "/HOME" removed)
- The command supports both UUID-based and path-based identification for both the folder to move and the destination parent folder

#### Downloading All Assets in a Folder

The `folder download` command allows you to download all assets from a specified folder and its entire subfolder hierarchy as a single ZIP archive. This is particularly useful for backing up entire folder trees or transferring assets between systems.

```bash
# Download all assets in a folder as a ZIP archive
pcli2 folder download --folder-path "/Root/MyFolder" --output "my_folder.zip"

# Download all assets in a folder with progress indicator
pcli2 folder download --folder-path "/Root/MyFolder" --progress

# Download all assets in a folder with custom output path
pcli2 folder download --folder-path "/Root/MyFolder" --output "./backups/my_backup.zip"

# Download all assets from the root folder (uses tenant name as default filename)
pcli2 folder download --folder-path "/"
```

**Key Features**:
- **Recursive Download**: Downloads assets from the specified folder AND all its subfolders recursively
- **Folder Structure Preservation**: The ZIP file maintains the original folder hierarchy with appropriate subdirectories
- **Progress Indication**: Use `--progress` flag to show download progress
- **Custom Output**: Use `--output` to specify a custom file path and name
- **Default Naming**: When no output path is specified, uses the folder name (or tenant name for root folder) as the default filename

**Important Notes about Folder Download**:
- The command downloads assets from the specified folder and ALL its subfolders recursively
- The folder structure is preserved in the ZIP file with appropriate subdirectories
- For the root folder (`/`), if no output filename is specified, the tenant name is used as the default (e.g., `demo-1.zip`)
- Large folder hierarchies may take considerable time to download depending on the number of assets
- The command creates a temporary directory during the download process which is cleaned up after the ZIP file is created
- Assets in subfolders will be placed in corresponding subdirectories within the ZIP file (e.g., an asset in `/Root/Parent/Child/file.stl` will be placed as `Child/file.stl` in the ZIP)

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

# Download a single asset
pcli2 asset download --path /Root/MyFolder/model.stl

# Download all assets in a folder as a ZIP archive
pcli2 folder download --folder-path "/Root/MyFolder" --output "my_folder.zip"

# Download all assets in a folder with progress indicator
pcli2 folder download --folder-path "/Root/MyFolder" --progress

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

#### Best Practices

The following are just recommendations. You can use any threshold value you would like between 0%-100%:

1. **Start with moderate thresholds** (80-85%) for balanced results
2. **Use folder-based matching for bulk operations** to leverage parallel processing
3. **Monitor progress** for long-running operations using the `--progress` flag
4. **Adjust concurrency** based on your system's capabilities and API rate limits
5. **Save results to files** when performing extensive matching operations
6. **Use appropriate output formats** for your intended use case (JSON for scripting, CSV for spreadsheets)

#### Part Matching

Part matching is a specialized feature that finds parts within assemblies, rather than direct part-to-part comparisons. This is particularly useful for identifying components that make up larger assemblies or finding similar sub-assemblies within complex designs.

Use the part-match command to find parts within assemblies similar to a specific reference model:

```bash
# Find parts within assemblies for a single asset
pcli2 asset part-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0
```

The key difference from geometric matching is that part matching outputs two similarity scores:
- **Forward match percentage**: Measures how well the reference asset matches as a part within the candidate assembly
- **Reverse match percentage**: Measures how well the candidate asset matches as a part within the reference assembly

The threshold parameter controls the minimum similarity requirement for both directions.

#### Part Matching Output

Part matching results include both forward and reverse match percentages to indicate the directional relationship between parts and assemblies:

```bash
# View part matching results in CSV format with headers
pcli2 asset part-match --path /Root/Folder/ReferenceModel.stl --threshold 80 --format csv --headers
```

The CSV output includes columns for both similarity scores:
- `FORWARD_MATCH_PERCENTAGE`: Similarity when reference is considered a part of candidate
- `REVERSE_MATCH_PERCENTAGE`: Similarity when candidate is considered a part of reference

This bidirectional matching approach is ideal for discovering hierarchical relationships between components and assemblies in your design database.

### Visual Matching

Visual matching is a specialized feature that finds assets with similar visual appearance using advanced computer vision algorithms. Unlike geometric matching which focuses on 3D shape similarity, or part matching which identifies hierarchical relationships, visual matching identifies assets that look similar from a visual standpoint.

#### Single Asset Visual Matching

Use the visual-match command to find visually similar assets to a specific reference model:

```bash
# Find visually similar assets for a single asset
pcli2 asset visual-match --path /Root/Folder/ReferenceModel.stl
```

Visual matching results are ordered by relevance as determined by the visual search algorithm. Unlike geometric and part matching, visual matching does not use a threshold parameter and does not provide similarity percentages since the results are ranked by visual similarity rather than a percentage-based comparison.

#### Folder-Based Visual Matching

For bulk operations, use visual-match-folder to find visually similar assets for all models in one or more folders:

```bash
# Find visually similar assets for all assets in a folder (parallel processing)
pcli2 asset visual-match-folder --path /Root/SearchFolder/ --format csv --progress

# Find visually similar assets across multiple folders
pcli2 asset visual-match-folder --path /Root/Folder1/ --path /Root/Folder2/ --format json --progress

# Use exclusive flag to limit results to matches within specified folders only
pcli2 asset visual-match-folder --path /Root/SearchFolder/ --exclusive --format csv --progress
```

This command processes all assets in the specified folder(s) simultaneously, making it efficient for large-scale visual similarity searches. The progress flag provides visual feedback during long-running operations.

#### Visual Matching Output

Visual matching results differ from geometric and part matching in that they do not include similarity percentages:

```bash
# View visual matching results in CSV format with headers
pcli2 asset visual-match --path /Root/Folder/ReferenceModel.stl --format csv --headers --metadata
```

The CSV output includes columns for:
- `REFERENCE_ASSET_PATH`: Path of the reference asset
- `CANDIDATE_ASSET_PATH`: Path of the visually similar asset
- `REFERENCE_ASSET_UUID`: UUID of the reference asset
- `CANDIDATE_ASSET_UUID`: UUID of the visually similar asset
- `COMPARISON_URL`: URL to view the comparison in the Physna UI
- Metadata columns (when using `--metadata` flag): `REF_*` and `CAND_*` prefixed columns for reference and candidate asset metadata

Visual matching is particularly useful for identifying assets with similar visual characteristics, textures, or appearances that may not be captured by geometric analysis alone.

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

### Environment Variables for Output Formatting

PCLI2 supports environment variables for configuring output formatting options:

- `PCLI2_FORMAT`: Sets the default output format (json, csv, tree). Can be overridden by the `--format` flag.
- `PCLI2_HEADERS`: Controls whether CSV output includes headers (true, 1, yes, on). Can be overridden by the `--headers` flag.

Example usage:
```bash
# Set default format to CSV
export PCLI2_FORMAT=csv

# Enable headers for CSV output by default
export PCLI2_HEADERS=true

# Both can be overridden by command-line flags
pcli2 config get --format json  # Uses JSON despite PCLI2_FORMAT=csv
```

The priority order for format configuration is:
1. Command-line flags (highest priority)
2. Environment variables
3. Default values (lowest priority)

### Using UNIX Pipes with PCLI2

PCLI2's structured output formats make it easy to pipe data to other command-line tools for further processing and analysis. This enables powerful workflows that combine PCLI2 with other utilities to manipulate, filter, and transform your data.

#### Basic Pipe Operations

PCLI2 outputs can be piped to standard Unix tools like `grep`, `awk`, `sed`, and others:

```bash
# Filter assets by name using grep
pcli2 asset list --format csv | grep "bearing"

# Count the number of assets
pcli2 asset list --format csv | wc -l

# Extract specific columns using cut (for CSV format)
pcli2 asset list --format csv | cut -d',' -f1,3
```

#### Working with Structured Data Tools

For more sophisticated data manipulation, PCLI2 works well with structured data processing tools:

**Using jq for JSON processing:**
```bash
# Extract specific fields from JSON output
pcli2 asset list --format json | jq '.[].name'

# Filter assets based on a condition
pcli2 asset list --format json | jq '.[] | select(.size > 10000)'
```

**Using csvkit for CSV processing:**
```bash
# Sort assets by a specific column
pcli2 asset list --format csv | csvsort -c "name"

# Filter CSV data based on conditions
pcli2 asset list --format csv | csvgrep -c "status" -m "active"
```

**Using NuShell for advanced data manipulation:**
```bash
# Convert CSV output and manipulate with NuShell
pcli2 asset list --format csv --headers | nu -c 'from csv | select name size status | sort-by name'

# More complex NuShell pipeline
pcli2 asset list --format csv --headers | nu -c 'from csv | where size > 1000 | get name | sort'
```

#### Combining Multiple PCLI2 Commands

You can chain multiple PCLI2 commands together for complex operations:

```bash
# Get a list of folders and process each one
pcli2 folder list --format csv | tail -n +2 | while IFS=',' read -r uuid name path; do
  echo "Processing folder: $name"
  pcli2 asset list --folder-uuid "$uuid" --format json
done
```

#### Best Practices for Pipelines

1. **Use appropriate output formats**: Choose CSV for tabular data processing, JSON for hierarchical data manipulation, and avoid tree format for piping.

2. **Include headers when needed**: Use `--headers` flag for CSV output when working with tools that expect header rows.

3. **Handle large datasets**: For large outputs, consider using the `--limit` and `--offset` parameters to process data in chunks.

4. **Error handling**: Redirect stderr appropriately when building pipelines:
   ```bash
   pcli2 asset list --format csv 2>/dev/null | head -n 10
   ```

5. **Performance considerations**: When chaining multiple API calls, consider the rate limits and use appropriate delays if needed.

These pipeline capabilities enable you to create sophisticated automation scripts and integrate PCLI2 seamlessly into your existing command-line workflows.

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



### Command Aliases and Short Argument Names

Some commands have shorter aliases. For example `list` has an alias of `ls`. Similarly, some command arguments have short names too. For example `--path` can be provided as `-p`. See the help for details.


### Uninstalling PCLI2

To completely uninstall PCLI2 from your system, follow the instructions for your platform:

#### Windows

**Uninstall via Control Panel:**
1. Open **Control Panel** → **Programs and Features**
2. Find **PCLI2** in the list of installed programs
3. Select it and click **Uninstall**
4. Follow the prompts to complete the removal

**Alternative method using command line:**
```cmd
# If you still have access to the original MSI file:
msiexec /x pcli2-x86_64-pc-windows-msvc.msi

# Or if you know the product code:
msiexec /x {PRODUCT_CODE}
```

**Manual cleanup (if needed):**
```cmd
# Remove configuration directory
rmdir /s "%APPDATA%\pcli2"

# Remove cache directory
rmdir /s "%LOCALAPPDATA%\pcli2"

# Remove data directory
rmdir /s "%LOCALAPPDATA%\pcli2-data"
```

#### macOS

**Uninstall using the installer script:**
```bash
# Run the installer script with uninstall flag (if supported)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh -s -- --uninstall
```

**Manual removal:**
```bash
# Remove the executable (typically installed in /usr/local/bin/)
sudo rm /usr/local/bin/pcli2

# Remove configuration directory
rm -rf ~/Library/Application\ Support/pcli2

# Remove cache directory
rm -rf ~/Library/Caches/pcli2

# Remove data directory
rm -rf ~/Library/Application\ Support/pcli2/data
```

#### Linux

**Uninstall using the installer script:**
```bash
# Run the installer script with uninstall flag (if supported)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh -s -- --uninstall
```

**Manual removal:**
```bash
# Remove the executable (common locations)
sudo rm /usr/local/bin/pcli2
# OR
sudo rm /usr/bin/pcli2

# Remove configuration directory
rm -rf ~/.config/pcli2

# Remove cache directory
rm -rf ~/.cache/pcli2

# Remove data directory
rm -rf ~/.local/share/pcli2
```

#### Removing Configuration and Data Files

Regardless of platform, to completely remove all PCLI2 data including configuration, cache, and stored credentials:

```bash
# Find configuration file location
pcli2 config get path

# Remove configuration directory
rm -rf "$(dirname "$(pcli2 config get path)")"

# Remove cache directory
rm -rf "/path/to/cache/directory"  # Platform-specific location

# Remove data directory
rm -rf "/path/to/data/directory"   # Platform-specific location
```

Platform-specific locations:
- **Windows**: `%APPDATA%\pcli2\`, `%LOCALAPPDATA%\pcli2\cache\`, `%LOCALAPPDATA%\pcli2\data\`
- **macOS**: `~/Library/Application Support/pcli2/`, `~/Library/Caches/pcli2/`, `~/Library/Application Support/pcli2/data/`
- **Linux**: `~/.config/pcli2/`, `~/.cache/pcli2/`, `~/.local/share/pcli2/`

PCLI2 follows standard platform conventions for data storage, so all data can be easily located and removed using standard file system operations.

## Commands Reference

The application uses a hierarchy of commands:

```
pcli2
├── asset
│   ├── create                    Create a new asset by uploading a file
│   ├── create-batch              Create multiple assets by uploading files matching a glob pattern
│   ├── delete                    Delete an asset [aliases: rm]
│   ├── list                      List all assets in a folder [aliases: ls]
│   ├── get                       Get asset details
│   ├── metadata                  Manage asset metadata
│   │   ├── get                   Get metadata for an asset
│   │   ├── create                Add metadata to an asset [aliases: update]
│   │   ├── delete                Delete specific metadata fields from an asset [aliases: rm]
│   │   ├── create-batch          Create metadata for multiple assets from a CSV file [aliases: update-batch]
│   │   └── inference             Apply metadata from a reference asset to geometrically similar assets
│   ├── dependencies              Get dependencies for an asset
│   ├── download                  Download asset file
│   ├── geometric-match           Find geometrically similar assets
│   ├── part-match                Find geometrically similar assets using part search algorithm
│   ├── geometric-match-folder    Find geometrically similar assets for all assets in one or more folders
│   ├── part-match-folder         Find part matches for all assets in one or more folders
│   ├── visual-match              Find visually similar assets for a specific reference asset
│   └── visual-match-folder       Find visually similar assets for all assets in one or more folders
├── folder
│   ├── create                    Create a new folder
│   ├── list                      List all folders [aliases: ls]
│   ├── get                       Get folder details
│   ├── delete                    Delete a folder [aliases: rm]
│   ├── rename                    Rename a folder
│   ├── move                      Move a folder to a new parent folder [aliases: mv]
│   ├── resolve                   Resolve a folder path to its UUID
│   └── download                  Download all assets in a folder as a ZIP archive
├── tenant
│   ├── list                      List all tenants [aliases: ls]
│   ├── get                       Get tenant details
│   ├── use                       Set the active tenant
│   ├── current                   Get the active tenant
│   ├── clear                     Clear the active tenant
│   └── state                     Get asset state counts for the current tenant
├── auth
│   ├── login                     Login using client credentials
│   ├── logout                    Logout and clear session
│   ├── get                       Get current access token
│   ├── clear-token               Clear the cached access token
│   └── expiration                Show the expiration time of the current access token
├── config
│   ├── get                       Get configuration details
│   ├── export                    Export configuration to file
│   ├── import                    Import configuration from file
│   └── environment               Manage environment configurations
│       ├── add                   Add a new environment configuration
│       ├── use                   Switch to an environment
│       ├── remove                Remove an environment
│       ├── list                  List all environments
│       ├── reset                 Reset all environment configurations to blank state
│       └── get                   Get environment details
├── completions                   Generate shell completions for various shells
└── help                          Show help information
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

### Shell Completions

PCLI2 supports shell completions for various shells. To generate completions:

```bash
# Generate ZSH completions
pcli2 completions zsh > ~/.zsh/completion/_pcli2

# Add the completion directory to your ZSH configuration (~/.zshrc)
fpath=(~/.zsh/completion $fpath)
autoload -U compinit && compinit

# Generate Bash completions
pcli2 completions bash > /etc/bash_completion.d/pcli2

# Generate Fish completions
pcli2 completions fish > ~/.config/fish/completions/pcli2.fish

# Generate PowerShell completions
pcli2 completions powershell > pcli2.ps1
```

After installing the completions, restart your shell or source your configuration file to enable tab completion for PCLI2 commands.

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
   pcli2 tenant current
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

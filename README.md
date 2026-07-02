# PCLI2 - Physna Command Line Interface v2

**Documentation**: [https://jchultarsky101.github.io/pcli2/](https://jchultarsky101.github.io/pcli2/)

PCLI2 is a powerful command-line interface for the Physna public API, designed for advanced 3D geometry search and analysis. Built with an intuitive nested sub-command structure, it offers sensible defaults and comprehensive configuration management.

## 🚀 Quick Start

Get up and running with PCLI2 in minutes:

```bash
# 1. Authenticate with your Physna tenant
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# 2. Verify your setup
pcli2 auth get

# 3. Start managing your assets and folders
pcli2 folder list --format tree
```

### 💡 Quick Tips

**Use aliases for faster workflows:**
```bash
pcli2 folder ls          # List folders
pcli2 asset ls           # List assets
pcli2 asset rm           # Delete asset
pcli2 folder mv          # Move folder
pcli2 auth in            # Login
pcli2 env list           # List environments
```

**Skip confirmation prompts in scripts:**
```bash
pcli2 asset delete --path /Root/Models/part.stl --yes
```

**Validate your setup:**
```bash
pcli2 config validate --verbose
```

## 📋 Table of Contents

- [Features](#-features)
- [Installation](#-installation)
- [Authentication](#-authentication)
- [Basic Usage](#-basic-usage)
- [Advanced Features](#-advanced-features)
- [Troubleshooting](#-troubleshooting)
- [Support](#-support)

## ✨ Features

- **Intuitive Command Structure** - Nested sub-commands like Git CLI
- **Command Aliases** - Unix-style shortcuts (`ls`, `rm`, `cat`, `dl`, `mv`) for faster workflows
- **Comprehensive Asset Management** - Create, list, get, delete, and analyze
- **Folder Operations** - Organize assets with full folder management
- **Geometric Matching** - Find similar 3D geometries
- **Part Matching** - Find part matches within assemblies
- **Visual Matching** - Find visually similar assets
- **Text Matching** - Find assets using text search
- **Metadata Operations** - Manage custom properties efficiently
- **Bulk Operations** - Process multiple assets with batch commands
- **Secure Authentication** - OAuth2 with system keyring integration
- **Confirmation Prompts** - Safety for destructive operations with `--yes` flag for scripting
- **Configuration Validation** - `config validate` command to verify setup before operations
- **Flexible Output Formats** - JSON, CSV, and tree views
- **Resume Capability** - Continue interrupted downloads seamlessly
- **Performance Optimizations** - Concurrent operations and caching for faster processing
- **Structured Logging** - Debug with `--verbose`/`--quiet` flags or the `PCLI2_LOG_LEVEL` environment variable
- **Progress Tracking** - Enhanced progress bars with throughput and ETA
- **Dry Run Mode** - Preview deletes and uploads with `--dry-run` before making changes
- **Automatic Retries** - Transient network and server errors retried with exponential backoff
- **Man Pages** - Generate Unix man pages for every command with `pcli2 man`
- **Update Notifications** - A gentle hint when a newer release is available
- **Pipe-Friendly Output** - Colors disabled automatically when output is piped (respects `NO_COLOR`)

## 💻 Installation

### Prerequisites
- Physna tenant with API client credentials
- Compatible OS (Windows, macOS, or Linux)

### Installation Methods

#### 📦 Pre-built Installers (Recommended)

Download from the [Latest Release](https://github.com/jchultarsky101/pcli2/releases/latest):

**macOS/Linux Universal Script:**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
```

**Windows PowerShell:**
```powershell
irm https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.ps1 | iex
```

#### 🔧 Building from Source

```bash
git clone https://github.com/jchultarsky101/pcli2.git
cd pcli2
cargo build --release
# Binary located at target/release/pcli2
```

#### 🍺 Homebrew (macOS/Linux)

```bash
# Add the PCLI2 tap
brew tap jchultarsky101/pcli2

# Install PCLI2
brew install pcli2
```

#### 🐳 Docker

Run PCLI2 in a container:

```bash
# Build the Docker image
docker build -t pcli2 .

# Run PCLI2 commands
docker run --rm -v $(pwd):/data -v ~/.config/pcli2:/home/pcli2/.config/pcli2 pcli2 --help

# Example: List folders
docker run --rm -v $(pwd):/data -v ~/.config/pcli2:/home/pcli2/.config/pcli2 pcli2 folder list

# Authenticate first (credentials persist in mounted volume)
docker run --rm -it -v ~/.config/pcli2:/home/pcli2/.config/pcli2 pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET
```

### Verification
```bash
pcli2 --version
```

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

## 🔐 Authentication

Securely authenticate with your Physna tenant:

```bash
# First-time login (interactive): prompts for the client ID and secret,
# with masked input so the secret never lands in your shell history
pcli2 auth login

# First-time login (non-interactive)
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# Subsequent logins (uses cached credentials)
pcli2 auth login

# Verify authentication
pcli2 auth get

# Check token expiration
pcli2 auth expiration
```

Credentials are securely stored using your system's keyring:
- **macOS**: Keychain Services
- **Windows**: Credential Manager  
- **Linux**: Secret Service API

## 🛠️ Basic Usage

### 🏢 Tenant Management

Set and manage your active tenant:

```bash
# List available tenants
pcli2 tenant list --format csv

# Set active tenant
pcli2 tenant use

# Get current tenant
pcli2 tenant current
```

### 📁 Folder Operations

Manage your folder structure:

```bash
# List folders in tree format
pcli2 folder list --format tree

# Create a new folder
pcli2 folder create --name "New Folder" --parent-folder-path "/Root/Parent"

# Download all assets from a folder
pcli2 folder download --folder-path "/Root/MyFolder" --output "backup" --resume

# Upload all assets from a local directory to a Physna folder
pcli2 folder upload --local-path "./local_models" --folder-path "/Root/MyFolder" --skip-existing

# Download thumbnails for all assets in a folder
pcli2 folder thumbnail --folder-path "/Root/MyFolder" --output "thumbnails" --progress
```

### 📦 Asset Management

Upload, download, and manage assets:

```bash
# Upload a single asset
pcli2 asset create --file path/to/model.stl --folder-path "/Root/Models/"

# Replace an existing asset (delete + re-upload)
pcli2 asset create --file path/to/model.stl --folder-path "/Root/Models/" --override

# Replace an existing asset and preserve its metadata
pcli2 asset create --file path/to/model.stl --folder-path "/Root/Models/" --override --restore-metadata

# List assets in a folder
pcli2 asset list --path "/Root/Models/" --format json

# Recursively list assets in subfolders
pcli2 asset list --path "/Root/Models/" --recursive --format csv --headers

# Download an asset
pcli2 asset download --path "/Root/Models/model.stl"

# View asset details
pcli2 asset get --path "/Root/Models/model.stl" --metadata

# Reprocess an asset to refresh its analysis
pcli2 asset reprocess --path "/Root/Models/model.stl"
# or
pcli2 asset reprocess --uuid 550e8400-e29b-41d4-a716-446655440000

# Download asset thumbnail
pcli2 asset thumbnail --path "/Root/Models/model.stl"
# or
pcli2 asset thumbnail --uuid 550e8400-e29b-41d4-a716-446655440000
# Specify custom output file
pcli2 asset thumbnail --uuid 550e8400-e29b-41d4-a716-446655440000 --file "my_thumbnail.png"
```

### 🔍 Geometric Matching

Find similar 3D geometries:

```bash
# Find similar assets to a reference model
pcli2 asset geometric-match --path "/Root/Models/reference.stl" --threshold 85.0

# Bulk matching across folders
pcli2 folder geometric-match --folder-path "/Root/SearchFolder/" --threshold 90.0 --progress

# Exclusive matching - only show matches where both assets belong to the specified paths
pcli2 folder geometric-match --folder-path "/Root/SearchFolder/" --threshold 90.0 --exclusive

# Color-highlighted Excel report (frozen headers, grouped metadata pairs, diff colors, match heat-map)
pcli2 folder geometric-match --folder-path "/Root/SearchFolder/" --threshold 90.0 --format xls --output report.xlsx

# Compare two specific assets and get their pairwise match scores
# (each asset can be given by --*-path or --*-uuid; alias: asset match-scores)
pcli2 asset similarity --reference-path "/Root/Models/block1.stl" --candidate-path "/Root/Models/block2.stl"
```

#### Exclusive Matching

The `--exclusive` flag restricts results to only show matches where **both** the reference asset and the matched asset belong to the specified folder paths. Without this flag, matches between assets in the specified folder and assets in other folders will also be included.

For example:
- Without `--exclusive`: Assets in `/FolderA` can match assets in `/FolderB`
- With `--exclusive`: Only assets in `/FolderA` matching other assets in `/FolderA` will be shown

### 🏷️ Metadata Operations

Manage custom properties:

```bash
# Add metadata to an asset
pcli2 asset metadata create --path "/Root/Models/part.stl" --name "Material" --value "Steel"

# Get all metadata for an asset
pcli2 asset metadata get --path "/Root/Models/part.stl"

# Bulk metadata update from CSV (classic vertical or Physna UI horizontal
# layout, auto-detected from the header row)
pcli2 asset metadata create-batch --csv-file "metadata.csv"
```

## 🚀 Advanced Features

### ⚡ Performance Options

Optimize operations for large datasets:

```bash
# Concurrent downloads (faster for many files)
pcli2 folder download --folder-path "/Root/LargeFolder/" --concurrent 5 --progress

# Add delays to prevent rate limiting
pcli2 folder download --folder-path "/Root/Folder/" --delay 2

# Continue on errors
pcli2 folder download --folder-path "/Root/Folder/" --continue-on-error

# Continue past unresolvable asset paths in a metadata batch
pcli2 asset metadata create-batch --csv-file "metadata.csv" --continue-on-error

# Download thumbnails for all assets in a folder
pcli2 folder thumbnail --folder-path "/Root/Folder/" --progress --concurrent 3
```

### 🔄 Resume Interrupted Downloads

Skip existing files to resume large downloads:

```bash
# Resume a partially completed download
pcli2 folder download --folder-path "/Root/LargeFolder/" --resume --progress
```

### 🧪 Dry Run Mode

Preview destructive or bulk operations without changing anything on the server:

```bash
# See what a delete would remove
pcli2 folder delete --folder-path "/Root/Old Projects/" --force --dry-run
pcli2 asset delete --path "/Root/Models/part.stl" --dry-run

# See exactly which files a batch upload would send, and where
pcli2 asset create-batch --files "data/*.stl" --folder-path "/Root/Models/" --dry-run
pcli2 folder upload --local-path ./models --folder-path "/Root/Models/" --dry-run
```

### 🔁 Automatic Retries

Transient failures (network timeouts, connection errors, and HTTP
408/429/502/503/504 responses) are retried automatically with exponential
backoff, honoring the server's `Retry-After` header when present. The
default is 2 retries; override it with the `PCLI2_MAX_RETRIES` environment
variable (0 disables retries):

```bash
PCLI2_MAX_RETRIES=5 pcli2 folder download --folder-path "/Root/Models/" --output ./downloads
```

### 🎨 Color Control

Colors are disabled automatically when output is piped or redirected.
You can also disable them explicitly with the `--no-color` flag, or the
`NO_COLOR`/`PCLI2_NO_COLOR` environment variables.

### 🔔 Update Notifications

After a successful command, PCLI2 prints a one-line hint on stderr when a
newer release is available (checked at most once per day, terminal sessions
only, never in CI). Opt out with:

```bash
export PCLI2_NO_UPDATE_CHECK=1
```

### 📊 Download and Upload Statistics Reports

When using folder download and upload commands, you'll receive detailed statistics reports:

**Download Statistics Report:**
```
📊 Download Statistics Report
===========================
✅ Successfully downloaded: 125 assets
⏭️  Skipped (already existed): 75 assets
❌ Failed downloads: 2 assets
📁 Total assets processed: 202 assets
⏳ Operation completed successfully!
```

**Upload Statistics Report:**
```
📊 Upload Statistics Report
==========================
✅ Successfully uploaded: 150 assets
⏭️  Skipped (already existed): 0 assets
❌ Failed uploads: 1 asset
📁 Total assets processed: 151 assets
⏳ Operation completed successfully!
```

### 📊 Output Formats

Choose the right format for your needs:

```bash
# JSON for scripting
pcli2 asset list --format json

# CSV for spreadsheets
pcli2 asset list --format csv --headers

# Recursively list assets in subfolders
pcli2 asset list --folder-path "/Root/Models/" --recursive --format csv --headers

# Tree for visual hierarchy
pcli2 folder list --format tree
```

### 🔗 UNIX Pipeline Integration

Chain commands with other tools:

```bash
# Filter assets with grep
pcli2 asset list --format csv | grep "bearing"

# Process with jq
pcli2 asset list --format json | jq '.[] | select(.size > 10000)'

# Count results
pcli2 asset list --format csv | wc -l

# Advanced filtering with NuShell (nushell)
# Filter assets by metadata values like weight in specific range
pcli2 asset list --folder-path "/Root/MyFolder" --metadata --format json | nu -c 'from json | where ((metadata | get-or-null Weight) | default 0) >= 5.0 and ((metadata | get-or-null Weight) | default 0) <= 50.0 | select name path metadata.Material metadata.Weight'

# Group assets by material type using NuShell
pcli2 asset list --folder-path "/Root/Inventory" --metadata --format json | nu -c 'from json | where metadata.Material != null | group-by metadata.Material | each {|it| {material: ($it | get 0).metadata.Material, count: ($it | length), avg_weight: ($it | get metadata.Weight | compact | math avg)}}'
```

### 🤖 CI/CD Integration

PCLI2 is designed to behave well in automation: colors and spinners turn off
when output is piped, `--yes` skips confirmation prompts, `--quiet` limits
diagnostics to errors, and exit codes identify the failure class (see
[Exit Codes](#exit-codes)). Example GitHub Actions job that uploads build
artifacts to Physna:

```yaml
jobs:
  upload-models:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install pcli2
        run: curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
      - name: Authenticate
        run: pcli2 auth login --client-id "${{ secrets.PHYSNA_CLIENT_ID }}" --client-secret "${{ secrets.PHYSNA_CLIENT_SECRET }}"
      - name: Upload models
        run: |
          pcli2 tenant use --name my-tenant
          pcli2 asset create-batch --files "build/*.stl" \
            --folder-path "/Root/CI Builds/" --quiet --format json
```

### ⚙️ Configuration Management

Manage multiple environments:

```bash
# Add a development environment
pcli2 config environment add --name "development" \
  --api-url "https://dev-api.physna.com/v3"

# Switch environments
pcli2 config environment use --name development

# List all environments
pcli2 config environment list
```

#### Environment Command (Alias: `env`)

For better ergonomics, you can also use the top-level `env` command:

```bash
pcli2 env add -n dev --api-url "https://dev-api.physna.com/v3"
pcli2 env use -n dev
pcli2 env list
pcli2 env get
```

#### Configuration Validation

Validate your configuration before running operations:

```bash
# Quick validation
pcli2 config validate

# Detailed validation with API connectivity test
pcli2 config validate --verbose --api
```

#### Structured Logging

Enable debug logging for troubleshooting:

```bash
# Quick verbosity control with global flags
pcli2 --verbose folder download --path /Root/Models/   # debug-level logging
pcli2 --quiet asset list --folder-path /Root/Models/   # errors only

# Fine-grained control with environment variables
PCLI2_LOG_LEVEL=debug pcli2 folder download --path /Root/Models/
RUST_LOG=pcli2=trace pcli2 asset get --uuid xxx
```

The `--verbose`/`-v` and `--quiet` flags work on every command and take
precedence over the environment variables.

## 🛠️ Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| **API Rate Limiting** | Reduce concurrency with `--concurrent N` |
| **Timeout Errors** | Operations now have 30-minute timeouts |
| **Authentication Expired** | Run `pcli2 auth login` |
| **Large Folder Processing** | Use `--progress` for feedback |
| **Memory Issues** | Reduce concurrency for limited RAM systems |

### Error Messages

- **"Asset not found"** - Verify the asset path exists in your tenant
- **"API rate limit exceeded"** - Reduce concurrency or add delays
- **"Permission denied"** - Check your access permissions
- **"Configuration file not found"** - Run `pcli2 config get path`

### Exit Codes

PCLI2 uses distinct exit codes (following BSD `sysexits.h` conventions where
possible) so scripts can react to specific failure classes:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 64 | Command line usage error |
| 65 | Data format error |
| 66 | Cannot open input file |
| 67 | Resource not found |
| 68 | Service unavailable |
| 69 | Temporary failure |
| 70 | Internal software error |
| 71 | Operating system error |
| 78 | Configuration error |
| 100 | Authentication error |
| 101 | Network communication error |
| 102 | Remote API error |

```bash
pcli2 asset get --path "/Root/Models/part.stl" --format json
case $? in
  0)   echo "found" ;;
  100) pcli2 auth login ;;
  101) echo "network problem, try again later" ;;
  *)   echo "failed" ;;
esac
```

### Debugging Tips

```bash
# Check authentication status
pcli2 auth get

# Verify current context
pcli2 tenant current

# Review configuration
pcli2 config get

# Validate setup
pcli2 config validate --verbose

# Enable debug logging
PCLI2_LOG_LEVEL=debug pcli2 folder list
```

## 🔢 Command Aliases Reference

Quick reference for all available command aliases:

### Tenant Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 tenant list` | `pcli2 tenant ls` |
| `pcli2 tenant use` | `pcli2 tenant select` |
| `pcli2 tenant clear` | `pcli2 tenant unset` |

### Folder Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 folder list` | `pcli2 folder ls` |
| `pcli2 folder delete` | `pcli2 folder rm` |
| `pcli2 folder get` | `pcli2 folder cat` |
| `pcli2 folder create` | `pcli2 folder add` |
| `pcli2 folder move` | `pcli2 folder mv` |
| `pcli2 folder rename` | `pcli2 folder ren` |
| `pcli2 folder resolve` | `pcli2 folder res` |
| `pcli2 folder download` | `pcli2 folder dl` |

### Asset Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 asset list` | `pcli2 asset ls` |
| `pcli2 asset delete` | `pcli2 asset rm` |
| `pcli2 asset get` | `pcli2 asset cat` |
| `pcli2 asset create` | `pcli2 asset upload` |
| `pcli2 asset download` | `pcli2 asset dl` |
| `pcli2 asset dependencies` | `pcli2 asset deps` |
| `pcli2 asset dependency-diff` | `pcli2 asset deps-diff` |
| `pcli2 asset thumbnail` | `pcli2 asset thumb` |

### Authentication Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 auth` | `pcli2 a` |
| `pcli2 auth login` | `pcli2 auth in` |
| `pcli2 auth logout` | `pcli2 auth out` |
| `pcli2 auth get` | `pcli2 auth token` |
| `pcli2 auth clear-token` | `pcli2 auth clear` |
| `pcli2 auth expiration` | `pcli2 auth exp` |

### Environment Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 environment` | `pcli2 env` |

## 📜 Commands Reference

### Asset Commands

Manage individual assets in your Physna tenant.

```
pcli2 asset create           # Upload a file as an asset
pcli2 asset create-batch     # Upload multiple files as assets using glob patterns
pcli2 asset list             # List assets in a folder with optional recursive listing (--recursive)
pcli2 asset inventory        # List complete inventory of all assets in the tenant
pcli2 asset counts           # Show asset health report with counts by state, type, and structure
pcli2 asset get              # Get asset details
pcli2 asset download         # Download an asset
pcli2 asset delete           # Delete an asset
pcli2 asset dependencies     # Get dependencies for an asset
pcli2 asset dependency-diff  # Diff the dependency trees of two assets
pcli2 asset geometric-match  # Find geometrically similar assets
pcli2 asset part-match       # Find part matches for an asset
pcli2 asset visual-match     # Find visually similar assets (--limit N, default 100; --threshold N size filter, default 80)
pcli2 asset text-match       # Find assets using text search
pcli2 asset reprocess        # Reprocess an asset to refresh its analysis
pcli2 asset thumbnail        # Download asset thumbnail
pcli2 asset metadata         # Manage asset metadata
```

#### Asset List Command

The `asset list` command lists assets in a folder with various filtering and formatting options.

```bash
# List assets in a specific folder
pcli2 asset list --folder-path "/Root/Models/"

# List assets recursively (including all subfolders)
pcli2 asset list --folder-path "/Root/Models/" --recursive

# Output in CSV format with headers
pcli2 asset list --folder-path "/Root/Models/" --format csv --headers

# Include metadata in JSON output
pcli2 asset list --folder-path "/Root/Models/" --format json --metadata

# Force refresh folder cache before listing (useful after folder changes)
pcli2 asset list --reload --folder-path "/Root/Models/" --format csv
```

#### Asset Inventory and Counts Commands

The `asset inventory` and `asset counts` commands both retrieve all assets across the entire tenant (not scoped to a folder). They differ in output:

- `asset inventory` outputs the full list of assets (same format as `asset list`)
- `asset counts` outputs an aggregated health report with counts by processing state, file type, and structure

```bash
# Full inventory in CSV with headers
pcli2 asset inventory --format csv --headers

# Full inventory in JSON with metadata
pcli2 asset inventory --format json --metadata

# Asset health report in JSON
pcli2 asset counts --format json

# Asset health report in CSV
pcli2 asset counts --format csv --headers
```

#### Asset Dependency Diff Command

The `asset dependency-diff` command (alias `deps-diff`) compares the recursive dependency trees of two assemblies — a **reference** and a **candidate** — and reports which parts differ between them.

Each asset is identified by either its UUID or its path, consistent with other asset commands. Provide exactly one identifier per side:

- `--reference-uuid` or `--reference-path`
- `--candidate-uuid` or `--candidate-path`

The comparison is **structural**: the two trees are walked in parallel and their nodes are matched by **filename**. It is **presence-only** — a part is reported as present in both (`=`), only in the reference (`-`), or only in the candidate (`+`); occurrence counts are not compared. If a whole subassembly is present on only one side, its entire subtree is marked accordingly.

Supported output formats: `tree` (default view of the merged diff), `json`, and `csv`.

```bash
# Diff two assemblies by path, rendered as a tree
pcli2 asset dependency-diff \
  --reference-path /Parts/AssemblyA.SLDASM \
  --candidate-path /Parts/AssemblyB.SLDASM \
  --format tree

# Diff by UUID, as pretty JSON
pcli2 asset deps-diff \
  --reference-uuid 00000000-0000-0000-0000-000000000001 \
  --candidate-uuid 00000000-0000-0000-0000-000000000002 \
  --format json --pretty

# Diff as CSV with headers (STATUS, ASSEMBLY_PATH, FILENAME, ASSET_UUID, ASSET_STATE)
pcli2 asset deps-diff \
  --reference-path /Parts/AssemblyA.SLDASM \
  --candidate-path /Parts/AssemblyB.SLDASM \
  --format csv --headers
```

Example tree output:

```
dependency diff: reference `/Parts/AssemblyA.SLDASM` vs candidate `/Parts/AssemblyB.SLDASM`
├─ (=) gearbox.sldasm [finished] (…)
│  ├─ (=) shaft.stl [finished] (…)
│  ├─ (-) bearing-v1.stl [finished] (…)
│  └─ (+) bearing-v2.stl [finished] (…)
├─ (-) bracket-old.stl [finished] (…)
└─ (+) bracket-new.stl [finished] (…)

Legend: (=) in both  (-) only in reference  (+) only in candidate
Summary: 2 common, 2 only in reference, 2 only in candidate
```

If either asset cannot be resolved, the command reports which input (reference or candidate) failed. An asset that is not an assembly is treated as having no dependencies.

#### Asset Metadata Commands

Manage custom properties for assets.

```
pcli2 asset metadata get           # Get metadata for an asset
pcli2 asset metadata create        # Add metadata to an asset
pcli2 asset metadata delete        # Delete specific metadata fields from an asset
pcli2 asset metadata create-batch  # Create metadata for multiple assets from a CSV file
pcli2 asset metadata inference     # Apply metadata from a reference asset to geometrically similar assets
```

#### Asset Metadata Create-Batch CSV Formats

The `asset metadata create-batch` command accepts two CSV layouts. The layout is auto-detected from the header row (any column starting with `metadata:` selects the UI format), or can be forced with `--csv-format classic|ui`.

**Classic (vertical) format** — one row per asset+field combination:

```
ASSET_PATH,NAME,VALUE
/Root/Folder/Model1.stl,Material,Steel
/Root/Folder/Model1.stl,Weight,"15.5 kg"
/Root/Folder/Model2.ipt,Material,Aluminum
/Root/Folder/Model2.ipt,Supplier,Richardson Electronics
```

- The first row must contain the headers `ASSET_PATH,NAME,VALUE`
- Each row represents a single metadata field assignment for an asset
- If an asset has multiple metadata fields to update, include multiple rows with the same `ASSET_PATH` but different `NAME` and `VALUE` combinations

**UI (horizontal) format** — one row per asset, as exported by the Physna web UI's bulk metadata upload:

```
path,id,metadata:Material,metadata:Color
/Root/Folder/Model1.stl,,Steel,Blue
/Root/Folder/Model2.ipt,123e4567-e89b-12d3-a456-426614174000,Aluminum,Red
```

- `path` is the asset path; the optional `id` column holds the asset UUID and takes precedence over the path when present
- Each `metadata:<field name>` column sets one metadata field (the prefix is stripped)
- Unrecognized columns are ignored with a warning

In both formats, empty values are skipped by default (existing metadata is left untouched). Pass `--delete-if-empty` to instead delete a metadata field from the asset when the file contains an empty value for it.

**General requirements** (both formats):
- The file must be UTF-8 encoded
- Values containing commas, quotes, or newlines must be enclosed in double quotes
- Empty rows will be ignored

**Error Handling**:

By default, the batch stops on the first error and reports how many assets were processed successfully. Pass `--continue-on-error` to skip rows whose `ASSET_PATH` cannot be resolved and continue with the remaining assets. Metadata API failures (e.g. a failed update for a resolved asset) always terminate the batch regardless of the flag, because the API layer already retries transient HTTP failures internally.

### Folder Commands

Manage folder structures and bulk operations.

```
pcli2 folder list             # List folder structure (defaults to root path if no path/UUID specified)
pcli2 folder create           # Create a new folder
pcli2 folder get              # Get folder details
pcli2 folder delete           # Delete a folder
pcli2 folder rename           # Rename a folder
pcli2 folder move             # Move a folder to a new parent folder
pcli2 folder resolve          # Resolve a folder path to its UUID
pcli2 folder download         # Download all assets in a folder (supports --resume flag)
pcli2 folder upload           # Upload all assets from a local directory to a Physna folder
pcli2 folder dependencies     # Get dependencies for all assembly assets in folder
pcli2 folder geometric-match  # Find geometrically similar assets for all assets in folder
pcli2 folder part-match       # Find part matches for all assets in folder
pcli2 folder visual-match     # Find visually similar assets for all assets in folder (--limit N, default 100; --threshold N size filter, default 80)
pcli2 folder thumbnail        # Download thumbnails for all assets in a folder
```

**Important Note**: Folder paths are **case-insensitive**. You can use any capitalization when specifying folder paths (e.g., `/Root/Models`, `/root/models`, `/ROOT/MODELS` all refer to the same folder). This matches the behavior of Windows file systems and provides a more user-friendly experience.

#### Folder Resolve Command

The `folder resolve` command resolves a folder path to its UUID, which can be useful for scripting or debugging.

```bash
# Resolve a folder path to UUID
pcli2 folder resolve --folder-path "/Root/MyFolder"

# Force refresh folder cache before resolving (useful if folder was recently recreated)
pcli2 folder resolve --reload --folder-path "/Root/MyFolder"
```

#### Folder List Command

The `folder list` command allows you to list folders in your Physna tenant. When no folder path or UUID is specified, it defaults to listing the root folder.

```bash
# List all folders in the root directory (default behavior)
pcli2 folder list

# List folders in a specific path
pcli2 folder list --folder-path "/Root/MyFolder"

# List folders using folder UUID
pcli2 folder list --folder-uuid 123e4567-e89b-12d3-a456-426614174000

# List folders with specific output format
pcli2 folder list --format tree
```

**Key Features**:
- **Default Root Path**: When no folder identifier is provided, defaults to the root path (`/`)
- **Mutual Exclusivity**: You can specify either `--folder-path` or `--folder-uuid`, but not both
- **Flexible Output**: Supports JSON, CSV, and tree formats
- **Folder Hierarchy**: Shows the complete folder structure when using tree format
- **Cache Refresh**: Use `--reload` flag to force refresh of folder cache before listing

```bash
# Force refresh folder cache before listing (useful after folder changes)
pcli2 folder list --reload

# Combine with other flags
pcli2 folder list --reload --format tree
```

### Tenant Commands

Manage tenant-level operations.

```
pcli2 tenant list     # List all tenants
pcli2 tenant get      # Get tenant details
pcli2 tenant use      # Set the active tenant
pcli2 tenant current  # Get the active tenant
pcli2 tenant clear    # Clear the active tenant
pcli2 tenant state    # Get asset state counts for the current tenant
```

### Authentication Commands

Manage authentication with your Physna tenant.

```
pcli2 auth login        # Authenticate with Physna using client credentials
pcli2 auth logout       # Logout and clear session
pcli2 auth get          # Get current access token
pcli2 auth clear-token  # Clear the cached access token
pcli2 auth expiration   # Show token expiration time
```

### Configuration Commands

Manage PCLI2 configuration settings.

```
pcli2 config get           # Get configuration details
pcli2 config export        # Export configuration to file
pcli2 config import        # Import configuration from file
pcli2 config environment   # Manage environment configurations
```

#### Environment Configuration Commands

Manage multiple Physna environment configurations.

```
pcli2 config environment add --name <name>     # Add a new environment configuration
pcli2 config environment add -n <name>         # Short form of add with name
pcli2 config environment use --name <name>     # Switch to an environment
pcli2 config environment use -n <name>         # Short form of use with name
pcli2 config environment remove --name <name>  # Remove an environment
pcli2 config environment remove -n <name>      # Short form of remove with name
pcli2 config environment list                  # List all environments
pcli2 config environment reset                 # Reset all environment configurations
pcli2 config environment get --name <name>     # Get environment details
pcli2 config environment get -n <name>         # Short form of get with name
```

### Other Commands

Additional utility commands.

```
pcli2 cache          # Cache management (clear cached data)
pcli2 completions    # Generate shell completions for various shells
pcli2 man            # Generate man pages for all commands
```

#### Cache Management Command

The `cache` command provides tools for managing PCLI2's local cache. Caching improves performance by storing folder hierarchies, metadata definitions, and tenant lists locally, but can sometimes contain stale data.

```bash
# Clear all caches (folder, metadata, and tenant)
pcli2 cache clear

# Clear all caches without confirmation prompt (useful for scripts)
pcli2 cache clear --yes

# Clear specific cache types
pcli2 cache clear --folder      # Clear only folder hierarchy cache
pcli2 cache clear --metadata    # Clear only metadata field cache
pcli2 cache clear --tenant      # Clear only tenant list cache

# Combine flags to clear multiple specific caches
pcli2 cache clear --folder --metadata --yes

# Use the alias 'clean'
pcli2 cache clean --yes
```

**When to clear the cache:**
- After deleting and recreating folders with the same name
- When folder paths return unexpected results
- After making bulk changes to folder structure
- When troubleshooting "folder not found" errors
- Periodically to ensure fresh data from the API

**Note:** The `--reload` flag on commands like `folder list`, `folder resolve`, and `asset list` provides a convenient way to refresh the folder cache for a single operation without clearing all caches.

```bash
# Example: List assets with fresh folder data
pcli2 asset list --reload --folder-path "/Root/Models/" --format csv
```

#### Shell Completions

Generate shell completions for various shells to enable tab completion for PCLI2 commands.

```bash
# Generate shell completions for various shells
pcli2 completions bash      # Generate bash completions
pcli2 completions zsh       # Generate zsh completions
pcli2 completions fish      # Generate fish completions
pcli2 completions powershell # Generate PowerShell completions
pcli2 completions elvish    # Generate Elvish completions

# Install bash completions (system-wide)
sudo pcli2 completions bash > /etc/bash_completion.d/pcli2
# Or for user-specific installation:
mkdir -p ~/.local/share/bash-completion/completions
pcli2 completions bash > ~/.local/share/bash-completion/completions/pcli2

# Install zsh completions (MacOS/Linux)
# For system-wide installation (requires sudo):
sudo pcli2 completions zsh > /usr/local/share/zsh/site-functions/_pcli2
# For user-specific installation:
mkdir -p ~/.zsh/completions  # Standard location (note the 's' at the end)
pcli2 completions zsh > ~/.zsh/completions/_pcli2
# Then add to your ~/.zshrc:
# fpath=(~/.zsh/completions $fpath)
# autoload -U compinit && compinit

# Alternative location (if your system uses the singular form):
# mkdir -p ~/.zsh/completion
# pcli2 completions zsh > ~/.zsh/completion/_pcli2

# Alternative zsh installation method (works on most systems):
pcli2 completions zsh > ~/.zfunc/_pcli2
# Add the following line to your ~/.zshrc:
# fpath+=~/.zfunc; autoload -U compinit && compinit

# Install fish completions
# For user-specific installation:
mkdir -p ~/.config/fish/completions
pcli2 completions fish > ~/.config/fish/completions/pcli2.fish

# Install PowerShell completions
# Add to your PowerShell profile:
pcli2 completions powershell > pcli2-completion.ps1
# Then dot source it in your PowerShell profile:
# . "/path/to/pcli2-completion.ps1"
```

#### Man Pages

Generate Unix man pages for PCLI2 and every subcommand (one page per
command, e.g. `pcli2-folder-delete.1`):

```bash
# Write pages to a directory
pcli2 man --output-dir ./man

# Install for the current user (path may vary by system)
mkdir -p ~/.local/share/man/man1
pcli2 man --output-dir ~/.local/share/man/man1

# Read a page
man pcli2-asset-create-batch
```

## 🤝 Support

Need help? 

1. Check the [GitHub Issues](https://github.com/jchultarsky101/pcli2/issues) for known issues
2. Search for similar problems in the issue tracker
3. Create a new issue with:
   - Your OS and PCLI2 version
   - The command you're executing
   - The error message received
   - Steps to reproduce the issue

## 🛠️ Code Quality

This project maintains high code quality standards:

- **Clean Code**: All code passes Rust clippy with `-D warnings` (deny warnings)
- **Formatted**: All code follows Rust fmt standards
- **Well Tested**: Comprehensive unit and integration tests
- **Documented**: Thorough documentation for all public APIs

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.
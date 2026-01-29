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
pcli2 asset list --format tree
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
- **Comprehensive Asset Management** - Create, list, get, delete, and analyze
- **Folder Operations** - Organize assets with full folder management
- **Geometric Matching** - Find similar 3D geometries
- **Metadata Operations** - Manage custom properties efficiently
- **Bulk Operations** - Process multiple assets with batch commands
- **Secure Authentication** - OAuth2 with system keyring integration
- **Flexible Output Formats** - JSON, CSV, and tree views
- **Resume Capability** - Continue interrupted downloads seamlessly

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

### Verification
```bash
pcli2 --version
```

## 🔐 Authentication

Securely authenticate with your Physna tenant:

```bash
# First-time login
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
pcli2 folder create --name "New Folder" --path "/Root/Parent"

# Download all assets from a folder
pcli2 folder download --folder-path "/Root/MyFolder" --output "backup" --resume
```

### 📦 Asset Management

Upload, download, and manage assets:

```bash
# Upload a single asset
pcli2 asset create --file path/to/model.stl --path "/Root/Models/"

# List assets in a folder
pcli2 asset list --path "/Root/Models/" --format json

# Download an asset
pcli2 asset download --path "/Root/Models/model.stl"

# View asset details
pcli2 asset get --path "/Root/Models/model.stl" --metadata
```

### 🔍 Geometric Matching

Find similar 3D geometries:

```bash
# Find similar assets to a reference model
pcli2 asset geometric-match --path "/Root/Models/reference.stl" --threshold 85.0

# Bulk matching across folders
pcli2 folder geometric-match --folder-path "/Root/SearchFolder/" --threshold 90.0 --progress
```

### 🏷️ Metadata Operations

Manage custom properties:

```bash
# Add metadata to an asset
pcli2 asset metadata create --path "/Root/Models/part.stl" --name "Material" --value "Steel"

# Get all metadata for an asset
pcli2 asset metadata get --path "/Root/Models/part.stl"

# Bulk metadata update from CSV
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
```

### 🔄 Resume Interrupted Downloads

Skip existing files to resume large downloads:

```bash
# Resume a partially completed download
pcli2 folder download --folder-path "/Root/LargeFolder/" --resume --progress

# Statistics report shows skipped, downloaded, and failed files
```

### 📊 Output Formats

Choose the right format for your needs:

```bash
# JSON for scripting
pcli2 asset list --format json

# CSV for spreadsheets
pcli2 asset list --format csv --headers

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
```

### ⚙️ Configuration Management

Manage multiple environments:

```bash
# Add a development environment
pcli2 config environment add --name "development" \
  --api-url "https://dev-api.physna.com/v3"

# Switch environments
pcli2 config environment use development

# List all environments
pcli2 config environment list
```

## 📊 Download Statistics Report

When using folder download commands, you'll receive a detailed statistics report:

```
📊 Download Statistics Report
===========================
✅ Successfully downloaded: 125 assets
⏭️  Skipped (already existed): 75 assets
❌ Failed downloads: 2 assets
📁 Total assets processed: 202 assets
⏳ Operation completed successfully!
```

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

### Debugging Tips

```bash
# Check authentication status
pcli2 auth get

# Verify current context
pcli2 tenant current

# Review configuration
pcli2 config get
```

## 📜 Commands Reference

### Asset Commands

Manage individual assets in your Physna tenant.

```
pcli2 asset create           # Upload a file as an asset
pcli2 asset create-batch     # Upload multiple files as assets using glob patterns
pcli2 asset list             # List assets in a folder
pcli2 asset get              # Get asset details
pcli2 asset download         # Download an asset
pcli2 asset delete           # Delete an asset
pcli2 asset dependencies     # Get dependencies for an asset
pcli2 asset geometric-match  # Find geometrically similar assets
pcli2 asset part-match       # Find part matches for an asset
pcli2 asset visual-match     # Find visually similar assets
pcli2 asset text-match       # Find assets using text search
pcli2 asset metadata         # Manage asset metadata
```

#### Asset Metadata Commands

Manage custom properties for assets.

```
pcli2 asset metadata get           # Get metadata for an asset
pcli2 asset metadata create        # Add metadata to an asset
pcli2 asset metadata delete        # Delete specific metadata fields from an asset
pcli2 asset metadata create-batch  # Create metadata for multiple assets from a CSV file
pcli2 asset metadata inference     # Apply metadata from a reference asset to geometrically similar assets
```

### Folder Commands

Manage folder structures and bulk operations.

```
pcli2 folder list             # List folder structure
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
pcli2 folder visual-match     # Find visually similar assets for all assets in folder
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
pcli2 config environment add     # Add a new environment configuration
pcli2 config environment use     # Switch to an environment
pcli2 config environment remove  # Remove an environment
pcli2 config environment list    # List all environments
pcli2 config environment reset   # Reset all environment configurations
pcli2 config environment get     # Get environment details
```

### Other Commands

Additional utility commands.

```
pcli2 completions  # Generate shell completions for various shells
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

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.
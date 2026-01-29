# Quick Start Guide

This guide will help you get started with PCLI2 quickly by walking through common tasks.

## Table of Contents
- [Authentication](#authentication)
- [Basic Navigation](#basic-navigation)
- [Working with Assets](#working-with-assets)
- [Working with Folders](#working-with-folders)
- [Geometric Matching](#geometric-matching)
- [Configuration](#configuration)
- [Context Management](#context-management)
- [Next Steps](#next-steps)
- [Getting Help](#getting-help)

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

## Basic Navigation

Learn to navigate your Physna tenant using PCLI2:

```bash
# List available tenants
pcli2 tenant list --format csv

# Set active tenant
pcli2 tenant use

# Get current tenant
pcli2 tenant current

# List folders in tree format
pcli2 folder list --format tree

# List assets in a specific folder
pcli2 asset list --path /Root/MyFolder/
```

## Working with Assets

Common asset operations you'll perform regularly:

```bash
# Upload a single asset
pcli2 asset create --file path/to/my/model.stl --path /Root/MyFolder/

# View asset details
pcli2 asset get --path /Root/MyFolder/model.stl

# View asset details with metadata
pcli2 asset get --path /Root/MyFolder/model.stl --metadata

# List all assets in a folder
pcli2 asset list --path "/Root/MyFolder" --format json

# Download a single asset
pcli2 asset download --path /Root/MyFolder/model.stl

# Delete an asset
pcli2 asset delete --path /Root/MyFolder/model.stl
```

## Working with Folders

Manage your folder structure:

```bash
# Create a new folder
pcli2 folder create --name "New Folder" --path "/Root/Parent"

# Download all assets from a folder
pcli2 folder download --folder-path "/Root/MyFolder" --output "backup" --resume

# Download with progress indicator
pcli2 folder download --folder-path "/Root/MyFolder" --progress

# Download with all options combined
pcli2 folder download --folder-path "/Root/MyFolder" --concurrent 3 --continue-on-error --delay 1 --progress --resume
```

## Geometric Matching

Find similar assets using PCLI2's powerful geometric matching:

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0

# Find matches for all assets in a folder (parallel processing)
pcli2 folder geometric-match --folder-path /Root/SearchFolder/ --threshold 90.0 --format csv --progress
```

## Configuration

Manage your PCLI2 configuration:

```bash
# View current configuration
pcli2 config get

# View configuration file path
pcli2 config get path

# Add a new environment configuration
pcli2 config environment add --name "development" \
  --api-url "https://dev-api.physna.com/v3" \
  --ui-url "https://dev.physna.com" \
  --auth-url "https://dev-auth.physna.com/oauth2/token"

# Switch to an environment
pcli2 config environment use development

# List all environments
pcli2 config environment list
```

## Context Management

Work with multiple tenants efficiently:

```bash
# Set active tenant
pcli2 tenant use

# Get current tenant
pcli2 tenant current

# Clear active tenant
pcli2 tenant clear

# Override tenant selection for specific commands
pcli2 asset list --tenant "Demo Environment 1"
```

## Next Steps

After completing this quick start guide, explore these topics:

1. **[Geometric Matching](geometric-matching.md)** - Advanced techniques for finding similar assets
2. **[Configuration Management](#configuration)** - Customize PCLI2 to your workflow
3. **[Using UNIX Pipes with PCLI2](#using-unix-pipes-with-pcli2)** - Chain commands with other tools
4. **[Commands Reference](#commands-reference)** - Detailed information about all available commands

## Getting Help

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
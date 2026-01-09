# Quick Start Guide

This guide will help you get started with PCLI2 quickly by walking through common tasks.

## Table of Contents
- [Installation](#installation)
- [Authentication](#authentication)
- [Basic Navigation](#basic-navigation)
- [Working with Assets](#working-with-assets)
- [Geometric Matching](#geometric-matching)
- [Configuration](#configuration)
- [Context Management](#context-management)
- [Next Steps](#next-steps)
- [Getting Help](#getting-help)

## Authentication

Before using most PCLI2 commands, you need to authenticate with your Physna tenant. First, you'll need to obtain your API credentials.

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

### Logging In

Once you have your credentials, you can authenticate with PCLI2:

```bash
# Login with client credentials
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# Verify authentication
pcli2 auth get
```

## Basic Navigation

Learn to navigate your Physna tenant using PCLI2:

```bash
# List available tenants
pcli2 tenant list

# List folders in the root directory
pcli2 folder list

# List assets in a specific folder
pcli2 asset list --path /Root/MyFolder/
```

## Working with Assets

Asset management is a core function of PCLI2. These commands allow you to upload, retrieve, organize, and maintain your 3D models and other assets in Physna.

### Uploading Assets

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

### Viewing and Managing Assets

Use these commands to inspect and manage your assets:

```bash
# View asset details
pcli2 asset get --path /Root/MyFolder/model.stl

# Delete an asset
pcli2 asset delete --path /Root/MyFolder/model.stl
```

## Geometric Matching

Geometric matching is a powerful feature that allows you to find assets with similar 3D geometry in your Physna tenant. This is particularly useful for identifying duplicate parts, finding design variations, or discovering similar components across different projects.

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0

# Find matches for all assets in a folder (parallel processing)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 90.0 --format csv --progress
```

The threshold parameter controls the similarity requirement, where higher values (closer to 100) require closer matches. The progress flag provides visual feedback during long-running operations.

## Metadata Operations

Metadata is essential for organizing and searching your assets effectively. PCLI2 supports comprehensive metadata operations including creating, retrieving, updating, and deleting asset metadata. Metadata helps you categorize, filter, and find assets based on custom properties like material, supplier, weight, or any other characteristic relevant to your workflow.

### Creating and Updating Metadata

The `metadata create` command adds or updates a single metadata field on an asset. This is useful for setting specific properties on individual assets:

```bash
# Add or update a single metadata field on an asset
pcli2 asset metadata create --path "/Root/Folder/Model.stl" --name "Material" --value "Steel" --type "text"

# Add or update multiple metadata fields on an asset
pcli2 asset metadata create --path "/Root/Folder/Model.stl" --name "Weight" --value "15.5" --type "number"
```

### Retrieving Metadata

Use the `metadata get` command to view all metadata associated with an asset:

```bash
# Get all metadata for an asset
pcli2 asset metadata get --path "/Root/Folder/Model.stl"
```

### Deleting Metadata

The `metadata delete` command removes specific metadata fields from an asset without affecting other metadata on the same asset:

```bash
# Delete specific metadata fields from an asset
pcli2 asset metadata delete --path "/Root/Folder/Model.stl" --name "Material" --name "Weight"

# Delete metadata fields using comma-separated list
pcli2 asset metadata delete --path "/Root/Folder/Model.stl" --name "Material,Weight,Description"
```

### Metadata Inference

Metadata inference automatically applies metadata from a reference asset to geometrically similar assets. This feature helps you efficiently propagate metadata across similar assets, reducing manual work and ensuring consistency in your asset database:

```bash
# Apply specific metadata fields from a reference asset to similar assets
pcli2 asset metadata inference --path /Root/Folder/ReferenceModel.stl --name "Material,Cost" --threshold 85.0

# Apply metadata recursively to create chains of similar assets
pcli2 asset metadata inference --path /Root/Folder/ReferenceModel.stl --name "Category" --threshold 90.0 --recursive

# Apply multiple metadata fields with different thresholds
pcli2 asset metadata inference --path /Root/Folder/ReferenceModel.stl --name "Material" --name "Finish" --name "Supplier" --threshold 80.0
```

The metadata operations help you efficiently manage your asset metadata, whether you need to add, update, retrieve, or delete specific metadata fields, or propagate metadata across geometrically similar assets.

## Configuration

Manage your PCLI2 configuration:

```bash
# View current configuration
pcli2 config get

# Export configuration for backup
pcli2 config export --output my-config.yaml

# Import configuration from a file
pcli2 config import --file my-config.yaml
```

## Multi-Environment Configuration

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

# Switch to an environment (with interactive selection)
pcli2 config environment use

# Or switch to an environment by name
pcli2 config environment use --name development

# Get details of the active environment
pcli2 config environment get

# Get details of a specific environment
pcli2 config environment get --name production

# Reset all environment configurations
pcli2 config environment reset
```

Each environment can have its own:
- API base URL (for API calls)
- UI base URL (for comparison viewer links)
- Authentication URL (for OAuth2 token requests)

## Context Management

Work with multiple tenants efficiently:

```bash
# Set active context (tenant) using either tenant name or ID
pcli2 context set --tenant "Demo Environment 1"

# Or using tenant ID
pcli2 context set --tenant 123e4567-e89b-12d3-a456-426614174000

# View current context
pcli2 context get

# Clear active context
pcli2 context clear
```

## Next Steps

After completing this quick start guide, explore these topics:

1. **[Command Reference](commands/)** - Detailed information about all available commands
2. **[Batch Operations](batch.md)** - Learn to process multiple assets efficiently
3. **[Geometric Matching](geometric-matching.md)** - Advanced techniques for finding similar assets
4. **[Configuration](configuration.md)** - Customize PCLI2 to your workflow

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
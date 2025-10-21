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

Before using most PCLI2 commands, you need to authenticate with your Physna tenant:

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

## Geometric Matching

Find similar assets using PCLI2's powerful geometric matching:

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0

# Find matches for all assets in a folder (parallel processing)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 90.0 --format csv --progress
```

## Metadata Inference

Automatically apply metadata from a reference asset to geometrically similar assets using PCLI2's metadata inference capability:

```bash
# Apply specific metadata fields from a reference asset to similar assets
pcli2 asset metadata inference --path /Root/Folder/ReferenceModel.stl --name "Material,Cost" --threshold 85.0

# Apply metadata recursively to create chains of similar assets
pcli2 asset metadata inference --path /Root/Folder/ReferenceModel.stl --name "Category" --threshold 90.0 --recursive

# Apply multiple metadata fields with different thresholds
pcli2 asset metadata inference --path /Root/Folder/ReferenceModel.stl --name "Material" --name "Finish" --name "Supplier" --threshold 80.0
```

The metadata inference command helps you efficiently propagate metadata across geometrically similar assets, reducing manual work and ensuring consistency in your asset database.

## Configuration

Manage your PCLI2 configuration:

```bash
# View current configuration
pcli2 config show

# Set default tenant (using either tenant name or ID)
pcli2 config set tenant.default "Demo Environment 1"
# Or using tenant ID:
# pcli2 config set tenant.default 123e4567-e89b-12d3-a456-426614174000

# Export configuration for backup
pcli2 config export --output my-config.yaml
```

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
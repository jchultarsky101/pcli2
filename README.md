# PCLI2 - Physna Command Line Interface v2

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
   
   See the [Installation Guide](docs/installation.md) for detailed installation instructions.
   
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

### Cross-Platform Configuration

PCLI2 supports cross-platform environments through environment variables:

```bash
# Set custom configuration directory (cross-platform support)
export PCLI2_CONFIG_DIR="/custom/path/to/config"

# Set custom cache directory (for all cache files)
export PCLI2_CACHE_DIR="/custom/path/to/cache"

# Useful for WSL users running Windows executables
export PCLI2_CONFIG_DIR="/home/$USER/.pcli2"
export PCLI2_CACHE_DIR="/home/$USER/.pcli2/cache"
```

Environment Variables:
- `PCLI2_CONFIG_DIR`: Custom directory for configuration file (`config.yml`)
- `PCLI2_CACHE_DIR`: Custom directory for all cache files (asset, metadata, folder caches)

For detailed information about cross-platform configuration, see the [Cross-Platform Guide](docs/cross_platform.md).

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

For detailed command references, see the [Commands](#commands) section below.

### Troubleshooting

If you encounter issues:

1. **Authentication problems**: 
   - Ensure your client credentials are correct
   - Try logging out and back in: `pcli2 auth logout` followed by `pcli2 auth login`
   
2. **Permission issues**:
   - Verify your API key has the necessary permissions
   - Contact your Physna administrator if needed

3. **Network issues**:
   - Check your internet connection
   - Verify that Physna's API endpoints are accessible from your network

4. **For detailed debugging**:
   - Use the `--verbose` flag to see more detailed logs
   - Check the logs in your system's temporary directory if issues persist

## Documentation

Detailed documentation is available in the [docs](docs/) directory:

- [Installation Guide](docs/installation.md) - How to install and set up PCLI2
- [Quick Start Guide](docs/quickstart.md) - Getting started with basic commands
- [Geometric Matching](docs/geometric-matching.md) - Comprehensive guide to finding similar assets
- [Cross-Platform Configuration](docs/cross_platform.md) - Environment variables for cross-platform support
- [Documentation Deployment](docs/documentation_deployment.md) - Setting up documentation website with Oranda and GitHub Pages
- [Command Reference](docs/commands/) - Detailed reference for all commands (coming soon)

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

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For support, please contact the Physna development team or open an issue on GitHub.
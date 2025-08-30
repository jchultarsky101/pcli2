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

## Quick Start

### Installation

See the [Installation Guide](docs/installation.md) for detailed installation instructions.

```bash
# Clone the repository
git clone https://github.com/physna/pcli2.git
cd pcli2

# Build the project
cargo build --release

# The executable will be located at target/release/pcli2
```

### Authentication

Before using most commands, you need to authenticate with your Physna tenant:

```bash
# Login with client credentials
pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET

# Verify authentication
pcli2 auth get
```

### Basic Asset Operations

```bash
# List assets in the root folder
pcli2 asset list

# Create a new asset
pcli2 asset create --file path/to/your/file.stl --path /Root/MyFolder/

# Get asset details
pcli2 asset get --path /Root/MyFolder/MyAsset.stl

# Delete an asset
pcli2 asset delete --path /Root/MyFolder/MyAsset.stl
```

### Geometric Matching

Find similar assets using PCLI2's powerful geometric matching:

```bash
# Find matches for a single asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 85.0

# Find matches for all assets in a folder (parallel processing)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 90.0 --progress
```

## Documentation

Detailed documentation is available in the [docs](docs/) directory:

- [Installation Guide](docs/installation.md) - How to install and set up PCLI2
- [Quick Start Guide](docs/quickstart.md) - Getting started with basic commands
- [Geometric Matching](docs/geometric-matching.md) - Comprehensive guide to finding similar assets
- [Command Reference](docs/commands/) - Detailed reference for all commands (coming soon)

## Commands

The application uses a hierarchy of commands:

```
pcli2
├── asset
│   ├── create           Create a new asset by uploading a file
│   ├── create-batch     Create multiple assets by uploading files matching a glob pattern
│   ├── list             List all assets in a folder
│   ├── get              Get asset details
│   ├── delete           Delete an asset
│   ├── geometric-match  Find geometrically similar assets for a single asset
│   └── geometric-match-folder  Find geometrically similar assets for all assets in a folder
├── folder
│   ├── create           Create a new folder
│   ├── list             List all folders in a parent folder
│   ├── get              Get folder details
│   ├── delete           Delete a folder
│   └── update           Update folder details
├── tenant
│   ├── list             List all available tenants
│   └── get              Get current tenant information
├── auth
│   ├── login            Authenticate with client credentials
│   ├── logout           Clear authentication tokens
│   └── get              Get current authentication status
├── config
│   ├── show             Show current configuration
│   ├── set              Set configuration values
│   ├── export           Export configuration to a file
│   └── import           Import configuration from a file
├── context
│   ├── set              Set active context (tenant)
│   ├── get              Get current context
│   └── clear            Clear active context
└── help                 Show help information
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
# Installation Guide

This guide explains how to install and set up PCLI2 on your system.

## Prerequisites

Before installing PCLI2, ensure you have the following:

- **Rust toolchain** (latest stable version)
- **Cargo package manager** (usually installed with Rust)
- **Git** (for cloning the repository)

### Installing Rust

If you don't have Rust installed, you can install it using rustup:

```bash
# Install Rust using rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Reload your shell or source the rustup environment
source ~/.cargo/env

# Verify the installation
rustc --version
cargo --version
```

## Installation Methods

### Method 1: Building from Source (Recommended)

This method gives you the latest version of PCLI2:

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

### Method 2: Installing via Cargo

If you want to install PCLI2 directly from crates.io (once published):

```bash
# Install PCLI2 globally
cargo install pcli2

# Verify the installation
pcli2 --version
```

## Verifying the Installation

After installation, verify that PCLI2 is working correctly:

```bash
# Check the version
pcli2 --version

# View available commands
pcli2 --help

# View asset-related commands
pcli2 asset --help
```

## Updating PCLI2

To update PCLI2 when building from source:

```bash
# Navigate to your PCLI2 directory
cd /path/to/pcli2

# Pull the latest changes
git pull

# Rebuild the project
cargo build --release

# Copy the updated binary (if needed)
sudo cp target/release/pcli2 /usr/local/bin/
```

## Troubleshooting

### Common Issues

1. **Permission denied when copying binary**: Use `sudo` or copy to a directory you own
2. **Command not found**: Ensure the binary directory is in your PATH
3. **Build failures**: Make sure you have the latest stable Rust version

### Getting Help

If you encounter issues during installation:

1. Check that all prerequisites are met
2. Verify your Rust installation is working
3. Consult the [GitHub Issues](https://github.com/physna/pcli2/issues) page
4. Contact the Physna development team for support
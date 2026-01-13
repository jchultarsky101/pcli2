# Installation Guide

This guide explains how to install and set up PCLI2 on your system.

## Table of Contents
- [Prerequisites](#prerequisites)
- [Installation Methods](#installation-methods)
- [Verifying Installation](#verifying-installation)
- [Updating PCLI2](#updating-pcli2)
- [Troubleshooting](#troubleshooting)

## Prerequisites

Before installing PCLI2, ensure you have the following:

- **Rust toolchain** (latest stable version)
- **Cargo package manager** (usually installed with Rust)
- **Git** (for cloning the repository)

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

### Method 1: Pre-built Binaries (Recommended)

PCLI2 provides pre-built binaries for Windows, macOS, and Linux through GitHub Releases:

1. Visit the [Latest Release](https://github.com/physna/pcli2/releases/latest)
2. Download the appropriate installer or binary for your platform:
   - **Windows**: `pcli2-x86_64-pc-windows-msvc.msi` (Installer) or `pcli2-x86_64-pc-windows-msvc.zip` (ZIP)
   - **macOS**: `pcli2-installer.sh` (Universal script) or platform-specific archives
   - **Linux**: `pcli2-installer.sh` (Universal script) or `pcli2-x86_64-unknown-linux-gnu.tar.xz` (Archive)

#### Using the Universal Installer Script:
```bash
# Download and run the installer script
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/physna/pcli2/releases/latest/download/pcli2-installer.sh | sh
```

#### Manual Installation:
```bash
# Extract the archive (example for Linux)
tar -xf pcli2-x86_64-unknown-linux-gnu.tar.xz
sudo cp pcli2 /usr/local/bin/
```

### Method 2: Building from Source

This method gives you the latest development version of PCLI2:

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

### Method 3: Installing via Cargo

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

## Security and Authentication

PCLI2 uses your operating system's secure credential storage to protect your API credentials:

### System Keyring Integration

PCLI2 automatically integrates with your system's secure credential storage:

- **macOS**: Uses Keychain Services for secure credential storage
- **Windows**: Uses Windows Credential Manager
- **Linux**: Uses Secret Service API or KWallet

### First-Time Authorization

When you run PCLI2 for the first time after installation, you may see authorization prompts requesting permission to access the system keyring:

- **macOS**: A dialog will appear asking you to allow PCLI2 to access the keychain
- **Windows**: You may see a User Account Control (UAC) prompt
- **Linux**: You may be prompted to unlock the keyring

This is normal and required for secure credential storage. The authorization is typically remembered for subsequent runs.

### Credential Storage

Your API credentials (client ID, client secret, and access tokens) are securely encrypted and stored in your system's keyring. These credentials are environment-specific and stored separately for each configured environment.

### Authorization Prompts

- On **macOS**, you may see multiple authorization prompts during the first run as PCLI2 accesses different credential types (access token, client ID, client secret)
- The system will remember your authorization decision for future runs
- If you deny access, authentication commands will fail until you grant permission

## Troubleshooting

### Common Issues

1. **Permission denied when copying binary**: Use `sudo` or copy to a directory you own
2. **Command not found**: Ensure the binary directory is in your PATH
3. **Build failures**: Make sure you have the latest stable Rust version
4. **Keyring access denied**: Grant permission when prompted, or check system keyring settings
5. **Authentication failures**: Ensure keyring access is granted and credentials are properly stored

### Getting Help

If you encounter issues during installation:

1. Check that all prerequisites are met
2. Verify your Rust installation is working
3. Ensure keyring access permissions are granted
4. Consult the [GitHub Issues](https://github.com/physna/pcli2/issues) page
5. Contact the Physna development team for support
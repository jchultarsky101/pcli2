# PCLI2 - Physna Command Line Interface v2

**Documentation**: [https://jchultarsky101.github.io/pcli2/](https://jchultarsky101.github.io/pcli2/)

PCLI2 is a powerful command-line interface for the Physna public API, designed for advanced 3D geometry search and analysis. Built with an intuitive nested sub-command structure, it offers sensible defaults and comprehensive configuration management.

## 🚀 Quick Start

Get up and running with PCLI2 in minutes:

```bash
# 1. Authenticate with your Physna tenant (prompts for the client ID and secret)
pcli2 auth login

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
pcli2 asset delete --path /Home/Models/part.stl --yes
```

**Validate your setup:**
```bash
pcli2 config validate --verbose
```

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
- **Authentication** - OAuth2 client-credentials login; the token is renewed automatically before it expires
- **Confirmation Prompts** - Safety for destructive operations with `--yes` flag for scripting
- **Configuration Validation** - `config validate` command to verify setup before operations
- **Flexible Output Formats** - JSON, CSV, and tree views
- **Resumable Runs** - Interrupted downloads (`--resume`), uploads (`--skip-existing`) and folder matches (`--checkpoint`) pick up where they stopped
- **Performance Optimizations** - Concurrent operations and caching for faster processing
- **Structured Logging** - Debug with `--verbose`/`--quiet` flags or the `PCLI2_LOG_LEVEL` environment variable
- **Progress Tracking** - Enhanced progress bars with throughput and ETA
- **Dry Run Mode** - Preview changes with `--dry-run`: deletes (asset, folder, report, metadata field), uploads, `asset move`, `asset resolve-dependency` and `tenant metadata rename`
- **Built for Scripts** - `--no-input` turns any prompt into an error, `--error-format json` makes every error on stderr a JSON object with the exit code, `--safe-csv` guards CSV cells against spreadsheet formulas
- **Any API Endpoint** - `pcli2 api /tenants/{tenantId}/...` calls endpoints no command covers yet, with pcli2's login, retries and tenant
- **Diagnostics** - `pcli2 doctor` checks the whole setup in one screen; `--stats` reports API requests, retries and token renewals at exit
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
# Homebrew 6 refuses formulae from taps it has not been told to trust.
# Run this once per machine and user account:
brew trust jchultarsky101/pcli2

# Install PCLI2
brew install jchultarsky101/pcli2/pcli2
```

Without the trust step, `brew install` and every later `brew upgrade pcli2`
refuse with "Refusing to load formula ... from untrusted tap". Homebrew 5 does
not have the check, but the first `brew upgrade` after updating Homebrew will.
In CI, set `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` instead.

#### 🐳 Docker

Run PCLI2 in a container:

```bash
# Build the Docker image
docker build -t pcli2 .

# Keep the container's configuration and login in one host directory
# (it is not your host's pcli2 configuration; the container has its own)
mkdir -p ~/.pcli2-docker
alias pcli2-docker='docker run --rm -it -v "$PWD:/data" -v "$HOME/.pcli2-docker:/config" -e PCLI2_CONFIG_DIR=/config pcli2'

# Authenticate first; you are prompted for the client ID and secret
pcli2-docker auth login

# Then any command
pcli2-docker folder list --format tree
```

### Verification
```bash
pcli2 --version
```

To update an existing installation, or to upgrade from 1.x, see the [Installation guide](https://jchultarsky101.github.io/pcli2/book/installation.html#updating).

## 🔐 Authentication

Securely authenticate with your Physna tenant:

```bash
# First-time login (interactive): prompts for the client ID and secret,
# with masked input so the secret never lands in your shell history
pcli2 auth login

# First-time login (non-interactive, e.g. in CI): credentials from the environment
PCLI2_CLIENT_ID=... PCLI2_CLIENT_SECRET=... pcli2 auth login

# Subsequent logins (uses cached credentials)
pcli2 auth login

# Verify authentication
pcli2 auth get

# Check token expiration
pcli2 auth expiration
```

Credentials (the client ID, client secret and the current access token) are
stored in `dev_credentials.json` inside the configuration directory shown by
`pcli2 config get path`. The file is created with owner-only permissions
(`0600`) on macOS and Linux; on Windows it inherits the directory's ACLs.
The file is plain text, not encrypted. Treat it like any other secret file: do
not commit it. `pcli2 auth logout` removes only the access token; to remove the
client secret from a shared machine, delete the file. See
[Credentials and Security](https://jchultarsky101.github.io/pcli2/book/security.html).

## 📚 Documentation

The user guide at [https://jchultarsky101.github.io/pcli2/](https://jchultarsky101.github.io/pcli2/) covers everything in depth:

- **Getting started**: [Installation](https://jchultarsky101.github.io/pcli2/book/installation.html) (including updating and upgrading to 2.0), [Quick Start](https://jchultarsky101.github.io/pcli2/book/quickstart.html)
- **Guides**: [Geometric Matching](https://jchultarsky101.github.io/pcli2/book/geometric-matching.html), [Metadata Operations](https://jchultarsky101.github.io/pcli2/book/metadata-operations.html), [Metadata Inference](https://jchultarsky101.github.io/pcli2/book/metadata-inference.html), [Reports](https://jchultarsky101.github.io/pcli2/book/reports.html), [Downloading and Uploading Folders](https://jchultarsky101.github.io/pcli2/book/bulk-operations.html), [Scripting and Automation](https://jchultarsky101.github.io/pcli2/book/scripting.html) (output formats, exit codes, CI, `pcli2 api`)
- **Reference**: [Command Reference](https://jchultarsky101.github.io/pcli2/book/commands.html) (every command and alias), [Configuration and Environment Variables](https://jchultarsky101.github.io/pcli2/book/cross_platform.html), [Credentials and Security](https://jchultarsky101.github.io/pcli2/book/security.html), [Proxies, Certificates and Timeouts](https://jchultarsky101.github.io/pcli2/book/network.html)
- **Help**: [Troubleshooting](https://jchultarsky101.github.io/pcli2/book/troubleshooting.html); `pcli2 doctor` checks a setup in one screen, and every command has `--help` with examples

Contributing: see [CONTRIBUTING.md](https://github.com/jchultarsky101/pcli2/blob/main/CONTRIBUTING.md).

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

This project is licensed under the Apache License 2.0 - see the [LICENSE](https://github.com/jchultarsky101/pcli2/blob/main/LICENSE) file for details.

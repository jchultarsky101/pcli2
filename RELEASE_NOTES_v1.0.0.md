# PCLI2 v1.0.0 - Major Release 🎉

We're thrilled to announce the release of **PCLI2 v1.0.0**, a major milestone that brings comprehensive UX improvements and powerful developer tooling to the Physna Command Line Interface.

## 🚀 What's New

### Enhanced Command Ergonomics

**Top-level `environment` command** - No more nesting! Access environment management directly:
```bash
pcli2 env list          # Was: pcli2 config environment list
pcli2 env add -n dev    # Quick environment setup
```

**Unix-style aliases** across all commands for faster workflows:
```bash
pcli2 folder ls         # List folders
pcli2 asset ls          # List assets  
pcli2 asset rm          # Delete asset
pcli2 folder mv         # Move folder
pcli2 auth in           # Login
pcli2 auth exp          # Check token expiration
```

### Safety & Control

**Confirmation prompts** for destructive operations:
```bash
pcli2 asset delete --path /Root/Models/part.stl
# → Prompts: "Delete asset '/Root/Models/part.stl'? (y/N)"
```

**Skip prompts with `--yes`** for scripting and CI/CD:
```bash
pcli2 asset delete --path /Root/Models/part.stl --yes
```

**Disable color output** for logs and CI:
```bash
pcli2 folder list --no-color
PCLI2_NO_COLOR=1 pcli2 asset ls
```

### New Commands

**`config validate`** - Validate your setup before running operations:
```bash
pcli2 config validate              # Quick check
pcli2 config validate --verbose    # Detailed results
pcli2 config validate --api -v     # Test API connectivity
```

**Structured logging** - Debug with fine-grained control:
```bash
PCLI2_LOG_LEVEL=debug pcli2 folder download --path /Root/Models/
RUST_LOG=pcli2=trace pcli2 asset get --uuid xxx
```

### Progress Improvements

Enhanced progress bars now show **throughput** and **ETA**:
```
⠁ [00:15] [████████████░░░░░░░░] 45/120 (00:32) - 3.2 assets/s
```

### Examples in Help

Run `pcli2 --help` to see comprehensive examples of common workflows right in the CLI!

---

## 🛠️ Developer Tooling

### Docker Support

Run PCLI2 in containers with the new official Dockerfile:
```bash
# Build image
docker build -t pcli2 .

# Run commands
docker run --rm -v $(pwd):/data -v ~/.config/pcli2:/home/pcli2/.config/pcli2 pcli2 folder list
```

### Homebrew Formula

Install via Homebrew (macOS/Linux):
```bash
brew tap jchultarsky101/pcli2
brew install pcli2
```

### Benchmark Suite

Performance tracking with criterion:
```bash
cargo bench
```

### Enhanced CI/CD

GitHub Actions now tests on **Linux, macOS, and Windows** with:
- `cargo fmt` checks
- `cargo clippy` linting
- Coverage reporting via Codecov
- Multi-platform validation

---

## 📊 Migration Guide

### Breaking Changes
**None!** All changes are backward compatible.

### Recommended Updates

**Old:**
```bash
pcli2 config environment list
pcli2 config environment use -n development
```

**New (recommended):**
```bash
pcli2 env list
pcli2 env use -n development
```

Both still work!

---

## 📝 Full Changelog

### Added
- Top-level `environment` command (alias: `env`)
- Command aliases: `ls`, `rm`, `cat`, `dl`, `mv`, `add`, `ren`, `res`, `deps`, `thumb`, `in`, `out`, `token`, `clear`, `exp`
- Confirmation prompts for `asset delete` and `folder delete`
- `--yes` / `-y` global flag to skip confirmations
- `PCLI2_NO_COLOR` environment variable and `--no-color` flag
- `config validate` command for configuration validation
- Structured logging via `PCLI2_LOG_LEVEL` and `RUST_LOG`
- Progress bar throughput display (items/second)
- Examples in `--help` output
- Dockerfile and .dockerignore
- Homebrew formula
- Criterion-based benchmark suite
- Multi-platform CI/CD (Linux, macOS, Windows)
- Coverage reporting with cargo-llvm-cov

### Changed
- Improved CI/CD workflow with comprehensive quality checks
- Better error handling for confirmation failures

### Technical Details
- ✅ All changes backward compatible
- ✅ No breaking changes
- ✅ 151+ tests passing
- ✅ Zero clippy warnings

---

## 📦 Installation

### Pre-built Installers

**macOS/Linux:**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
```

**Windows PowerShell:**
```powershell
irm https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.ps1 | iex
```

### Homebrew (macOS/Linux)
```bash
brew tap jchultarsky101/pcli2
brew install pcli2
```

### Docker
```bash
docker build -t pcli2 .
```

### From Source
```bash
git clone https://github.com/jchultarsky101/pcli2.git
cd pcli2
cargo build --release
```

---

## 🙏 Acknowledgments

This release represents a major milestone in making PCLI2 more ergonomic, safe, and developer-friendly. Thank you to all contributors!

---

## 📚 Documentation

- **Full Documentation**: https://jchultarsky101.github.io/pcli2/
- **GitHub Repository**: https://github.com/jchultarsky101/pcli2
- **Report Issues**: https://github.com/jchultarsky101/pcli2/issues

---

**Full commit history**: https://github.com/jchultarsky101/pcli2/compare/v0.2.35...v1.0.0

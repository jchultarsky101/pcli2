# 🍺 Homebrew Tap - Quick Start

## For Users (Installation)

```bash
# Method 1: Tap then install
brew tap jchultarsky101/pcli2
brew install pcli2

# Method 2: Direct install (auto-taps)
brew install jchultarsky101/pcli2/pcli2

# Verify
pcli2 --version
```

---

## For Maintainers (Setup Steps)

### 1. Create Tap Repository

```bash
# Go to https://github.com/new
# Repository name: homebrew-pcli2
# MUST follow naming: homebrew-<formula_name>
# Must be PUBLIC
```

### 2. Copy Files to Tap Repo

```bash
cd /Users/julian/projects/physna

# Clone the tap repository
git clone git@github.com:jchultarsky101/homebrew-pcli2.git
cd homebrew-pcli2

# Copy formula from pcli2 repo
cp ../pcli2/.github/homebrew/pcli2.rb .
cp ../pcli2/.github/homebrew/README.md .
cp ../pcli2/LICENSE .

# Commit and push
git add pcli2.rb README.md LICENSE
git commit -m "feat: Add PCLI2 Homebrew formula v1.0.0"
git push origin main
```

### 3. Test the Tap

```bash
# Tap your repository
brew tap jchultarsky101/pcli2

# Verify
brew tap-info jchultarsky101/pcli2

# Install
brew install pcli2

# Test
pcli2 --version
brew test pcli2
```

---

## For Maintainers (Updating for New Release)

### When You Release v1.0.1:

```bash
# 1. Get SHA256 for new release
curl -sL https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.1.tar.gz \
  -o /tmp/pcli2.tar.gz
shasum -a 256 /tmp/pcli2.tar.gz
# Output: abc123...def456

# 2. Edit pcli2.rb in tap repo
# Update url: .../tags/v1.0.1.tar.gz
# Update sha256: abc123...def456

# 3. Commit and push
cd /Users/julian/projects/physna/homebrew-pcli2
git add pcli2.rb
git commit -m "chore: Update formula for v1.0.1"
git push origin main

# 4. Users can now upgrade
# brew update && brew upgrade pcli2
```

---

## Formula Details (v1.0.0)

- **URL**: https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.0.tar.gz
- **SHA256**: `ef1ebda08e92fee175b437ace43c9dcb0916906ee18ebf681f2759c063317c7a`
- **License**: Apache-2.0
- **Dependencies**: rust (build), pkg-config (build), openssl@3

---

## Repository Structure

```
homebrew-pcli2/
├── pcli2.rb           # ← REQUIRED: The formula
├── README.md          # Tap documentation
└── LICENSE            # License file
```

⚠️ **Formula MUST be in root directory** (not in a subfolder)

---

## Common Commands

```bash
# Tap info
brew tap-info jchultarsky101/pcli2

# Install
brew install pcli2

# Upgrade
brew upgrade pcli2

# Uninstall
brew uninstall pcli2

# Untap (remove tap)
brew untap jchultarsky101/pcli2

# Test formula
brew test pcli2

# Audit formula
brew audit --strict pcli2
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Formula not found | Check repo is public, formula in root |
| SHA256 mismatch | Recalculate with `shasum -a 256` |
| Build fails | Update Rust: `rustup update` |
| Tap not recognized | Verify naming: `homebrew-<formula>` |

---

## Full Documentation

See `HOMEBREW_TAP_SETUP.md` for complete guide with:
- Detailed setup instructions
- Best practices
- GitHub Actions CI setup
- Advanced troubleshooting

---

## Links

- **Tap Repository**: https://github.com/jchultarsky101/homebrew-pcli2
- **Main Repository**: https://github.com/jchultarsky101/pcli2
- **Homebrew Taps Docs**: https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap

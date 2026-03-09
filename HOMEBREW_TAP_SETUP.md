# 🍺 Homebrew Tap Setup Guide for PCLI2

This guide walks you through setting up and maintaining your own Homebrew tap for PCLI2.

---

## 📋 Prerequisites

- GitHub account with repository creation permissions
- Homebrew installed on macOS or Linux
- Git installed

---

## 🚀 Step 1: Create the Tap Repository

### 1.1 Create New GitHub Repository

1. Go to https://github.com/new
2. **Repository name**: `homebrew-pcli2`
   - ⚠️ **Important**: Must follow naming convention: `homebrew-<formula_name>`
3. **Description**: "Homebrew tap for PCLI2 - Physna Command Line Interface v2"
4. **Visibility**: Public (required for Homebrew taps)
5. **Initialize**: Check "Add a README file"
6. Click **Create repository**

### 1.2 Clone the Repository

```bash
cd /Users/julian/projects/physna
git clone git@github.com:jchultarsky101/homebrew-pcli2.git
cd homebrew-pcli2
```

---

## 📦 Step 2: Add the Formula

### 2.1 Copy Formula File

From the pcli2 repository:

```bash
# Copy the formula to your tap repository
cp /Users/julian/projects/physna/pcli2/.github/homebrew/pcli2.rb \
   /Users/julian/projects/physna/homebrew-pcli2/
```

### 2.2 Copy Supporting Files

```bash
# Copy README
cp /Users/julian/projects/physna/pcli2/.github/homebrew/README.md \
   /Users/julian/projects/physna/homebrew-pcli2/

# Copy LICENSE from main repo
cp /Users/julian/projects/physna/pcli2/LICENSE \
   /Users/julian/projects/physna/homebrew-pcli2/
```

### 2.3 Commit and Push

```bash
git add pcli2.rb README.md LICENSE
git commit -m "feat: Add PCLI2 Homebrew formula v1.0.0"
git push origin main
```

---

## ✅ Step 3: Verify the Tap

### 3.1 Test the Tap Locally

```bash
# Untap if previously tapped
brew untap jchultarsky101/pcli2

# Tap your repository
brew tap jchultarsky101/pcli2

# Verify tap is recognized
brew tap-info jchultarsky101/pcli2
```

Expected output:
```
jchultarsky101/pcli2
/usr/local/Homebrew/Library/Taps/jchultarsky101/homebrew-pcli2 (64 files, 320KB)
From: https://github.com/jchultarsky101/homebrew-pcli2
```

### 3.2 Install and Test

```bash
# Install from your tap
brew install jchultarsky101/pcli2/pcli2

# Verify installation
pcli2 --version
# Expected: pcli2 1.0.0

# Run tests
brew test pcli2
```

---

## 🔄 Step 4: Updating for New Releases

### 4.1 Calculate New SHA256

When you release a new version (e.g., v1.0.1):

```bash
# Download the new release tarball
curl -sL https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.1.tar.gz \
  -o /tmp/pcli2-1.0.1.tar.gz

# Calculate SHA256
shasum -a 256 /tmp/pcli2-1.0.1.tar.gz
```

Example output:
```
abc123...def456  /tmp/pcli2-1.0.1.tar.gz
```

### 4.2 Update the Formula

Edit `pcli2.rb`:

```ruby
class Pcli2 < Formula
  desc "Physna Command Line Interface v2"
  homepage "https://github.com/jchultarsky101/pcli2"
  url "https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.1.tar.gz"  # ← Update version
  sha256 "abc123...def456"  # ← Update with new SHA256
  license "Apache-2.0"
  
  # ... rest of formula
end
```

### 4.3 Commit and Push Update

```bash
git add pcli2.rb
git commit -m "chore: Update formula for v1.0.1"
git push origin main
```

### 4.4 Users Can Now Upgrade

Your users can upgrade with:
```bash
brew update
brew upgrade pcli2
```

---

## 🧪 Step 5: Testing the Formula

### Local Testing

```bash
# Install from source (tests the build)
brew install --build-from-source pcli2

# Run formula tests
brew test pcli2

# Check for common issues
brew audit --strict pcli2
```

### Fix Common Issues

**Audit warnings:**
```bash
brew audit --strict pcli2
# Follow the suggestions to fix any issues
```

**Build failures:**
```bash
# Verbose build to see what's wrong
brew install --verbose --build-from-source pcli2
```

---

## 🎯 Best Practices

### 1. Keep Formula Updated
- Update the formula within 24-48 hours of each PCLI2 release
- Always test before pushing updates

### 2. Version Consistency
- Formula version must match the git tag version
- SHA256 must match the release tarball exactly

### 3. Testing
- Always run `brew test pcli2` after updates
- Test on both macOS (Intel and Apple Silicon) if possible

### 4. Documentation
- Keep the README.md updated
- Document any breaking changes in release notes

### 5. Communication
- Announce new releases in your main repository
- Update installation instructions if needed

---

## 🔧 Advanced: GitHub Actions for Auto-Testing

Create `.github/workflows/test.yml` in your tap repository:

```yaml
name: Test Formula
on: [push, pull_request]

jobs:
  test:
    runs-on: macos-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install formula
        run: brew install ./pcli2.rb

      - name: Test formula
        run: brew test ./pcli2.rb

      - name: Audit formula
        run: brew audit --strict ./pcli2.rb || true
```

---

## 📊 Repository Structure

Your final tap repository should look like:

```
homebrew-pcli2/
├── .github/
│   └── workflows/
│       └── test.yml          # Optional: CI/CD for testing
├── pcli2.rb                  # The formula (REQUIRED)
├── README.md                 # Tap documentation
└── LICENSE                   # License file
```

---

## 🐛 Troubleshooting

### "Formula not found"

```bash
# Make sure repository is public
# Check formula is in root directory (not in a subfolder)
# Verify formula filename matches: pcli2.rb
```

### "SHA256 mismatch"

```bash
# Download the tarball manually
curl -sL https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.0.tar.gz -o test.tar.gz

# Verify SHA256
shasum -a 256 test.tar.gz

# Update formula with correct SHA256
```

### "Build fails with Rust error"

```bash
# Users should update Rust
rustup update

# Or install Rust via Homebrew
brew install rust
```

### "Tap not recognized"

```bash
# Verify repository name follows convention: homebrew-<formula>
# Make sure repository is public
# Try retapping:
brew untap jchultarsky101/pcli2
brew tap jchultarsky101/pcli2
```

---

## 📚 Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew Taps Documentation](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)
- [Homebrew API Documentation](https://rubydoc.brew.sh/)

---

## ✅ Checklist for Initial Setup

- [ ] Create `homebrew-pcli2` repository on GitHub
- [ ] Copy `pcli2.rb` formula to repository root
- [ ] Copy `README.md` to repository
- [ ] Copy `LICENSE` to repository
- [ ] Commit and push to main branch
- [ ] Test: `brew tap jchultarsky101/pcli2`
- [ ] Test: `brew install jchultarsky101/pcli2/pcli2`
- [ ] Test: `pcli2 --version`
- [ ] Test: `brew test pcli2`
- [ ] Add installation instructions to main PCLI2 README

---

## 🎉 Success!

Your Homebrew tap is now live! Users can install PCLI2 with:

```bash
brew tap jchultarsky101/pcli2
brew install pcli2
```

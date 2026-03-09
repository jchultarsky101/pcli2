# Homebrew Tap for PCLI2

This repository contains the Homebrew tap for PCLI2 (Physna Command Line Interface v2).

## Installation

### Step 1: Tap the Repository

```bash
brew tap jchultarsky101/pcli2
```

### Step 2: Install PCLI2

```bash
brew install pcli2
```

### Step 3: Verify Installation

```bash
pcli2 --version
```

## Uninstall

```bash
# Remove PCLI2
brew uninstall pcli2

# Untap the repository (optional)
brew untap jchultarsky101/pcli2
```

## Direct Installation (Without Tapping)

You can also install directly without tapping first:

```bash
brew install jchultarsky101/pcli2/pcli2
```

This automatically taps the repository, installs pcli2, and then untaps.

## Updating

When a new version is released:

```bash
brew update
brew upgrade pcli2
```

## Formula Information

```bash
brew info pcli2
```

## Shell Completions

Shell completions are automatically installed:

- **Bash**: `/opt/homebrew/etc/bash_completion.d/pcli2` (Apple Silicon)
- **Zsh**: `/opt/homebrew/share/zsh/site-functions/_pcli2` (Apple Silicon)
- **Fish**: `/opt/homebrew/share/fish/vendor_completions.d/pcli2.fish` (Apple Silicon)

For Intel Macs, replace `/opt/homebrew` with `/usr/local`.

## Troubleshooting

### Build Fails

If the build fails, try:

```bash
# Update Homebrew
brew update

# Upgrade Rust toolchain
rustup update

# Clean and reinstall
brew uninstall pcli2
brew cleanup
brew install pcli2 --verbose
```

### Missing Dependencies

```bash
brew install rust pkg-config openssl@3
```

### Permission Errors

```bash
sudo chown -R $(whoami) $(brew --prefix)
```

## For Maintainers

### Updating the Formula

1. **Create a new release** on GitHub with a git tag (e.g., `v1.0.1`)

2. **Calculate the SHA256** for the new release tarball:
   ```bash
   curl -sL https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.1.tar.gz -o pcli2.tar.gz
   shasum -a 256 pcli2.tar.gz
   ```

3. **Update `pcli2.rb`**:
   - Update `url` to point to the new tag
   - Update `sha256` with the new checksum
   - Update `version` if not using `url` with tag

4. **Commit and push**:
   ```bash
   git add pcli2.rb
   git commit -m "chore: Update formula for v1.0.1"
   git push origin main
   ```

5. **Test the installation**:
   ```bash
   brew uninstall pcli2
   brew install --build-from-source pcli2
   brew test pcli2
   ```

### Formula Location

The formula must be named `pcli2.rb` and located in the root of the repository for automatic discovery.

### Repository Structure

```
homebrew-pcli2/
├── pcli2.rb              # The formula file
├── LICENSE               # License file (optional but recommended)
└── README.md             # This file
```

### GitHub Actions (Optional)

Add CI to test the formula on updates:

```yaml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install formula
        run: brew install ./pcli2.rb
      - name: Test formula
        run: brew test ./pcli2.rb
```

## License

Apache License 2.0 - See [LICENSE](../LICENSE) for details.

## Links

- **PCLI2 Repository**: https://github.com/jchultarsky101/pcli2
- **Documentation**: https://jchultarsky101.github.io/pcli2/
- **Issues**: https://github.com/jchultarsky101/pcli2/issues

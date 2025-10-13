#!/bin/bash
# Test script for PCLI2 on WSL

echo "=== PCLI2 WSL Test Script ==="
echo

# Check if pcli2 is installed
if ! command -v pcli2 &> /dev/null; then
    echo "❌ ERROR: pcli2 is not installed or not in PATH"
    echo "   Please install pcli2 and make sure it's in your PATH"
    echo
    echo "   Installation options:"
    echo "   1. Build from source: cargo build --release && cp target/release/pcli2 ~/.local/bin/"
    echo "   2. Download pre-built binary from GitHub Releases"
    echo "   3. Install via cargo: cargo install pcli2 (once published)"
    echo
    exit 1
fi

echo "✅ pcli2 is installed"
echo "   Version: $(pcli2 --version)"
echo

# Test basic functionality
echo "=== Testing basic functionality ==="
echo

echo "Testing help command..."
if pcli2 --help > /dev/null 2>&1; then
    echo "✅ Help command works"
else
    echo "❌ ERROR: Help command failed"
    exit 1
fi
echo

echo "Testing configuration creation..."
if pcli2 config get > /dev/null 2>&1; then
    echo "✅ Configuration created successfully"
    echo "   Config file location: $(pcli2 config get path)"
else
    echo "❌ ERROR: Failed to create configuration"
    echo "   This might be due to permission issues with the config directory"
    exit 1
fi
echo

# Show configuration
echo "=== Current Configuration ==="
pcli2 config get
echo

echo "=== Test Summary ==="
echo "✅ All tests passed! PCLI2 is working correctly on WSL."
echo
echo "Next steps:"
echo "1. Authenticate with your Physna tenant:"
echo "   pcli2 auth login --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET"
echo
echo "2. List available tenants:"
echo "   pcli2 tenant list"
echo
echo "3. Explore your folders:"
echo "   pcli2 folder list --format tree"
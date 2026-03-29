# 🚀 Release Announcement: pcli2 v1.1.4

**Date:** March 29, 2026
**Version:** v1.1.4

---

## 📋 What's New

This is a **hotfix release** that resolves critical token expiration issues with the `asset metadata update-batch` command during long-running batch operations.

---

## 🐛 Bug Fixes

### Asset Metadata Update-Batch Token Expiration

**Problem:** Users running `pcli2 asset metadata update-batch` on large CSV files experienced authentication failures when their token expired mid-session:
- Token would expire during long batch operations (60+ minutes)
- Generic "Authentication failed" error with no clear remediation
- Batch continued processing after auth failure, causing cascading errors
- No warning that token might expire during the operation
- No summary of successful vs. failed operations

**Solution:** Proactive token management for long-running batch operations:
- ✅ **Pre-flight warning** - Warns if token may expire during batch operation
- ✅ **Proactive token refresh** - Refreshes token before each asset if within 2 minutes of expiration
- ✅ **Immediate stop on auth failure** - Batch stops immediately with clear remediation steps
- ✅ **Success/failure summary** - Shows count of successful and failed operations at end
- ✅ **Better error messages** - Clear guidance to run `pcli2 auth expiration` and re-authenticate

**Example Output:**
```
:warning: Warning: Token expires in approximately 10 minutes, but batch operation may take 25 minutes
  Token will be refreshed automatically if needed during processing.

Processing asset 50/100: /folder/asset50.stl
...
Batch operation completed: 98 successful, 2 failed
```

**If authentication fails:**
```
Authentication failed while updating metadata for asset '/folder/asset.stl': ...
  • Your access token may have expired
  • Try running 'pcli2 auth expiration' to check token status
  • Re-authenticate with 'pcli2 auth login' and retry the batch operation

Batch operation completed: 49 successful, 1 failed
```

---

## 📦 How to Update

### Homebrew (recommended)
```bash
brew update
brew upgrade pcli2
```

### Cargo
```bash
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.1.4
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.1.4
```

---

## 🔗 Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.1.4
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## 🙏 Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

---

**Happy batching! 🎉**

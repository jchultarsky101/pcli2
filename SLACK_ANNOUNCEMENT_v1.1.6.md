# 🚀 Release Announcement: pcli2 v1.1.6

**Date:** April 13, 2026
**Version:** v1.1.6

---

## 📋 What's New

This is a **bugfix release** with three reliability fixes: root folder path resolution, batch metadata token expiration handling, and empty value handling in batch metadata CSV files.

---

## 🐛 Bug Fixes

### Root Folder Path `"/"` Resolution

**Problem:** Commands that accept `--folder-path` would fail with a confusing error when users specified `"/"` (the root):

```
❌ Error: Folder '/' not found. Please verify the folder path exists in your tenant.
```

This happened even though `"/"` is always a valid path. The internal hierarchy lookup correctly returns `None` for root (since root is not a single folder node), but the calling code was treating that as "not found."

**Solution:**
- ✅ `asset list --folder-path "/"` now correctly lists root-level assets
- ✅ `asset list --folder-path "/" --recursive` now traverses all folders
- ✅ Root path is treated the same as omitting `--folder-path`

---

### Asset Metadata Batch — Token Expiration

**Problem:** Long-running `asset metadata update-batch` operations failed with authentication errors when the token expired mid-session, with no warning or summary of what was completed.

**Solution:** Proactive token management for batch operations:
- ✅ **Pre-flight warning** — Warns if token may expire before the batch finishes
- ✅ **Proactive refresh** — Refreshes token automatically before each asset if within 2 minutes of expiry
- ✅ **Immediate stop on auth failure** — Stops cleanly with clear remediation steps
- ✅ **Success/failure summary** — Shows completed vs. failed count at end of run

**Example:**
```
⚠️  Warning: Token expires in approximately 10 minutes, but batch operation may take 25 minutes.
    Token will be refreshed automatically if needed during processing.

Processing asset 50/100: /folder/asset50.stl
...
Batch operation completed: 98 successful, 2 failed
```

---

### Asset Metadata Batch — Empty Value Handling

**Problem:** CSV files with empty metadata values caused confusing `400 Bad Request` errors (displayed as "Operation conflict") instead of a clear message.

**Solution:**
- ✅ Empty values are silently skipped during CSV parsing (the API rejects empty strings)
- ✅ `400 Bad Request` errors now show the actual API error message instead of "Operation conflict"
- ✅ Debug logging shows which empty values were skipped (`--log-level debug`)

---

## 📦 How to Update

### Homebrew (recommended)
```bash
brew update
brew upgrade pcli2
```

### Cargo
```bash
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.1.6
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.1.6
```

---

## 🔗 Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.1.6
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## 🙏 Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

---

**Happy querying! 🎉**

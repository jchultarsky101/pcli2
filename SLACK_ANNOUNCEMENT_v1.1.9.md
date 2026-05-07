# 🚀 Release Announcement: pcli2 v1.1.9

**Date:** May 7, 2026
**Version:** v1.1.9

---

## 📋 What's New

This release adds a new **`--override`** flag to the `asset create` command, enabling seamless re-upload of assets that already exist at the target path.

---

## ✨ New Feature

### `--override` Flag for `asset create`

**Problem:** When uploading an asset to a path where an asset already exists, the API returns a conflict error. Previously, users had to manually delete the existing asset and then re-upload — a two-step process that's tedious in scripted workflows.

**Solution:** The new `--override` flag automates this:

```bash
# Upload normally (fails if asset exists)
pcli2 asset create --file part.stl --folder-path /MyFolder

# Automatically replace existing asset
pcli2 asset create --file part.stl --folder-path /MyFolder --override
```

**How it works:**
- ✅ Attempts the upload as normal
- ✅ If the asset already exists (HTTP 409) and `--override` is set, deletes the existing asset and re-uploads
- ✅ Without `--override`, behavior is unchanged — conflict errors are reported as before
- ✅ Other errors (file too large, unsupported type, etc.) are unaffected by the flag

---

## 📦 How to Update

### Homebrew (recommended)
```bash
brew update
brew upgrade pcli2
```

### Cargo
```bash
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.1.9
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.1.9
```

---

## 🔗 Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.1.9
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## 🙏 Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

---

**Happy querying! 🎉**

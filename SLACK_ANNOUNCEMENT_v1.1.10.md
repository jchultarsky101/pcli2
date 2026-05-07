# 🚀 Release Announcement: pcli2 v1.1.10

**Date:** May 7, 2026
**Version:** v1.1.10

---

## 📋 What's New

This release adds two new flags to the `asset create` command for seamless asset replacement with optional metadata preservation.

---

## ✨ New Features

### `--override` Flag for `asset create`

**Problem:** Re-uploading an asset to a path where one already exists creates a duplicate — two assets with different UUIDs at the same path. Previously, users had to manually find and delete the existing asset before uploading.

**Solution:** The `--override` flag handles this automatically:

```bash
# Replace an existing asset in one step
pcli2 asset create --file part.stl --folder-path /MyFolder --override
```

- ✅ Checks if an asset already exists at the target path before uploading
- ✅ If found, deletes the existing asset and uploads the new version
- ✅ If no existing asset, uploads normally
- ✅ Without `--override`, behavior is unchanged

---

### `--restore-metadata` Flag for `asset create`

**Problem:** When replacing an asset with `--override`, the old asset's metadata is lost because the asset is deleted and a new one is created.

**Solution:** The `--restore-metadata` flag preserves metadata through the replacement:

```bash
# Replace an asset and keep its metadata
pcli2 asset create --file part.stl --folder-path /MyFolder --override --restore-metadata
```

- ✅ Reads metadata from the existing asset before deletion
- ✅ Passes metadata directly in the upload request (single API call)
- ✅ Requires `--override` (enforced by CLI parser)
- ✅ Silently ignored if the existing asset has no metadata

---

## 📦 How to Update

### Homebrew (recommended)
```bash
brew update
brew upgrade pcli2
```

### Cargo
```bash
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.1.10
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.1.10
```

---

## 🔗 Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.1.10
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## 🙏 Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

---

**Happy querying! 🎉**

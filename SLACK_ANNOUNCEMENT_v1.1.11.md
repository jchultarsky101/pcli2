# Release Announcement: pcli2 v1.1.11

**Date:** May 12, 2026
**Version:** v1.1.11

---

## What's New

This release improves the reliability of the `--override` flag for `asset create` by handling asynchronous delete propagation on the Physna server.

---

## Bug Fix

### Retry on 409 Conflict During `--override`

**Problem:** When using `--override`, pcli2 deletes the existing asset and immediately uploads the replacement. On some tenants, the Physna server processes deletes asynchronously, so the upload can fail with a 409 Conflict because the old asset's path hasn't been freed yet. This is confusing because the user explicitly requested an override.

**Solution:** When `--override` is active and the upload returns 409, pcli2 now retries with exponential backoff:

- Up to 6 attempts (500ms, 1s, 2s, 4s, 8s, 16s delays)
- Only retries on 409 Conflict; other errors fail immediately
- Logs a warning on each retry so the user knows what's happening
- If all retries are exhausted, reports a clear error message

```bash
# This now handles async delete gracefully
pcli2 asset create --file part.stl --folder-path /MyFolder --override
```

No changes required to existing workflows. The retry is transparent.

---

## How to Update

### Homebrew (recommended)
```bash
brew update
brew upgrade pcli2
```

### Cargo
```bash
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.1.11
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.1.11
```

---

## Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.1.11
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

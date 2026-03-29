# 🚀 Release Announcement: pcli2 v1.1.3

**Date:** March 16, 2026  
**Version:** v1.1.3

---

## 📋 What's New

This release fixes critical issues with the `folder download` command error handling and reporting.

---

## 🐛 Bug Fixes

### Folder Download Error Handling

**Problem:** Users experienced confusing error reporting when downloading folders with `pcli2 folder download`:
- Error counts were incorrect when using `--concurrent` flag
- Progress bars were corrupted by error messages scrolling during download
- Final error message was generic and not actionable
- Statistics summary didn't appear when errors occurred
- Error messages disappeared from screen too quickly

**Solution:** Complete overhaul of error collection and display:
- ✅ All download tasks now complete before counting errors (accurate counts)
- ✅ Progress bars stay clean - errors printed AFTER they're cleared
- ✅ Statistics summary ALWAYS shown (success or failure)
- ✅ Detailed error list printed LAST so it remains visible on screen
- ✅ Shows actual API error messages with traceId for debugging
- ✅ Folder cache invalidated at start to ensure fresh data

**Example Output:**
```
📊 Download Statistics Report
===========================
✅ Successfully downloaded: 308
⏭️  Skipped (already existed): 0
❌ Failed downloads: 155
📁 Total assets processed: 463
⏳ Operation completed with errors!

📁 Files downloaded to destination directory: "Turck"

📋 Detailed Error List:
======================
⚠️  Failed to download asset '1012299887.step' (Physna path: Turck/1012299887.step): 
   Conflict: Asset not found - the asset may have been deleted or the path is incorrect. 
   API Response: {"message":"Asset not found","traceId":"abc123..."}
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
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.1.3
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.1.3
```

---

## 🔗 Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.1.3
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## 🙏 Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

---

**Happy downloading! 🎉**

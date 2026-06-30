# Release Announcement: pcli2 v1.3.0

**Date:** June 30, 2026
**Version:** v1.3.0

---

## What's New

This release adds a new `asset similarity` command for directly comparing two specific assets and retrieving their pairwise match scores.

---

## New Feature

### `asset similarity` — Pairwise Match Scores

**What it does:** Compares two specific assets and returns the geometric (and, when enabled, volumetric) match scores between them, backed by the Physna `GetMatchScores` API endpoint.

Unlike `asset geometric-match` — which searches your whole tenant for assets similar to a single reference — `asset similarity` compares two assets you *already know*.

Each asset can be identified by **either** a UUID **or** a path; paths are resolved to UUIDs automatically:

```bash
# Compare two assets by path
pcli2 asset similarity \
  --reference-path /Root/Models/block1.stl \
  --candidate-path /Root/Models/block2.stl

# Mix identifiers (reference by UUID, candidate by path)
pcli2 asset similarity \
  --reference-uuid 123e4567-e89b-12d3-a456-426614174000 \
  --candidate-path /Root/Models/block2.stl

# CSV output with headers
pcli2 asset similarity \
  --reference-path /Root/Models/block1.stl \
  --candidate-path /Root/Models/block2.stl \
  --format csv --headers
```

Highlights:

- **JSON (default) and CSV** output, with an optional `--headers` row for CSV
- Reports overall, forward, and reverse geometric match percentages, plus a UI **comparison URL**
- The **volumetric** score is included only when volumetric scoring is enabled for your tenant (contact Physna sales to enable it)
- Also available under the alias `pcli2 asset match-scores`

Both assets must be 3D models in a finished state, and they must be different assets.

---

## How to Update

### Homebrew (recommended)
```bash
brew update
brew upgrade pcli2
```

### Cargo
```bash
cargo install --git https://github.com/jchultarsky101/pcli2.git --tag v1.3.0
```

### Docker
```bash
docker pull ghcr.io/jchultarsky101/pcli2:v1.3.0
```

---

## Links

- **Release Notes:** https://github.com/jchultarsky101/pcli2/releases/tag/v1.3.0
- **Full Changelog:** https://github.com/jchultarsky101/pcli2/blob/main/CHANGELOG.md
- **Documentation:** https://github.com/jchultarsky101/pcli2/blob/main/README.md

---

## Reporting Issues

Found a bug or have feedback? Please report it at:
https://github.com/jchultarsky101/pcli2/issues

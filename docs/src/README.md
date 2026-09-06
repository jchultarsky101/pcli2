# PCLI2 Documentation

PCLI2 is the command-line client for the Physna public API: 3D geometry search,
asset and folder management, metadata, and batch operations, with output that is
built for scripts (JSON, CSV, Excel) as much as for people.

## Chapters

- [Installation Guide](installation.md) - Installers for every platform, updating, building from source
- [Quick Start Guide](quickstart.md) - Logging in, choosing a tenant, the everyday commands
- [Geometric Matching](geometric-matching.md) - Finding similar assets, for one asset or a whole folder
- [Metadata Operations](metadata-operations.md) - Reading, writing and bulk-loading metadata
- [Metadata Inference](metadata-inference.md) - Propagating metadata to geometrically similar assets
- [Scripting and Automation](scripting.md) - Machine-friendly output, JSON errors, exit codes, prompts, resumable runs, retries, CI
- [Cross-Platform Configuration](cross_platform.md) - Environment variables and file locations
- [Documentation Deployment](documentation_deployment.md) - How this site is built

## Features

- Nested sub-commands with short aliases (`pcli2 asset ls`, `pcli2 folder rm`)
- Multiple environments (production, staging) and multiple tenants
- OAuth2 client-credentials login with automatic token renewal
- Asset upload, download, listing, deletion, reprocessing and thumbnails
- Folder tree listing, creation, renaming, moving, bulk upload and download
- Geometric, part and visual matching, single-asset or folder-wide, with CSV and
  Excel reports
- Metadata fields: create, read, delete, bulk-load from CSV, infer from matches
- Resumable runs: downloads skip files already on disk, uploads skip assets
  already in the folder, folder matches continue from a checkpoint file
- Retries with backoff for transient failures, and exit codes that say what
  went wrong
- Built for scripts: `--no-input`, `--error-format json`, `--safe-csv`, and
  `pcli2 doctor` for checking a setup

Start with the [Installation Guide](installation.md), then the
[Quick Start Guide](quickstart.md).

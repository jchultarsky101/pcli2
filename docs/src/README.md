# PCLI2 Documentation

PCLI2 is the command-line client for the Physna public API: 3D geometry search,
asset and folder management, metadata, and batch operations, with output that is
built for scripts (JSON, CSV, Excel) as much as for people.

## Chapters

**Getting started**
- [Installation](installation.md) - Installers for every platform, updating, upgrading to 2.0, building from source
- [Quick Start](quickstart.md) - Logging in, choosing a tenant, the everyday commands

**Guides**
- [Geometric Matching](geometric-matching.md) - Finding similar assets, for one asset or a whole folder, and groups of duplicates
- [Metadata Operations](metadata-operations.md) - Reading, writing and bulk-loading metadata
- [Metadata Inference](metadata-inference.md) - Propagating metadata to geometrically similar assets
- [Reports](reports.md) - Listing, downloading, explaining and starting the server-side reports
- [Downloading and Uploading Folders](bulk-operations.md) - Bulk transfers, speed, resuming
- [Scripting and Automation](scripting.md) - Output formats, JSON errors, exit codes, prompts, retries, CI, `pcli2 api`

**Reference**
- [Command Reference](commands.md) - Every command and alias
- [Configuration and Environment Variables](cross_platform.md) - Environment variables and file locations
- [Credentials and Security](security.md) - Where the login is kept and what leaves your machine
- [Proxies, Certificates and Timeouts](network.md) - Corporate networks and firewalls

**Help**
- [Troubleshooting](troubleshooting.md) - By exit code, and the common situations

## Features

- Nested sub-commands with short aliases (`pcli2 asset ls`, `pcli2 folder rm`)
- Multiple environments (production, staging) and multiple tenants
- OAuth2 client-credentials login with automatic token renewal
- Asset upload, download, listing, deletion, reprocessing, thumbnails, and
  server-side failure diagnostics for assets that did not process
- Folder tree listing, creation, renaming, moving, bulk upload and download
- Geometric, part and visual matching, single-asset or folder-wide, with CSV and
  Excel reports
- Metadata fields: create, read, delete, bulk-load from CSV, infer from matches;
  rename and delete registered fields, see which assets use them and how many
  have none
- Reports: list, download (CSV/XLSX), explain a failed one, start a duplication
  report and wait for it
- Resumable runs: downloads skip files already on disk, uploads skip assets
  already in the folder, folder matches continue from a checkpoint file
- Retries with backoff for transient failures, and exit codes that say what
  went wrong
- Built for scripts: `--no-input`, `--error-format json`, `--safe-csv`, and
  `pcli2 doctor` for checking a setup

Start with the [Installation Guide](installation.md), then the
[Quick Start Guide](quickstart.md).

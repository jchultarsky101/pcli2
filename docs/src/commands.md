# Command Reference

An overview of every command and its aliases. `pcli2 <command> --help` shows each
command's flags and examples in full.

## Aliases

Quick reference for all available command aliases:

### Tenant Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 tenant list` | `pcli2 tenant ls` |
| `pcli2 tenant use` | `pcli2 tenant select` |
| `pcli2 tenant get` | `pcli2 tenant current` |
| `pcli2 tenant clear` | `pcli2 tenant unset` |
| `pcli2 tenant metadata list` | `pcli2 tenant metadata ls` |

### Folder Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 folder list` | `pcli2 folder ls` |
| `pcli2 folder delete` | `pcli2 folder rm` |
| `pcli2 folder get` | `pcli2 folder cat` |
| `pcli2 folder create` | `pcli2 folder add` |
| `pcli2 folder move` | `pcli2 folder mv` |
| `pcli2 folder rename` | `pcli2 folder ren` |
| `pcli2 folder resolve` | `pcli2 folder res` |
| `pcli2 folder download` | `pcli2 folder dl` |
| `pcli2 folder geometric-match` | `pcli2 folder geometric-search`, `pcli2 folder gm` |
| `pcli2 folder part-match` | `pcli2 folder part-search`, `pcli2 folder pm` |
| `pcli2 folder visual-match` | `pcli2 folder visual-search`, `pcli2 folder vm` |

### Asset Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 asset list` | `pcli2 asset ls` |
| `pcli2 asset delete` | `pcli2 asset rm` |
| `pcli2 asset get` | `pcli2 asset cat` |
| `pcli2 asset create` | `pcli2 asset upload` |
| `pcli2 asset create-batch` | `pcli2 asset upload-batch` |
| `pcli2 asset download` | `pcli2 asset dl` |
| `pcli2 asset dependencies` | `pcli2 asset deps` |
| `pcli2 asset dependency-diff` | `pcli2 asset deps-diff` |
| `pcli2 asset thumbnail` | `pcli2 asset thumb` |
| `pcli2 asset geometric-match` | `pcli2 asset geometric-search`, `pcli2 asset gm` |
| `pcli2 asset part-match` | `pcli2 asset part-search`, `pcli2 asset pm` |
| `pcli2 asset visual-match` | `pcli2 asset visual-search`, `pcli2 asset vm` |
| `pcli2 asset text-match` | `pcli2 asset text-search`, `pcli2 asset tm` |
| `pcli2 asset similarity` | `pcli2 asset match-scores` |
| `pcli2 asset diagnose` | `pcli2 asset failure`, `pcli2 asset why` |
| `pcli2 asset move` | `pcli2 asset mv` |
| `pcli2 asset resolve-dependency` | `pcli2 asset resolve-dep` |
| `pcli2 asset metadata create` | `pcli2 asset metadata update` |
| `pcli2 asset metadata create-batch` | `pcli2 asset metadata update-batch` |
| `pcli2 asset metadata delete` | `pcli2 asset metadata rm` |

### Authentication Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 auth` | `pcli2 a` |
| `pcli2 auth login` | `pcli2 auth in` |
| `pcli2 auth logout` | `pcli2 auth out` |
| `pcli2 auth get` | `pcli2 auth token` |
| `pcli2 auth clear-token` | `pcli2 auth clear` |
| `pcli2 auth expiration` | `pcli2 auth exp` |

### Environment Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 environment` | `pcli2 env` |
| `pcli2 environment list` | `pcli2 env ls` |
| `pcli2 environment remove` | `pcli2 env rm` |

### Report Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 report list` | `pcli2 report ls` |
| `pcli2 report download` | `pcli2 report dl` |
| `pcli2 report diagnose` | `pcli2 report why` |
| `pcli2 report delete` | `pcli2 report rm` |

### Other Commands
| Full Command | Alias |
|-------------|-------|
| `pcli2 user list` | `pcli2 user ls` |
| `pcli2 cache clear` | `pcli2 cache clean` |

## Commands

### Asset Commands

Manage individual assets in your Physna tenant.

```
pcli2 asset create           # Upload a file as an asset
pcli2 asset create-batch     # Upload multiple files as assets using glob patterns (--skip-existing to re-run)
pcli2 asset list             # List assets in a folder with optional recursive listing (--recursive)
pcli2 asset inventory        # List complete inventory of all assets in the tenant
pcli2 asset counts           # Show asset health report with counts by state, type, and structure
pcli2 asset get              # Get asset details (--uuid may be repeated to fetch several in one request)
pcli2 asset download         # Download an asset
pcli2 asset delete           # Delete an asset
pcli2 asset move             # Move an asset to another folder, or to the root (--folder-path /)
pcli2 asset dependencies     # Get dependencies for an asset
pcli2 asset dependency-diff  # Diff the dependency trees of two assets
pcli2 asset resolve-dependency  # Link a missing dependency of an assembly to an existing asset
pcli2 asset geometric-match  # Find geometrically similar assets
pcli2 asset part-match       # Find part matches for an asset
pcli2 asset visual-match     # Find visually similar assets (--limit N, default 100; --threshold N size filter, default 80)
pcli2 asset text-match       # Find assets using text search (--limit N, default 1000; warns on stderr when more matches exist)
pcli2 asset similarity       # Match scores between two specific assets (--reference-* and --candidate-*)
pcli2 asset reprocess        # Reprocess an asset to refresh its analysis
pcli2 asset diagnose         # Explain why an asset failed to process (server-side failure diagnostics)
pcli2 asset thumbnail        # Download asset thumbnail
pcli2 asset metadata         # Manage asset metadata (get, create, create-batch, delete, inference)
```

#### Asset List Command

The `asset list` command lists assets in a folder with various filtering and formatting options.

```bash
# List assets in a specific folder
pcli2 asset list --folder-path "/Home/Models/"

# List assets recursively (including all subfolders)
pcli2 asset list --folder-path "/Home/Models/" --recursive

# Output in CSV format with headers
pcli2 asset list --folder-path "/Home/Models/" --format csv --headers

# Include metadata in JSON output
pcli2 asset list --folder-path "/Home/Models/" --format json --metadata

# Force refresh folder cache before listing (useful after folder changes)
pcli2 asset list --reload --folder-path "/Home/Models/" --format csv
```

#### Asset Inventory and Counts Commands

The `asset inventory` and `asset counts` commands both retrieve all assets across the entire tenant (not scoped to a folder). They differ in output:

- `asset inventory` outputs the full list of assets (same format as `asset list`)
- `asset counts` outputs an aggregated health report with counts by processing state, file type, and structure

```bash
# Full inventory in CSV with headers
pcli2 asset inventory --format csv --headers

# Full inventory in JSON with metadata
pcli2 asset inventory --format json --metadata

# Asset health report in JSON
pcli2 asset counts --format json

# Asset health report in CSV
pcli2 asset counts --format csv --headers
```

#### Asset Dependency Diff Command

The `asset dependency-diff` command (alias `deps-diff`) compares the recursive dependency trees of two assemblies — a **reference** and a **candidate** — and reports which parts differ between them.

Each asset is identified by either its UUID or its path, consistent with other asset commands. Provide exactly one identifier per side:

- `--reference-uuid` or `--reference-path`
- `--candidate-uuid` or `--candidate-path`

The comparison is **structural**: the two trees are walked in parallel and their nodes are matched by **filename**. It is **presence-only** — a part is reported as present in both (`=`), only in the reference (`-`), or only in the candidate (`+`); occurrence counts are not compared. If a whole subassembly is present on only one side, its entire subtree is marked accordingly.

Supported output formats: `tree` (default view of the merged diff), `json`, and `csv`.

```bash
# Diff two assemblies by path, rendered as a tree
pcli2 asset dependency-diff \
  --reference-path /Parts/AssemblyA.SLDASM \
  --candidate-path /Parts/AssemblyB.SLDASM \
  --format tree

# Diff by UUID, as pretty JSON
pcli2 asset deps-diff \
  --reference-uuid 00000000-0000-0000-0000-000000000001 \
  --candidate-uuid 00000000-0000-0000-0000-000000000002 \
  --format json --pretty

# Diff as CSV with headers (STATUS, ASSEMBLY_PATH, FILENAME, ASSET_UUID, ASSET_STATE)
pcli2 asset deps-diff \
  --reference-path /Parts/AssemblyA.SLDASM \
  --candidate-path /Parts/AssemblyB.SLDASM \
  --format csv --headers
```

Example tree output:

```
dependency diff: reference `/Parts/AssemblyA.SLDASM` vs candidate `/Parts/AssemblyB.SLDASM`
├─ (=) gearbox.sldasm [finished] (…)
│  ├─ (=) shaft.stl [finished] (…)
│  ├─ (-) bearing-v1.stl [finished] (…)
│  └─ (+) bearing-v2.stl [finished] (…)
├─ (-) bracket-old.stl [finished] (…)
└─ (+) bracket-new.stl [finished] (…)

Legend: (=) in both  (-) only in reference  (+) only in candidate
Summary: 2 common, 2 only in reference, 2 only in candidate
```

If either asset cannot be resolved, the command reports which input (reference or candidate) failed. An asset that is not an assembly is treated as having no dependencies.

#### Asset Metadata Commands

Manage custom properties for assets.

```
pcli2 asset metadata get           # Get metadata for an asset
pcli2 asset metadata create        # Add metadata to an asset
pcli2 asset metadata delete        # Delete specific metadata fields from an asset
pcli2 asset metadata create-batch  # Create metadata for multiple assets from a CSV file
pcli2 asset metadata inference     # Apply metadata from a reference asset to geometrically similar assets
```

#### Asset Metadata Create-Batch CSV Formats

The `asset metadata create-batch` command accepts two CSV layouts. The layout is auto-detected from the header row (any column starting with `metadata:` selects the UI format), or can be forced with `--csv-format classic|ui`.

**Classic (vertical) format** — one row per asset+field combination:

```
ASSET_PATH,NAME,VALUE,TYPE
/Home/Folder/Model1.stl,Material,Steel,text
/Home/Folder/Model1.stl,Weight,15.5,number
/Home/Folder/Model2.ipt,Inventory Qty,42,number
/Home/Folder/Model2.ipt,Supplier Link,https://example.com/,url
```

- The first row must contain the headers `ASSET_PATH,NAME,VALUE`, optionally followed by `TYPE`
- Each row represents a single metadata field assignment for an asset
- If an asset has multiple metadata fields to update, include multiple rows with the same `ASSET_PATH` but different `NAME` and `VALUE` combinations
- **`TYPE`** is optional (default `text`; one of `text`, `number`, `boolean`, `url`) and only sets the type used when *registering a new* field — for an existing field, its registered type is authoritative and `TYPE` is ignored
- A leading `/Home` in `ASSET_PATH` (Physna's name for the root folder) is treated as the root, so `/Home/NX/part.prt` and `NX/part.prt` refer to the same asset

**UI (horizontal) format** — one row per asset, as exported by the Physna web UI's bulk metadata upload:

```
path,id,metadata:Material,metadata:Color
/Home/Folder/Model1.stl,,Steel,Blue
/Home/Folder/Model2.ipt,123e4567-e89b-12d3-a456-426614174000,Aluminum,Red
```

- `path` is the asset path; the optional `id` column holds the asset UUID and takes precedence over the path when present
- Each `metadata:<field name>` column sets one metadata field (the prefix is stripped)
- Unrecognized columns are ignored with a warning

In both formats, empty values are skipped by default (existing metadata is left untouched). Pass `--delete-if-empty` to instead delete a metadata field from the asset when the file contains an empty value for it.

**Automatic type coercion**: because a CSV cell is text, each value is coerced to the field's registered type before upload — a `number` field receives `18` (not `"18"`), a `boolean` field accepts `true`/`false`/`yes`/`no`/`1`/`0`, and `text`/`url` fields store the value as a string. A value that cannot be represented as the field's type (e.g. `N/A` for a number field) is a type conflict and is reported as an error.

**General requirements** (both formats):
- The file must be UTF-8 encoded
- Values containing commas, quotes, or newlines must be enclosed in double quotes
- Empty rows will be ignored

**Error Handling**:

By default, the batch stops on the first error and reports how many assets were processed successfully. Pass `--continue-on-error` to skip the offending asset — whether its `ASSET_PATH` cannot be resolved or its metadata update fails (including a type conflict) — and continue with the remaining assets. Authentication failures always terminate the batch regardless of the flag.

To generate a starting CSV of the fields already registered in the tenant (with their types), use `pcli2 tenant metadata list --format csv --headers` (see [Tenant Commands](#tenant-commands)).

### Folder Commands

Manage folder structures and bulk operations.

```
pcli2 folder list             # List folder structure (defaults to root path if no path/UUID specified)
pcli2 folder create           # Create a new folder
pcli2 folder get              # Get folder details
pcli2 folder delete           # Delete a folder
pcli2 folder rename           # Rename a folder
pcli2 folder move             # Move a folder to a new parent folder
pcli2 folder resolve          # Resolve a folder path to its UUID
pcli2 folder download         # Download all assets in a folder (--resume skips files already on disk)
pcli2 folder upload           # Upload all assets from a local directory to a Physna folder (--skip-existing)
pcli2 folder dependencies     # Get dependencies for all assembly assets in folder
pcli2 folder geometric-match  # Find geometrically similar assets for all assets in folder (--checkpoint FILE to resume)
pcli2 folder part-match       # Find part matches for all assets in folder (--checkpoint FILE to resume)
pcli2 folder visual-match     # Find visually similar assets for all assets in folder (--limit N, default 100; --threshold N size filter, default 80)
pcli2 folder thumbnail        # Download thumbnails for all assets in a folder
```

**Important Note**: Folder paths are **case-insensitive**. You can use any capitalization when specifying folder paths (e.g., `/Home/Models`, `/home/models`, `/HOME/MODELS` all refer to the same folder). This matches the behavior of Windows file systems and provides a more user-friendly experience.

#### Folder Resolve Command

The `folder resolve` command resolves a folder path to its UUID, which can be useful for scripting or debugging.

```bash
# Resolve a folder path to UUID
pcli2 folder resolve --folder-path "/Home/MyFolder"

# Force refresh folder cache before resolving (useful if folder was recently recreated)
pcli2 folder resolve --reload --folder-path "/Home/MyFolder"
```

#### Folder List Command

The `folder list` command allows you to list folders in your Physna tenant. When no folder path or UUID is specified, it defaults to listing the root folder.

```bash
# List all folders in the root directory (default behavior)
pcli2 folder list

# List folders in a specific path
pcli2 folder list --folder-path "/Home/MyFolder"

# List folders using folder UUID
pcli2 folder list --folder-uuid 123e4567-e89b-12d3-a456-426614174000

# List folders with specific output format
pcli2 folder list --format tree
```

**Key Features**:
- **Default Root Path**: When no folder identifier is provided, defaults to the root path (`/`)
- **Mutual Exclusivity**: You can specify either `--folder-path` or `--folder-uuid`, but not both
- **Flexible Output**: Supports JSON, CSV, and tree formats
- **Folder Hierarchy**: Shows the complete folder structure when using tree format
- **Cache Refresh**: Use `--reload` flag to force refresh of folder cache before listing

```bash
# Force refresh folder cache before listing (useful after folder changes)
pcli2 folder list --reload

# Combine with other flags
pcli2 folder list --reload --format tree
```

### Tenant Commands

Manage tenant-level operations.

```
pcli2 tenant list           # List all tenants
pcli2 tenant get            # Get the active tenant (alias: current)
pcli2 tenant use            # Set the active tenant
pcli2 tenant clear          # Clear the active tenant
pcli2 tenant state          # Get asset state counts for the current tenant
pcli2 tenant failures       # List recent failures (assets, reports, part-finder reports), newest first
pcli2 tenant usage          # Searches, downloads, active users, ... over a period, plus asset counts by type
pcli2 tenant metadata list      # List the tenant's registered metadata fields with their types
pcli2 tenant metadata rename    # Rename a field (--name OLD --new-name NEW); values on assets are kept
pcli2 tenant metadata delete    # Delete a field (--name NAME); --force also removes its values from every asset
pcli2 tenant metadata assets    # List the assets that have a value for a field (--name NAME, --limit N)
pcli2 tenant metadata coverage  # How many assets carry any metadata at all (counts and percentage)
pcli2 tenant metadata missing   # List the assets with no metadata at all (--folder-path, --extension, --limit)
```

The `tenant metadata list` output (CSV) uses the same header as the classic `create-batch` input (`ASSET_PATH,NAME,VALUE,TYPE`) with `NAME` and `TYPE` filled from the registry and `ASSET_PATH`/`VALUE` blank, so it can be saved and turned into a batch-upload template:

```bash
pcli2 tenant metadata list --format csv --headers > fields.csv
```

#### Tenant Usage

`tenant usage` reports how much the tenant used Physna over a period of UTC
days: searches (by type), compares, downloads, uploads, reports (by type),
distinct active users and per-feature counts, plus how many assets of each type
the tenant holds now (demo assets uploaded by Physna are not counted). It needs
the tenant admin role.

The period is the last 30 days unless `--days N`, `--from` or `--to`
(`YYYY-MM-DD`) say otherwise, up to 366 days. The CSV and table output has one
`CATEGORY,NAME,COUNT` row per number, so a count Physna adds later arrives as a
new row, not a new column; `--daily` prints one row per day instead.

```bash
# The last 30 days
pcli2 tenant usage

# A quarter, as JSON (the API's fields plus from, to and assetTypes)
pcli2 tenant usage --from 2026-07-01 --to 2026-09-30 --format json

# Active users per day over the last week
pcli2 tenant usage --days 7 --daily --format csv --headers --columns DATE,ACTIVE_USERS

# One number in a script: searches in the last 30 days
pcli2 tenant usage --format csv | awk -F, '$1=="activity" && $2=="searches" {print $3}'
```

#### Finding Out Why an Asset Failed

`tenant state` and `asset counts` say *how many* assets failed; `tenant failures`
says *which* (newest first, with the tenant-wide totals in the JSON output), and
`asset diagnose` asks the server *why*. The server searches its ingestion logs
on demand, so the answer is only as durable as log retention.

```bash
# The failed assets, newest first (--kind report / part-finder-report for the others)
pcli2 tenant failures --kind asset --format csv --headers

# Why one of them failed, by UUID from that listing or by path
pcli2 asset diagnose --uuid 5db69606-661e-4019-a9b4-a4d122fc0f9e
```

```json
{
  "assetPath": "/Home/Rail/04_Automatic_drain_cock_module.step",
  "assetUuid": "5db69606-661e-4019-a9b4-a4d122fc0f9e",
  "assetState": "failed",
  "status": "found",
  "kind": "internal",
  "summary": "Processing failed inside Physna.",
  "traceId": "d55c845b-80f1-44fb-95df-256aebf7bf3c",
  "occurredAt": "2026-09-07T14:02:02.356Z"
}
```

- `kind: user` means the failure is yours to fix and `summary` says what
  (an unsupported format, for example). `kind: internal` means it failed on
  Physna's side: quote the `traceId` to support.
- `status: not-found` (exit 0) means the asset is not failed, or the log entry
  has aged out; a note on stderr says which. Reprocessing produces a fresh entry
  if the asset fails again.
- On a deployment without failure log search the command exits 68. `pcli2 doctor`
  has a `diagnostics` line that says up front whether the lookup is available.

CSV columns: `ASSET_PATH,ASSET_STATE,STATUS,KIND,SUMMARY,TRACE_ID,OCCURRED_AT,ASSET_UUID`.

### Report Commands

Reports (duplication, simplification, custom) are jobs that run on the server and then hold data you can download. See the [Reports](docs/src/reports.md) chapter for details.

```
pcli2 report list      # List the tenant's reports, newest first (--type, --status, --limit)
pcli2 report get       # Show one report: status, progress, settings (--id)
pcli2 report download  # Download a COMPLETED report's data (--id, --format csv|xlsx, -o PATH)
pcli2 report diagnose  # Explain why a report failed, like 'asset diagnose' (--id)
pcli2 report delete    # Delete a report; asks for confirmation unless --yes (--id, --dry-run)
pcli2 report create    # Start a duplication report over folders; --wait follows it to the end
```

```bash
# Start a duplication report over a folder, wait for it, then download the data as Excel
pcli2 report create --name "Brackets" --folder-path "/Home/Parts/Brackets" --wait --format csv --headers
pcli2 report download --id <ID> --format xlsx -o brackets.xlsx

# Why did a report fail? (tenant failures lists the failed ones)
pcli2 report diagnose --id <ID>
```

### Authentication Commands

Manage authentication with your Physna tenant.

```
pcli2 auth login        # Authenticate with Physna using client credentials
pcli2 auth logout       # Logout and clear session
pcli2 auth get          # Get current access token
pcli2 auth clear-token  # Clear the cached access token
pcli2 auth expiration   # Show token expiration time
```

### Configuration Commands

Manage PCLI2 configuration settings.

```
pcli2 config get path      # Print the path of the configuration file
pcli2 config validate      # Check the configuration and credentials (--api also calls the API)
pcli2 config export        # Export configuration to file (-o/--output)
pcli2 config import        # Import configuration from file (-i/--input)
pcli2 env ...              # Manage environment configurations (below)
```

#### Environment Configuration Commands

Manage multiple Physna environment configurations.

```
pcli2 env add --name <name>     # Add a new environment configuration
pcli2 env add -n <name>         # Short form of add with name
pcli2 env use --name <name>     # Switch to an environment
pcli2 env use -n <name>         # Short form of use with name
pcli2 env remove --name <name>  # Remove an environment
pcli2 env remove -n <name>      # Short form of remove with name
pcli2 env list                  # List all environments
pcli2 env reset                 # Reset all environment configurations
pcli2 env get --name <name>     # Get environment details
pcli2 env get -n <name>         # Short form of get with name
```

### Other Commands

Additional utility commands.

```
pcli2 user list      # List the users of the active tenant
pcli2 user get <id>  # Get one user's details
pcli2 cache clear    # Clear cached data (all caches, or --folder / --metadata / --tenant)
pcli2 completions    # Generate shell completions for various shells
pcli2 man            # Generate man pages for all commands
pcli2 doctor         # Check the local setup: binary, configuration, credentials, token, tenant, caches, connectivity
```

#### Cache Management Command

The `cache` command provides tools for managing PCLI2's local cache. Caching improves performance by storing folder hierarchies, metadata definitions, and tenant lists locally, but can sometimes contain stale data.

```bash
# Clear all caches (folder, metadata, and tenant)
pcli2 cache clear

# Clear all caches without confirmation prompt (useful for scripts)
pcli2 cache clear --yes

# Clear specific cache types
pcli2 cache clear --folder      # Clear only folder hierarchy cache
pcli2 cache clear --metadata    # Clear only metadata field cache
pcli2 cache clear --tenant      # Clear only tenant list cache

# Combine flags to clear multiple specific caches
pcli2 cache clear --folder --metadata --yes

# Use the alias 'clean'
pcli2 cache clean --yes
```

**When to clear the cache:**
- After deleting and recreating folders with the same name
- When folder paths return unexpected results
- After making bulk changes to folder structure
- When troubleshooting "folder not found" errors
- Periodically to ensure fresh data from the API

**Note:** The `--reload` flag on commands like `folder list`, `folder resolve`, and `asset list` provides a convenient way to refresh the folder cache for a single operation without clearing all caches.

```bash
# Example: List assets with fresh folder data
pcli2 asset list --reload --folder-path "/Home/Models/" --format csv
```

#### Shell Completions

Generate shell completions for various shells to enable tab completion for PCLI2 commands.

```bash
# Generate shell completions for various shells
pcli2 completions bash      # Generate bash completions
pcli2 completions zsh       # Generate zsh completions
pcli2 completions fish      # Generate fish completions
pcli2 completions powershell # Generate PowerShell completions
pcli2 completions elvish    # Generate Elvish completions

# Install bash completions (system-wide)
sudo pcli2 completions bash > /etc/bash_completion.d/pcli2
# Or for user-specific installation:
mkdir -p ~/.local/share/bash-completion/completions
pcli2 completions bash > ~/.local/share/bash-completion/completions/pcli2

# Install zsh completions (MacOS/Linux)
# For system-wide installation (requires sudo):
sudo pcli2 completions zsh > /usr/local/share/zsh/site-functions/_pcli2
# For user-specific installation:
mkdir -p ~/.zsh/completions  # Standard location (note the 's' at the end)
pcli2 completions zsh > ~/.zsh/completions/_pcli2
# Then add to your ~/.zshrc:
# fpath=(~/.zsh/completions $fpath)
# autoload -U compinit && compinit

# Alternative location (if your system uses the singular form):
# mkdir -p ~/.zsh/completion
# pcli2 completions zsh > ~/.zsh/completion/_pcli2

# Alternative zsh installation method (works on most systems):
pcli2 completions zsh > ~/.zfunc/_pcli2
# Add the following line to your ~/.zshrc:
# fpath+=~/.zfunc; autoload -U compinit && compinit

# Install fish completions
# For user-specific installation:
mkdir -p ~/.config/fish/completions
pcli2 completions fish > ~/.config/fish/completions/pcli2.fish

# Install PowerShell completions
# Add to your PowerShell profile:
pcli2 completions powershell > pcli2-completion.ps1
# Then dot source it in your PowerShell profile:
# . "/path/to/pcli2-completion.ps1"
```

#### Man Pages

Generate Unix man pages for PCLI2 and every subcommand (one page per
command, e.g. `pcli2-folder-delete.1`):

```bash
# Install for the current user, then read pages by name.
# Note: `man` only searches the directories listed by `manpath` - it never
# looks in the current directory, so the pages must be installed (or opened
# by path) to be found. If ~/.local/share/man is not in your `manpath`
# output, use a directory that is (e.g. /usr/local/share/man/man1).
mkdir -p ~/.local/share/man/man1
pcli2 man --output-dir ~/.local/share/man/man1
man pcli2
man pcli2-asset-create-batch

# Or read a generated file directly without installing
pcli2 man --output-dir ./man
man ./man/pcli2.1
```

The pages are a snapshot of the CLI at generation time - re-run the install
command after upgrading PCLI2 to refresh them.

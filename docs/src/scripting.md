# Scripting and Automation

PCLI2 is designed to work well in shell scripts, cron jobs, and CI/CD
pipelines. This page collects the features that matter when no human is
watching the terminal.

## Table of Contents

- [Machine-Friendly Output](#machine-friendly-output)
- [Skipping Prompts](#skipping-prompts)
- [Dry Run Mode](#dry-run-mode)
- [Exit Codes](#exit-codes)
- [Verbosity Control](#verbosity-control)
- [Automatic Retries](#automatic-retries)
- [Update Notifications](#update-notifications)
- [CI/CD Example](#cicd-example)

## Machine-Friendly Output

Colors, spinners, and progress bars are shown only when the output is a
terminal. When you pipe or redirect output, you get clean text
automatically:

```bash
# Clean JSON, no ANSI escape codes
pcli2 asset list --folder-path "/Root/Models/" --format json | jq '.[].name'

# CSV with headers for spreadsheets
pcli2 asset list --folder-path "/Root/Models/" --format csv --headers > assets.csv
```

To disable colors explicitly, use the `--no-color` flag or set the
`NO_COLOR` (or `PCLI2_NO_COLOR`) environment variable.

## Skipping Prompts

Destructive commands ask for confirmation when run interactively. In
scripts, pass `--yes`:

```bash
pcli2 folder delete --folder-path "/Root/Scratch/" --force --yes
pcli2 cache clear --yes
```

Authentication credentials can be passed as flags for non-interactive use:

```bash
pcli2 auth login --client-id "$PHYSNA_CLIENT_ID" --client-secret "$PHYSNA_CLIENT_SECRET"
```

## Dry Run Mode

Preview destructive or bulk operations without changing anything on the
server. Supported by `asset delete`, `folder delete`, `asset create`,
`asset create-batch`, and `folder upload`:

```bash
# List exactly which files a batch upload would send, and where
pcli2 asset create-batch --files "build/*.stl" --folder-path "/Root/CI Builds/" --dry-run

# Confirm what a forced folder delete would remove
pcli2 folder delete --folder-path "/Root/Old Projects/" --force --dry-run
```

## Exit Codes

PCLI2 uses distinct exit codes (following BSD `sysexits.h` conventions
where possible) so scripts can react to specific failure classes:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 64 | Command line usage error |
| 65 | Data format error |
| 66 | Cannot open input file |
| 67 | Resource not found |
| 68 | Service unavailable |
| 69 | Temporary failure |
| 70 | Internal software error |
| 71 | Operating system error |
| 78 | Configuration error |
| 100 | Authentication error |
| 101 | Network communication error |
| 102 | Remote API error |

```bash
pcli2 asset get --path "/Root/Models/part.stl" --format json
case $? in
  0)   echo "found" ;;
  100) pcli2 auth login ;;
  101) echo "network problem, try again later" ;;
  *)   echo "failed" ;;
esac
```

## Verbosity Control

The global `--quiet` flag limits diagnostics to errors; `--verbose` (`-v`)
enables debug-level logging. Both work on every command and take
precedence over the `PCLI2_LOG_LEVEL` and `RUST_LOG` environment
variables:

```bash
pcli2 --quiet asset create-batch --files "build/*.stl" --folder-path "/Root/CI Builds/"
PCLI2_LOG_LEVEL=trace pcli2 folder list
```

## Automatic Retries

Transient failures (network timeouts, connection errors, and HTTP
408/429/502/503/504 responses) are retried automatically with exponential
backoff, honoring the server's `Retry-After` header. The default is 2
retries; tune it with `PCLI2_MAX_RETRIES` (0 disables retries):

```bash
PCLI2_MAX_RETRIES=5 pcli2 folder download --folder-path "/Root/Models/" --output ./downloads
```

The request timeout defaults to 30 minutes (large model files take that
long to transfer). Lower it with `PCLI2_TIMEOUT` (seconds) if you prefer
fast failures over patience:

```bash
PCLI2_TIMEOUT=120 pcli2 asset list --folder-path "/Root/Models/"
```

Note that timeouts abort-and-retry only read requests (GETs); a timed-out
write is never retried automatically because the server may have already
processed it.

## Update Notifications

In interactive terminal sessions, PCLI2 prints a one-line hint on stderr
when a newer release is available (checked at most once per day). The
check never runs in CI (detected via the `CI` environment variable) or
when output is redirected. To opt out entirely:

```bash
export PCLI2_NO_UPDATE_CHECK=1
```

## CI/CD Example

A GitHub Actions job that uploads build artifacts to Physna:

```yaml
jobs:
  upload-models:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install pcli2
        run: curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jchultarsky101/pcli2/releases/latest/download/pcli2-installer.sh | sh
      - name: Authenticate
        run: pcli2 auth login --client-id "${{ secrets.PHYSNA_CLIENT_ID }}" --client-secret "${{ secrets.PHYSNA_CLIENT_SECRET }}"
      - name: Upload models
        run: |
          pcli2 tenant use --name my-tenant
          pcli2 asset create-batch --files "build/*.stl" \
            --folder-path "/Root/CI Builds/" --quiet --format json
```

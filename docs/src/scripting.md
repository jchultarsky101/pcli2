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
pcli2 asset list --folder-path "/Home/Models/" --format json | jq '.[].name'

# CSV with headers for spreadsheets
pcli2 asset list --folder-path "/Home/Models/" --format csv --headers > assets.csv
```

To disable colors explicitly, use the `--no-color` flag or set the
`NO_COLOR` (or `PCLI2_NO_COLOR`) environment variable.

The same rules apply to diagnostics on stderr: warnings and `--verbose`
logs captured with `2> warnings.log` are plain text with no ANSI escape
codes, so they can be grepped and parsed directly.

### Machine-Readable Errors

With `--error-format json` (or `PCLI2_ERROR_FORMAT=json`) everything pcli2
writes to stderr is one JSON object per line: errors, their hints, warnings,
`--verbose` log lines and the `--stats` summary. The last object of a failed
run carries the exit code and its class:

```bash
$ pcli2 --error-format json asset delete --path /Home/Parts/nope.stl
{"level":"ERROR","code":67,"kind":"not_found","message":"API error: Path not found: /Home/Parts/nope.stl"}
$ echo $?
67
```

| Field | Meaning |
|-------|---------|
| `level` | `ERROR`, `WARN`, `INFO` or `DEBUG`, matching the log lines |
| `code` | The process exit code, on the final error object |
| `kind` | The failure class: `usage`, `data`, `no_input`, `not_found`, `unavailable`, `temp_fail`, `software`, `os`, `config`, `auth`, `network`, `api` |
| `message` | The same text the human-readable error would show |
| `hint` | What to do about it, when pcli2 knows |
| `http_status` | The HTTP status behind an API error, when there is one |
| `steps` | Remediation steps, on errors that list them |

Progress bars and the upload/download statistics reports are not JSON; leave
`--progress` off in scripts that parse stderr.

```bash
# Read the exit code and message of a failed run
if ! out=$(pcli2 --error-format json asset list --folder-path "/Nope" 2>&1 >/dev/null); then
  echo "$out" | tail -n 1 | jq -r '"\(.kind): \(.message)"'
fi
```

## Skipping Prompts

Destructive commands ask for confirmation when run interactively. In
scripts, pass `--yes`:

```bash
pcli2 folder delete --folder-path "/Home/Scratch/" --force --yes
pcli2 cache clear --yes
```

A prompt that cannot be shown is refused rather than answered for you: when
stdin is not a terminal, or `--no-input` (or `PCLI2_NO_INPUT=1`) is set, a
command that would have to ask exits 64 and says which flag to pass instead.
`tenant use` and `env use` without `--name` fail the same way instead of
showing a menu nobody can answer. Set `PCLI2_NO_INPUT=1` in CI so a forgotten
`--yes` fails fast rather than hanging on a prompt.

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
pcli2 asset create-batch --files "build/*.stl" --folder-path "/Home/CI Builds/" --dry-run

# Confirm what a forced folder delete would remove
pcli2 folder delete --folder-path "/Home/Old Projects/" --force --dry-run
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
| 69 | Temporary failure |
| 70 | Internal software error |
| 71 | Operating system error |
| 78 | Configuration error |
| 100 | Authentication error |
| 101 | Network communication error |
| 102 | Remote API error |

A usage error rejected by the argument parser also exits 64. Batch commands that
finished with some items failed, and folder matches whose report would be
incomplete, exit 69.

```bash
pcli2 asset get --path "/Home/Models/part.stl" --format json
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
pcli2 --quiet asset create-batch --files "build/*.stl" --folder-path "/Home/CI Builds/"
PCLI2_LOG_LEVEL=trace pcli2 folder list
```

## Automatic Retries

Transient failures (network timeouts, connection errors, and HTTP
408/429/502/503/504 responses) are retried automatically with exponential
backoff, honoring the server's `Retry-After` header. The default is 2
retries; tune it with `PCLI2_MAX_RETRIES` (0 disables retries):

```bash
PCLI2_MAX_RETRIES=5 pcli2 folder download --folder-path "/Home/Models/" --output ./downloads
```

The request timeout defaults to 30 minutes (large model files take that
long to transfer). Lower it with `PCLI2_TIMEOUT` (seconds) if you prefer
fast failures over patience:

```bash
PCLI2_TIMEOUT=120 pcli2 asset list --folder-path "/Home/Models/"
```

Note that timeouts abort-and-retry only read requests (GETs); a timed-out
write is never retried automatically because the server may have already
processed it.

## Resuming Interrupted Runs

Long runs in a script should be written so that a retry does not redo finished
work:

```bash
# Downloads skip files already on disk
pcli2 folder download --folder-path "/Home/Parts" --output ./parts --resume

# Uploads skip files whose name is already in the folder
pcli2 asset create-batch --files "parts/*.stl" --folder-path "/Home/Parts" --skip-existing

# Folder matches record each completed search; the same command continues the run
pcli2 folder geometric-match --folder-path "/Home/Parts" --recursive \
  --checkpoint parts.jsonl --format csv --headers > parts.csv
```

The checkpoint file is removed when the report is written, so a loop that
retries until the command exits 0 needs no cleanup of its own.

## Request Statistics

Add `--stats` to any command to get one line on stderr at exit with the number
of API requests made, how many were retried, how many token renewals happened,
and the elapsed time. It is the quickest way to see whether a batch is doing
more work than it should:

```bash
pcli2 --stats asset metadata create-batch --csv-file metadata.csv
# 📊 5,102 API request(s), 2 retried, 1 token renewal(s) in 3m41s
```

## Checking the Setup

`pcli2 doctor` prints the local state in one screen (binary and PATH,
configuration, environment, credentials, token expiry, tenant, caches, API and
auth-server reachability, update state) and exits non-zero when something is
wrong: 78 for a local problem, 68 when a server cannot be reached.
`--format json` makes it machine-readable.

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
            --folder-path "/Home/CI Builds/" --quiet --format json
```

# Reports

Physna builds reports on the server: duplication reports (groups of similar
assets), simplification reports and custom reports. They are jobs that run for
a while and then hold data you can download. `pcli2 report` lists, inspects,
downloads, explains and deletes them, and starts duplication reports.

## Table of Contents
- [Listing Reports](#listing-reports)
- [One Report](#one-report)
- [Downloading the Data](#downloading-the-data)
- [Why a Report Failed](#why-a-report-failed)
- [Deleting a Report](#deleting-a-report)
- [Creating a Duplication Report](#creating-a-duplication-report)
- [Exit Codes](#exit-codes)

## Listing Reports

```bash
# Every report, newest first
pcli2 report list

# Only finished duplication reports, as CSV
pcli2 report list --type DUPLICATION --status COMPLETED --format csv --headers

# The five most recent
pcli2 report list --limit 5
```

```csv
ID,NAME,TYPE,STATUS,PROGRESS,GROUPS,MIN_THRESHOLD,MAX_THRESHOLD,CREATED_AT,UPDATED_AT,CREATOR
f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1,Custom 9/21/2026,CUSTOM,COMPLETED,100,1,80,100,2026-09-21T18:33:10.382Z,2026-09-21T18:33:11.254Z,someone@physna.com
98e51614-3e5b-4eee-957b-fa19699535ce,Custom 9/21/2026,CUSTOM,FAILED,20,0,80,100,2026-09-21T18:13:25.987Z,2026-09-22T14:16:24.033Z,someone@physna.com
```

`--type` takes `DUPLICATION`, `SIMPLIFICATION` or `CUSTOM`; `--status` takes
`PENDING`, `RUNNING`, `COMPLETED`, `FAILED` or `CANCELLED`. The JSON output is
an array of the full report records, including the folders, extensions and
thresholds the report was created with.

## One Report

```bash
pcli2 report get --id f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1 --format json --pretty
```

## Downloading the Data

Only a `COMPLETED` report has data. The file is streamed through a temporary
`.part` file and renamed into place once it has arrived whole, like an asset
download, so an interrupted transfer never leaves a truncated file.

```bash
# CSV under the report's name (here: "Custom 9_21_2026.csv")
pcli2 report download --id f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1 --format csv

# Excel workbook, to a path of your choice
pcli2 report download --id f046e5b8-c08b-4bbb-b685-c6fd85e4d1b1 --format xlsx -o dupes.xlsx
```

A report that is still `PENDING` or `RUNNING` is refused with exit 69 (try
again later); one that `FAILED` or was `CANCELLED` is refused with exit 64.

## Why a Report Failed

`tenant failures` lists failed reports beside failed assets. `report diagnose`
asks the server why, from its job logs, exactly like `asset diagnose` does for
an asset:

```bash
pcli2 report diagnose --id 98e51614-3e5b-4eee-957b-fa19699535ce
```

```json
{
  "reportId": "98e51614-3e5b-4eee-957b-fa19699535ce",
  "reportName": "Custom 9/21/2026",
  "reportStatus": "FAILED",
  "status": "found",
  "kind": "internal",
  "summary": "Processing failed inside Physna.",
  "traceId": "d55c845b-80f1-44fb-95df-256aebf7bf3c",
  "occurredAt": "2026-09-21T18:13:26.000Z"
}
```

`status: not-found` (exit 0) means the report did not fail, or the log entry
has aged out; a note on stderr says which. On a deployment without failure log
search the command exits 68.

## Deleting a Report

```bash
pcli2 report delete --id 98e51614-3e5b-4eee-957b-fa19699535ce --dry-run
pcli2 report delete --id 98e51614-3e5b-4eee-957b-fa19699535ce --yes
```

Without `--yes` the command asks for confirmation (and refuses with exit 64
under `--no-input`).

## Creating a Duplication Report

A duplication report groups assets that match each other between two
similarity thresholds (80% to 100% by default). It searches the folders you
name, subfolders included.

```bash
# Start it and print the record (status PENDING)
pcli2 report create --name "Brackets" --folder-path /Home/Parts/Brackets --extension stl

# Start it, follow it to the end, then download the data
pcli2 report create --name "Brackets" --folder-path /Home/Parts/Brackets --wait --format csv --headers
pcli2 report download --id <ID> --format xlsx
```

Options:

| Flag | Meaning |
|------|---------|
| `--folder-path P` | A folder to search, subfolders included (repeatable). `/` means the root's own assets (see `--include-home`). |
| `--exclude-folder-path P` | A subfolder of a searched folder to leave out, with its own subfolders (repeatable). |
| `--extension E` | Only assets with this file extension, e.g. `stl` (repeatable). |
| `--min-threshold N` / `--max-threshold N` | Similarity range, 0 to 100 (default 80 to 100). |
| `--exclude-assemblies` | Leave assemblies out of the groups. |
| `--exclude-exact-duplicates` | Leave out 100% matches that share a file name. |
| `--include-home` | Include the root folder's own assets. |
| `--wait` | Poll until the report is `COMPLETED`, `FAILED` or `CANCELLED`, printing progress on stderr. |
| `--poll-interval S` | Seconds between polls (default 5). |

With `--wait`, one failed poll is retried and a credential failure stops the
command at once, as with every other long-running pcli2 command. A report that
ended `FAILED` or `CANCELLED` exits 69 and points at `report diagnose`.

## Exit Codes

| Code | When |
|------|------|
| 0 | Done; for `diagnose`, `found` and `not-found` alike |
| 64 | Download of a `FAILED` or `CANCELLED` report; bad thresholds; a refused confirmation prompt |
| 67 | The report id does not exist |
| 68 | `diagnose` on a deployment without failure log search |
| 69 | Download of a report still running; `create --wait` that ended `FAILED` or `CANCELLED` |

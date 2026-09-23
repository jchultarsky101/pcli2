# Downloading and Uploading Folders

`folder download`, `folder upload` and `folder thumbnail` work on every asset of a
folder and its subfolders (`--folder-path /` is the whole tenant). They run four
items at a time by default, show progress bars in a terminal, and end with a
summary on stderr.

Without `--continue-on-error` the first failure stops the run: items that had not
started are counted as "Not attempted". With it, every item is tried and the run
still exits 69 when any failed. `folder upload` refuses files that already exist
in the destination before uploading anything (pass `--skip-existing` to skip them
instead). `folder download` downloads finished assets and assemblies waiting for
a missing part, and lists the assets it left out because they were still
processing or had failed.

## Speed and Rate Limits

Optimize operations for large datasets:

```bash
# Concurrent downloads (faster for many files)
pcli2 folder download --folder-path "/Home/LargeFolder/" --concurrent 5 --progress

# Add delays to prevent rate limiting
pcli2 folder download --folder-path "/Home/Folder/" --delay 2

# Continue on errors
pcli2 folder download --folder-path "/Home/Folder/" --continue-on-error

# Continue past unresolvable asset paths in a metadata batch
pcli2 asset metadata create-batch --input "metadata.csv" --continue-on-error

# Download thumbnails for all assets in a folder
pcli2 folder thumbnail --folder-path "/Home/Folder/" --progress --concurrent 3
```

## Resuming Interrupted Runs

Every long-running command can pick up where it stopped:

```bash
# Download: skip files that already exist in the destination
pcli2 folder download --folder-path "/Home/LargeFolder/" --resume --progress

# Upload: skip files whose name is already in the target folder
pcli2 folder upload --input ./parts --folder-path "/Home/Parts" --skip-existing
pcli2 asset create-batch --input "parts/*.stl" --folder-path "/Home/Parts" --skip-existing

# Folder match: record each completed search, re-run the same command to continue
pcli2 folder geometric-match --folder-path "/Home/Parts" --recursive \
  --checkpoint parts-match.jsonl --format csv --headers > matches.csv
```

With `--checkpoint FILE` a folder match appends each asset's result to FILE the
moment its search finishes. If the run is interrupted, re-running the same
command with the same file reuses the recorded results and searches only the
assets that are left. The file is tied to the exact run (search type, tenant,
folders, threshold, `--recursive`, `--exclusive`, `--limit`); a file from a
different run is refused rather than mixed in. It is deleted once the report has
been written.

## The Summary at the End

When using folder download and upload commands, you'll receive detailed statistics reports:

**Download Statistics Report:**
```
📊 Download Statistics Report
===========================
✅ Successfully downloaded: 125 assets
⏭️  Skipped (already existed): 75 assets
❌ Failed downloads: 2 assets
📁 Total assets processed: 202 assets
⏳ Operation completed successfully!
```

**Upload Statistics Report:**
```
📊 Upload Statistics Report
==========================
✅ Successfully uploaded: 150 assets
⏭️  Skipped (already existed): 0 assets
❌ Failed uploads: 1 asset
📁 Total assets processed: 151 assets
⏳ Operation completed successfully!
```

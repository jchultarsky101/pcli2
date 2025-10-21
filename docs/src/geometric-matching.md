# Geometric Matching

PCLI2 provides powerful geometric matching capabilities to find similar assets in your Physna tenant. This feature leverages advanced algorithms to identify assets with similar geometries, regardless of their orientation, scale, or position.

## Table of Contents
- [Overview](#overview)
- [Single Asset Matching](#single-asset-matching)
- [Folder-Based Matching](#folder-based-matching)
- [Threshold Settings](#threshold-settings)
- [Performance Options](#performance-options)
- [Error Handling](#error-handling)
- [Best Practices](#best-practices)
- [Advanced Usage](#advanced-usage)
- [Troubleshooting](#troubleshooting)

## Overview

Geometric matching helps you:

- Find duplicate or near-duplicate assets
- Identify variations of the same part
- Locate similar components across different projects
- Reduce storage costs by identifying redundant assets
- Improve design workflows by finding existing similar parts

## Overview

Geometric matching helps you:

- Find duplicate or near-duplicate assets
- Identify variations of the same part
- Locate similar components across different projects
- Reduce storage costs by identifying redundant assets
- Improve design workflows by finding existing similar parts

## Related Features

Geometric matching serves as the foundation for other powerful capabilities:

- **Metadata Inference**: Automatically propagate metadata from reference assets to geometrically similar assets using `pcli2 asset metadata inference`
- **Part Family Management**: Organize and categorize groups of similar components
- **Design Optimization**: Identify opportunities for part consolidation and standardization


## Single Asset Matching

Find geometrically similar assets for a specific reference asset.

### Basic Usage

```bash
# Find matches for a specific asset
pcli2 asset geometric-match --path /Root/Folder/ReferenceModel.stl --threshold 80.0

# Using asset UUID instead of path
pcli2 asset geometric-match --uuid 123e4567-e89b-12d3-a456-426614174000 --threshold 85.0
```

### Output Formats

#### JSON Format (Default)

```json
[
  {
    "referenceAssetName": "ReferenceModel.stl",
    "candidateAssetName": "SimilarModel.stl",
    "matchPercentage": 95.75,
    "referenceAssetPath": "/Root/Folder/ReferenceModel.stl",
    "candidateAssetPath": "/Root/DifferentFolder/SimilarModel.stl",
    "referenceAssetUuid": "123e4567-e89b-12d3-a456-426614174000",
    "candidateAssetUuid": "987fc321-fedc-ba98-7654-43210fedcba9"
  }
]
```

#### CSV Format

```csv
REFERENCE_ASSET_NAME,CANDIDATE_ASSET_NAME,MATCH_PERCENTAGE,REFERENCE_ASSET_PATH,CANDIDATE_ASSET_PATH,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID
ReferenceModel.stl,SimilarModel.stl,95.75,/Root/Folder/ReferenceModel.stl,/Root/DifferentFolder/SimilarModel.stl,123e4567-e89b-12d3-a456-426614174000,987fc321-fedc-ba98-7654-43210fedcba9
```

### Threshold Settings

The threshold parameter controls the minimum similarity percentage required for a match:

- **0.0** - Return all possible matches (may include unrelated assets)
- **50.0** - Very loose matching (many potential matches)
- **80.0** - Default setting (good balance of accuracy and recall)
- **90.0** - Strict matching (high confidence matches)
- **95.0+** - Very strict matching (near duplicates only)

## Folder-Based Matching

Find geometrically similar assets for all assets in a specified folder. This command processes assets in parallel for improved performance.

### Basic Usage

```bash
# Find matches for all assets in a folder
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --threshold 85.0
```

### Performance Options

#### Concurrency Control

Control how many simultaneous operations are performed:

```bash
# Use 10 concurrent operations (default is 5)
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --concurrent 10
```

#### Progress Tracking

Display a progress bar during long-running operations:

```bash
# Show progress bar
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --progress

# Combine with concurrency
pcli2 asset geometric-match-folder --path /Root/SearchFolder/ --concurrent 8 --progress
```

### Handling Large Folders

For folders with many assets, consider these strategies:

1. **Adjust threshold**: Higher thresholds reduce processing time
2. **Increase concurrency**: Use more concurrent operations (but watch resource usage)
3. **Process in batches**: Break large folders into smaller subfolders

## Error Handling

### Common Errors

#### HTTP 409 Conflict

When the server is busy or rate-limiting requests:

```
ERROR: Error performing geometric search for asset XXX after 3 retries: HTTP error: HTTP status client error (409 Conflict)
```

PCLI2 automatically retries up to 3 times with 500ms delays between attempts.

#### Permission Denied

When you don't have permission to perform geometric search:

```
ERROR: Error: Access forbidden. You don't have permission to perform geometric search on this asset.
```

Check your tenant permissions and API credentials.

#### Asset Not Found

When the specified asset or folder cannot be found:

```
ERROR: The asset with ID 'XXX' cannot be found in tenant 'YYY'
```

Verify the asset path or UUID is correct.

## Best Practices

### Optimizing Performance

1. **Use appropriate thresholds**: Start with 80-85% and adjust based on results
2. **Limit search scope**: Use specific folders rather than searching entire tenants
3. **Monitor resource usage**: Adjust concurrency based on your system capabilities
4. **Use progress tracking**: Monitor long-running operations

### Interpreting Results

- **High match percentages (>95%)**: Likely duplicates or very similar assets
- **Medium match percentages (80-95%)**: Similar geometry with variations
- **Low match percentages (<80%)**: May be false positives or loosely related

### Automation Tips

1. **Schedule regular deduplication**: Run geometric matching periodically to identify duplicates
2. **Integrate with CI/CD**: Use geometric matching in automated workflows
3. **Export results**: Use CSV format for easy analysis in spreadsheets

## Advanced Usage

### Scripting Examples

#### Bash Script for Regular Deduplication

```bash
#!/bin/bash
# deduplicate.sh

FOLDERS=("/Root/ProjectA/" "/Root/ProjectB/" "/Root/Archive/")
THRESHOLD=95.0
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

for folder in "${FOLDERS[@]}"; do
    echo "Processing folder: $folder"
    pcli2 asset geometric-match-folder \
        --path "$folder" \
        --threshold $THRESHOLD \
        --format csv \
        --progress \
        > "duplicates_${folder//\//_}_$TIMESTAMP.csv"
done

echo "Deduplication complete. Results saved to CSV files."
```

#### PowerShell Script for Windows

```powershell
# deduplicate.ps1

$Folders = @("/Root/ProjectA/", "/Root/ProjectB/", "/Root/Archive/")
$Threshold = 95.0
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"

foreach ($folder in $Folders) {
    Write-Host "Processing folder: $folder"
    pcli2 asset geometric-match-folder `
        --path $folder `
        --threshold $Threshold `
        --format csv `
        --progress `
        > "duplicates_$($folder.Replace('/', '_'))_$Timestamp.csv"
}

Write-Host "Deduplication complete. Results saved to CSV files."
```

## Troubleshooting

### Performance Issues

If matching operations are taking too long:

1. **Reduce concurrency**: Lower the `--concurrent` value
2. **Increase threshold**: Use higher threshold values to reduce matches
3. **Check network**: Ensure good connectivity to the Physna API
4. **Monitor server status**: Check if the Physna service is experiencing issues

### Incomplete Results

If you're not seeing expected matches:

1. **Lower threshold**: Try lower threshold values
2. **Check asset types**: Ensure assets are compatible geometric file types
3. **Verify permissions**: Confirm you have access to all assets in the search scope
4. **Contact support**: If issues persist, reach out to Physna support

## Related Commands

- `asset geometric-match` - Find matches for a single asset
- `asset geometric-match-folder` - Find matches for all assets in a folder
- `asset list` - List assets in a folder
- `asset get` - Get detailed asset information

Use `pcli2 asset geometric-match --help` and `pcli2 asset geometric-match-folder --help` for detailed command information.
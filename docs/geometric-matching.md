# Geometric and Part Matching

PCLI2 provides powerful geometric and part matching capabilities to find similar assets in your Physna tenant. These features leverage advanced algorithms to identify assets with similar geometries, regardless of their orientation, scale, or position.

## Table of Contents
- [Overview](#overview)
- [Geometric Matching](#geometric-matching)
  - [Single Asset Matching](#single-asset-matching)
  - [Folder-Based Matching](#folder-based-matching)
- [Part Matching](#part-matching)
  - [Single Asset Part Matching](#single-asset-part-matching)
  - [Folder-Based Part Matching](#folder-based-part-matching)
- [Threshold Settings](#threshold-settings)
- [Performance Options](#performance-options)
- [Error Handling](#error-handling)
- [Best Practices](#best-practices)
- [Advanced Usage](#advanced-usage)
- [Troubleshooting](#troubleshooting)

## Overview

Matching features help you:

- Find duplicate or near-duplicate assets
- Identify variations of the same part
- Locate similar components across different projects
- Reduce storage costs by identifying redundant assets
- Improve design workflows by finding existing similar parts

## Geometric Matching

Geometric matching identifies assets with similar 3D geometry characteristics.

### Single Asset Matching

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

### Folder-Based Matching

Find geometrically similar assets for all assets in one or more specified folders. This command processes assets in parallel for improved performance.

### Basic Usage

```bash
# Find matches for all assets in a folder
pcli2 folder geometric-match --folder-path /Root/SearchFolder/ --threshold 85.0

# Find matches for all assets in multiple folders
pcli2 folder geometric-match --folder-path /Root/Folder1/ --folder-path /Root/Folder2/ --threshold 80.0
```

### Performance Options

#### Concurrency Control

Control how many simultaneous operations are performed:

```bash
# Use 10 concurrent operations (default is 5)
pcli2 folder geometric-match --folder-path /Root/SearchFolder/ --concurrent 10
```

#### Progress Tracking

Display a progress bar during long-running operations:

```bash
# Show progress bar
pcli2 folder geometric-match --folder-path /Root/SearchFolder/ --progress

# Combine with concurrency
pcli2 folder geometric-match --folder-path /Root/SearchFolder/ --concurrent 8 --progress
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

## Part Matching

Part matching identifies assets that are parts of assemblies or contain similar components.

### Single Asset Part Matching

Find part matches for a specific reference asset.

#### Basic Usage

```bash
# Find part matches for a specific asset
pcli2 asset part-match --path /Root/Folder/ReferenceModel.stl --threshold 80.0

# Using asset UUID instead of path
pcli2 asset part-match --uuid 123e4567-e89b-12d3-a456-426614174000 --threshold 85.0
```

### Folder-Based Part Matching

Find part matches for all assets in one or more specified folders. This command processes assets in parallel for improved performance.

#### Basic Usage

```bash
# Find part matches for all assets in a folder
pcli2 folder part-match --folder-path /Root/SearchFolder/ --threshold 85.0

# Find part matches for assets in multiple folders
pcli2 folder part-match --folder-path /Root/Folder1/ --folder-path /Root/Folder2/ --threshold 80.0

# Use exclusive flag to only show matches where both assets belong to the specified paths
pcli2 folder part-match --folder-path /Root/SearchFolder/ --threshold 85.0 --exclusive

# Combine with performance options
pcli2 folder part-match --folder-path /Root/SearchFolder/ --threshold 85.0 --concurrent 8 --progress
```

### Performance Options

#### Concurrency Control

Control how many simultaneous operations are performed:

```bash
# Use 10 concurrent operations (default is 1)
pcli2 folder part-match --folder-path /Root/SearchFolder/ --concurrent 10
```

##### Progress Tracking

Display a progress bar during long-running operations:

```bash
# Show progress bar
pcli2 folder part-match --folder-path /Root/SearchFolder/ --progress

# Combine with concurrency
pcli2 folder part-match --folder-path /Root/SearchFolder/ --concurrent 8 --progress
```

### Handling Large Folders

For folders with many assets, consider these strategies:

1. **Adjust threshold**: Higher thresholds reduce processing time
2. **Increase concurrency**: Use more concurrent operations (but watch resource usage)
3. **Process in batches**: Break large folders into smaller subfolders
4. **Use exclusive flag**: Limit results to matches within specified folders only

### Advanced Usage

#### Scripting Examples

##### Bash Script for Regular Part Matching

```bash
#!/bin/bash
# part_match.sh

FOLDERS=("/Root/ProjectA/" "/Root/ProjectB/" "/Root/Archive/")
THRESHOLD=85.0
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

for folder in "${FOLDERS[@]}"; do
    echo "Processing folder: $folder"
    pcli2 asset part-match-folder \
        --path "$folder" \
        --threshold $THRESHOLD \
        --format csv \
        --progress \
        --concurrent 5 \
        > "part_matches_${folder//\//_}_$TIMESTAMP.csv"
done

echo "Part matching complete. Results saved to CSV files."
```

##### PowerShell Script for Windows

```powershell
# part_match.ps1

$Folders = @("/Root/ProjectA/", "/Root/ProjectB/", "/Root/Archive/")
$Threshold = 85.0
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"

foreach ($folder in $Folders) {
    Write-Host "Processing folder: $folder"
    pcli2 asset part-match-folder `
        --path $folder `
        --threshold $Threshold `
        --format csv `
        --progress `
        --concurrent 5 `
        > "part_matches_$($folder.Replace('/', '_'))_$Timestamp.csv"
}

Write-Host "Part matching complete. Results saved to CSV files."
```

### Output Formats

#### JSON Format (Default)

```json
[
  {
    "referenceAssetPath": "/Root/Folder/ReferenceModel.stl",
    "candidateAssetPath": "/Root/DifferentFolder/SimilarModel.stl",
    "forwardMatchPercentage": 95.75,
    "reverseMatchPercentage": 94.20,
    "referenceAssetUuid": "123e4567-e89b-12d3-a456-426614174000",
    "candidateAssetUuid": "987fc321-fedc-ba98-7654-43210fedcba9",
    "comparisonUrl": "https://app.physna.com/tenants/example/compare?asset1Id=123e4567-e89b-12d3-a456-426614174000&asset2Id=987fc321-fedc-ba98-7654-43210fedcba9&tenant1Id=tenant-uuid&tenant2Id=tenant-uuid&searchType=part&forwardMatch=95.75&reverseMatch=94.20"
  }
]
```

#### CSV Format

```csv
REFERENCE_ASSET_PATH,CANDIDATE_ASSET_PATH,FORWARD_MATCH_PERCENTAGE,REVERSE_MATCH_PERCENTAGE,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID,COMPARISON_URL
/Root/Folder/ReferenceModel.stl,/Root/DifferentFolder/SimilarModel.stl,95.75,94.20,123e4567-e89b-12d3-a456-426614174000,987fc321-fedc-ba98-7654-43210fedcba9,https://app.physna.com/...
```

## Related Commands

- `asset geometric-match` - Find matches for a single asset
- `folder geometric-match` - Find matches for all assets in one or more folders
- `asset part-match` - Find part matches for a single asset
- `folder part-match` - Find part matches for all assets in one or more folders
- `asset list` - List assets in a folder
- `asset get` - Get detailed asset information

Use `pcli2 asset part-match --help` and `pcli2 folder part-match --help` for detailed command information.
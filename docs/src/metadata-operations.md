# Metadata Operations

PCLI2 provides comprehensive metadata operations for managing asset metadata including creating, retrieving, updating, and deleting asset metadata.

## Overview

Metadata is essential for organizing and searching your assets effectively. PCLI2 supports comprehensive metadata operations to help you manage your asset metadata efficiently.

## Metadata Operations

PCLI2 provides several commands for working with asset metadata:

### 1. Create/Update Individual Asset Metadata

Add or update a single metadata field on an asset:

```bash
# Add or update a single metadata field on an asset
pcli2 asset metadata create --path "/Root/Folder/Model.stl" --name "Material" --value "Steel" --type "text"

# Add or update a single metadata field on an asset by UUID
pcli2 asset metadata create --uuid "123e4567-e89b-12d3-a456-426614174000" --name "Weight" --value "15.5" --type "number"
```

### 2. Retrieve Asset Metadata

Get all metadata for an asset:

```bash
# Get all metadata for an asset in JSON format (default)
pcli2 asset metadata get --path "/Root/Folder/Model.stl"

# Get all metadata for an asset in CSV format (suitable for batch operations)
pcli2 asset metadata get --uuid "123e4567-e89b-12d3-a456-426614174000" --format csv
```

### 3. Delete Asset Metadata

Delete specific metadata fields from an asset:

```bash
# Delete specific metadata fields from an asset
pcli2 asset metadata delete --path "/Root/Folder/Model.stl" --name "Material" --name "Weight"

# Delete metadata fields using comma-separated list
pcli2 asset metadata delete --uuid "123e4567-e89b-12d3-a456-426614174000" --name "Material,Weight,Description"
```

The delete command now uses the dedicated API endpoint to properly remove metadata fields from assets, rather than fetching all metadata and re-updating the asset without the specified fields. This provides more efficient and accurate metadata deletion.

### 4. Create/Update Metadata for Multiple Assets

Create or update metadata for multiple assets from a CSV file:

```bash
# Create or update metadata for multiple assets from a CSV file
pcli2 asset metadata create-batch --csv-file "metadata.csv"
```

## CSV Format for Batch Metadata Operations

The CSV format used by `asset metadata get --format csv` and `asset metadata create-batch --csv-file` is designed for seamless round-trip operations:

```csv
ASSET_PATH,NAME,VALUE
/Root/Folder/Model1.stl,Material,Steel
/Root/Folder/Model1.stl,Weight,"15.5 kg"
/Root/Folder/Model2.ipt,Material,Aluminum
/Root/Folder/Model2.ipt,Supplier,Richardson Electronics
```

The CSV format specifications:
- **Header Row**: Must contain exactly `ASSET_PATH,NAME,VALUE` in that order
- **ASSET_PATH**: Full path to the asset in Physna (e.g., `/Root/Folder/Model.stl`)
- **NAME**: Name of the metadata field to set
- **VALUE**: Value to assign to the metadata field
- **File Encoding**: Must be UTF-8 encoded
- **Quoting**: Values containing commas, quotes, or newlines must be enclosed in double quotes
- **Escaping**: Double quotes within values must be escaped by doubling them (e.g., `"15.5"" diameter"`)
- **Empty Rows**: Will be ignored during processing
- **Multiple Fields**: If an asset has multiple metadata fields to update, include multiple rows with the same ASSET_PATH but different NAME and VALUE combinations

**Example Command:**
```bash
# Create/update metadata for multiple assets from a CSV file
pcli2 asset metadata create-batch --csv-file "metadata.csv"
```

**Note:** The `create-batch` command processes each row as a single metadata field assignment for an asset. Multiple rows with the same ASSET_PATH will update multiple metadata fields for that asset in a single API call.

## Advanced Metadata Workflow: Export, Modify, Reimport

One of the most powerful features of PCLI2 is the ability to export metadata, modify it externally, and reimport it:

1. **Export Metadata**:
   ```bash
   # Export all metadata for an asset to a CSV file
   pcli2 asset metadata get --path "/Root/Folder/Model.stl" --format csv > model_metadata.csv

   # Export metadata for multiple assets in a folder
   pcli2 asset list --path "/Root/Folder/" --metadata --format csv > folder_metadata.csv
   ```

2. **Modify Metadata Externally**:
   - Open the CSV file in a spreadsheet application (Excel, Google Sheets, etc.)
   - Make the desired changes to metadata values
   - Save the file in CSV format

3. **Reimport Modified Metadata**:
   ```bash
   # Update assets with modified metadata
   pcli2 asset metadata create-batch --csv-file "modified_metadata.csv"
   ```

This workflow enables powerful bulk metadata operations while maintaining the flexibility to use familiar spreadsheet tools for data manipulation.

## Metadata Field Types

PCLI2 supports three metadata field types:

1. **Text** (default): String values
   ```bash
   pcli2 asset metadata create --path "/Root/Model.stl" --name "Description" --value "Sample part description" --type "text"
   ```

2. **Number**: Numeric values
   ```bash
   pcli2 asset metadata create --path "/Root/Model.stl" --name "Weight" --value "15.5" --type "number"
   ```

3. **Boolean**: True/False values
   ```bash
   pcli2 asset metadata create --path "/Root/Model.stl" --name "Approved" --value "true" --type "boolean"
   ```

## Best Practices

1. **Use Descriptive Names**: Choose clear, consistent names for metadata fields across your organization
2. **Validate Data Types**: Ensure values match the expected data type for each field
3. **Batch Operations**: Use CSV batch operations for large-scale metadata updates
4. **Backup Before Bulk Operations**: Export metadata before performing bulk deletions
5. **Test First**: Use small test sets before applying operations to large asset collections
6. **Use Proper Authentication**: Ensure your credentials have appropriate permissions for metadata operations

## Error Handling

The metadata operations are designed to be resilient:
- Continues processing even if individual asset operations fail
- Provides detailed error messages for troubleshooting
- Automatically handles network failures with retries
- Validates input formats before processing

Common error scenarios and their handling:
- **Missing assets**: Command skips missing assets with appropriate warnings
- **Network failures**: Individual operations retry, overall process continues
- **Permission issues**: Skips inaccessible assets with warning messages
- **Invalid metadata**: Logs error but continues processing other assets
- **API rate limits**: Respects rate limits and waits before retrying

## Performance Considerations

### Large-Scale Operations

For bulk metadata operations:

```bash
# Process during off-peak hours
pcli2 asset metadata create-batch --csv-file "large_metadata.csv"
```

### Monitoring Progress

Monitor progress during long-running operations:

```bash
# Show progress during batch operations
pcli2 asset metadata create-batch --csv-file "metadata.csv" --progress
```

## Integration with Other Commands

Metadata operations work seamlessly with other PCLI2 commands:

```bash
# Chain with asset operations
pcli2 asset list --path "/Root/Parts/" --format csv | \
pcli2 asset metadata create-batch --csv-file "metadata_updates.csv"

# Export results for auditing
pcli2 asset metadata get --path "/Root/Parts/Model.stl" --format csv > metadata_export.csv
```

## Limitations

1. **API Rate Limits**: Extensive operations may be rate-limited by the Physna API
2. **Processing Time**: Large batch operations can take considerable time
3. **Metadata Types**: Only supports text, number, and boolean metadata fields
4. **Asset Access**: Can only process assets accessible to your authenticated user
5. **Field Names**: Metadata field names must be unique per asset and follow Physna naming conventions

Always test operations on a small scale before running them on large datasets.
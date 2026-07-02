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

## CSV Formats for Batch Metadata Operations

The `create-batch` command accepts two CSV layouts. The layout is detected automatically from the header row: if any column name starts with `metadata:`, the file is treated as the UI (horizontal) format; otherwise it is treated as the classic (vertical) format. You can also force a layout explicitly with `--csv-format classic` or `--csv-format ui` (the default is `--csv-format auto`).

In both layouts, **empty values are skipped by default**: the existing metadata field on the asset, if any, is left untouched, so a sparse file can be used to incrementally add or update fields. Pass `--delete-if-empty` to instead **delete** a metadata field from the asset when the file contains an empty value for it — useful when replacing an asset's metadata wholesale.

### Classic (Vertical) Format

The classic CSV format used by `asset metadata get --format csv` and `asset metadata create-batch --csv-file` is designed for seamless round-trip operations:

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
- **VALUE**: Value to assign to the metadata field. An empty value is skipped by default, or deletes the field from the asset when `--delete-if-empty` is passed
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

**Deleting metadata fields via CSV:**

Pass `--delete-if-empty` and leave the VALUE column empty to remove a metadata field from an asset:

```csv
ASSET_PATH,NAME,VALUE
/Root/Folder/Model1.stl,ObsoleteField,
/Root/Folder/Model1.stl,Material,Steel
```

```bash
pcli2 asset metadata create-batch --csv-file "metadata.csv" --delete-if-empty
```

In the example above, `ObsoleteField` is deleted and `Material` is set to `Steel` in a single pass. Without `--delete-if-empty`, the `ObsoleteField` row would be skipped with a warning and only `Material` would be updated.

**Note:** The `create-batch` command groups all rows by asset path, then issues deletes (empty values, when `--delete-if-empty` is passed) followed by updates (non-empty values) per asset in one batch. Multiple rows with the same ASSET_PATH are combined into a single API interaction.

### UI (Horizontal) Format

The Physna web UI's bulk metadata upload uses a horizontal layout with one row per asset and one column per metadata field. `create-batch` accepts these files directly:

```csv
"path","id","metadata:Material","metadata:Color","metadata:Weight"
"/domain/assets/part1.sldprt","123e4567-e89b-12d3-a456-426614174000","Steel","Blue","2.5kg"
"/domain/assets/part2.step","","Aluminum","Red","1.2kg"
"/domain/assets/assembly.sldasm","","Mixed","",""
```

The UI format specifications:
- **path**: Full path to the asset in Physna
- **id**: Optional asset UUID. When present and non-empty, it takes precedence over the path and is used directly, without path resolution. An invalid UUID is an error (there is no fallback to the path, since that could silently target a different asset)
- **metadata:&lt;field name&gt;**: One column per metadata field. The `metadata:` prefix is stripped to obtain the field name
- **Empty metadata cells**: Skipped by default — the existing field value on the asset, if any, is left untouched. With `--delete-if-empty`, an empty cell deletes the field from the asset instead
- **Other columns**: Any column that is not `path`, `id`, or `metadata:*` is ignored, with a warning listing the ignored columns
- **Row identification**: Each row must provide a UUID or a path; a row with neither is an error

The whole file is parsed and validated before any API call is made, so a malformed file (e.g. an invalid UUID) fails fast with a line-numbered error instead of half-applying.

```bash
# Auto-detected from the header row
pcli2 asset metadata create-batch --csv-file "ui-export.csv"

# Or forced explicitly
pcli2 asset metadata create-batch --csv-file "ui-export.csv" --csv-format ui
```

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
   - To **delete** a field, clear its VALUE cell (leave it blank) and reimport with `--delete-if-empty`
   - Save the file in CSV format

3. **Reimport Modified Metadata**:
   ```bash
   # Update assets with modified metadata (blank values are skipped)
   pcli2 asset metadata create-batch --csv-file "modified_metadata.csv"

   # Or replace metadata wholesale: blank values delete the field from the asset
   pcli2 asset metadata create-batch --csv-file "modified_metadata.csv" --delete-if-empty
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

Metadata operations provide detailed error messages, retry transient network failures internally, and validate input formats before processing.

### `create-batch` Error Behavior

By default, `asset metadata create-batch` **stops on the first error** and prints a summary of how many assets were processed successfully. This makes failures visible instead of letting a batch silently complete with partial results.

Specifically:

- **CSV parsing errors**: always terminate immediately — the input file is expected to be well-formed
- **Unresolvable asset paths** (asset not found): by default, terminates the batch. Pass `--continue-on-error` to skip the failing asset and continue with the remaining rows
- **Metadata API failures** (delete/update): always terminate the batch, regardless of `--continue-on-error`. The API layer already retries transient HTTP failures, so a surfaced failure usually indicates a persistent problem (permissions, type conflict, etc.) that is likely to affect subsequent calls as well
- **Authentication failures**: always terminate with a remediation message directing the user to re-authenticate

**Example — skip unresolvable asset paths:**

```bash
pcli2 asset metadata create-batch --csv-file "metadata.csv" --continue-on-error
```

On completion (or termination), a summary is printed to stderr showing the number of successful and failed assets.

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
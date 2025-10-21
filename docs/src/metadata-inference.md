# Metadata Inference

The metadata inference feature allows you to automatically apply metadata from a reference asset to geometrically similar assets, significantly reducing manual metadata entry work.

## Overview

Metadata inference works by:
1. Taking a reference asset and specified metadata fields
2. Finding geometrically similar assets using the Physna geometric search
3. Applying the reference metadata to matching assets
4. Optionally continuing the process recursively for discovered matches

This is particularly useful for applying common metadata like materials, categories, suppliers, or costs to families of similar parts.

## Basic Usage

Apply metadata from a reference asset to similar assets:

```bash
pcli2 asset metadata inference --path /Root/Parts/Bolt-M8x20.stl --name "Material" --threshold 90.0
```

This command will:
- Find the asset at `/Root/Parts/Bolt-M8x20.stl`
- Extract the "Material" metadata field value
- Find all assets with 90% or higher geometric similarity
- Apply the same "Material" value to all matching assets

## Specifying Multiple Metadata Fields

You can apply multiple metadata fields in a single operation:

```bash
# Using comma-separated values
pcli2 asset metadata inference --path /Root/Parts/BaseModel.stl --name "Material,Cost,Supplier" --threshold 85.0

# Using multiple --name flags
pcli2 asset metadata inference --path /Root/Parts/BaseModel.stl --name "Material" --name "Cost" --name "Supplier" --threshold 85.0
```

## Recursive Processing

Enable recursive processing to apply metadata inference to discovered matches:

```bash
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "Category" --threshold 80.0 --recursive
```

With the `--recursive` flag, the system will:
1. Process the initial reference asset
2. Find and process similar assets (first level)
3. Continue finding and processing matches of those matches (second level)
4. And so on, until no new assets are discovered

The system automatically prevents infinite loops by tracking processed assets.

## Threshold Values

The threshold parameter controls the similarity requirement for matching assets:
- **Range**: 0.00 to 100.00
- **Higher values**: More stringent matching (fewer but more similar matches)
- **Lower values**: More permissive matching (more but less similar matches)
- **Recommended starting point**: 80.00-85.00 for most use cases

```bash
# Very strict matching (high similarity required)
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "CriticalField" --threshold 95.0

# Liberal matching (find more potential matches)
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "GeneralField" --threshold 75.0
```

## Practical Examples

### Applying Standard Materials

```bash
# Apply standard material to a family of similar bolts
pcli2 asset metadata inference --path /Root/StandardParts/Bolt-M8x20.stl --name "Material" --threshold 92.0 --recursive
```

### Categorizing Product Lines

```bash
# Assign category and supplier information to a product family
pcli2 asset metadata inference --path /Root/ProductLine/MainAssembly.stl --name "Category,Supplier,Division" --threshold 85.0
```

### Cost Propagation

```bash
# Apply estimated costs to similar components
pcli2 asset metadata inference --path /Root/Components/ReferenceBracket.stl --name "EstimatedCost,Currency" --threshold 88.0
```

## Best Practices

### 1. Start with Conservative Thresholds

Begin with higher threshold values (85-90%) to ensure high-quality matches, then adjust based on results:

```bash
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "Material" --threshold 90.0
```

### 2. Test with Non-Critical Metadata

Start by applying metadata to non-critical fields to understand the matching behavior:

```bash
pcli2 asset metadata inference --path /Root/Test/Reference.stl --name "TestTag" --threshold 85.0
```

### 3. Use Recursive Carefully

Recursive processing can be powerful but may affect many assets. Always review the scope first:

```bash
# First, see what would be affected without actually applying changes
pcli2 asset geometric-match --path /Root/Parts/Reference.stl --threshold 80.0 --format csv > potential_matches.csv

# Then proceed with metadata inference if results look good
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "Category" --threshold 80.0 --recursive
```

### 4. Combine with Geometric Matching

Use geometric matching first to preview results, then apply metadata inference:

```bash
# Preview matches
pcli2 asset geometric-match --path /Root/Parts/Reference.stl --threshold 85.0 --format csv

# Apply metadata if preview looks good
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "Material" --threshold 85.0
```

## Error Handling

The metadata inference command is designed to be resilient:
- Continues processing even if individual asset operations fail
- Provides detailed error messages for troubleshooting
- Automatically skips inaccessible assets
- Prevents infinite loops in recursive mode

Common error scenarios and their handling:
- **Missing reference asset**: Command aborts with clear error message
- **Network failures**: Individual operations retry, overall process continues
- **Permission issues**: Skips problematic assets with warning messages
- **Invalid metadata**: Logs error but continues processing other assets

## Performance Considerations

### Large-Scale Operations

For bulk metadata inference operations:

```bash
# Process during off-peak hours
pcli2 asset metadata inference --path /Root/LargeAssembly/Reference.stl --name "Category" --threshold 80.0 --recursive
```

### Monitoring Progress

Use the `--verbose` flag to monitor progress during long-running operations:

```bash
pcli2 --verbose asset metadata inference --path /Root/Parts/Reference.stl --name "Material" --threshold 85.0 --recursive
```

## Integration with Other Commands

Metadata inference works seamlessly with other PCLI2 commands:

```bash
# Chain with folder operations
pcli2 folder list --path /Root/ProductLine/ | \
pcli2 asset metadata inference --name "ProductLine" --threshold 85.0 --recursive

# Export results for auditing
pcli2 asset metadata inference --path /Root/Parts/Reference.stl --name "Category" --threshold 85.0 --recursive \
  --format csv > metadata_propagation_log.csv
```

## Limitations

1. **API Rate Limits**: Extensive recursive operations may be rate-limited by the Physna API
2. **Processing Time**: Large recursive operations can take considerable time
3. **Metadata Types**: Only supports text, number, and boolean metadata fields
4. **Asset Access**: Can only process assets accessible to your authenticated user

Always test operations on a small scale before running them on large datasets.
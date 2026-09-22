# Metadata Inference

The metadata inference feature allows you to automatically apply metadata from a reference asset to geometrically similar assets, significantly reducing manual metadata entry work.

## Overview

Metadata inference works by:
1. Taking a reference asset and specified metadata fields
2. Finding geometrically similar assets using the Physna geometric search
3. Applying the reference metadata to matching assets

This is particularly useful for applying common metadata like materials, categories, suppliers, or costs to families of similar parts.

## Basic Usage

Apply metadata from a reference asset to similar assets:

```bash
pcli2 asset metadata inference --path /Home/Parts/Bolt-M8x20.stl --name "Material" --threshold 90.0
```

This command will:
- Find the asset at `/Home/Parts/Bolt-M8x20.stl`
- Extract the "Material" metadata field value
- Find all assets with 90% or higher geometric similarity
- Apply the same "Material" value to all matching assets

## Specifying Multiple Metadata Fields

You can apply multiple metadata fields in a single operation:

```bash
# Using comma-separated values
pcli2 asset metadata inference --path /Home/Parts/BaseModel.stl --name "Material,Cost,Supplier" --threshold 85.0

# Using multiple --name flags
pcli2 asset metadata inference --path /Home/Parts/BaseModel.stl --name "Material" --name "Cost" --name "Supplier" --threshold 85.0
```

## Threshold Values

The threshold parameter controls the similarity requirement for matching assets:
- **Range**: 0.00 to 100.00
- **Higher values**: More stringent matching (fewer but more similar matches)
- **Lower values**: More permissive matching (more but less similar matches)
- **Recommended starting point**: 80.00-85.00 for most use cases

```bash
# Very strict matching (high similarity required)
pcli2 asset metadata inference --path /Home/Parts/Reference.stl --name "CriticalField" --threshold 95.0

# Liberal matching (find more potential matches)
pcli2 asset metadata inference --path /Home/Parts/Reference.stl --name "GeneralField" --threshold 75.0
```

## Practical Examples

### Applying Standard Materials

```bash
# Apply standard material to a family of similar bolts
pcli2 asset metadata inference --path /Home/StandardParts/Bolt-M8x20.stl --name "Material" --threshold 92.0
```

### Categorizing Product Lines

```bash
# Assign category and supplier information to a product family
pcli2 asset metadata inference --path /Home/ProductLine/MainAssembly.stl --name "Category,Supplier,Division" --threshold 85.0
```

### Cost Propagation

```bash
# Apply estimated costs to similar components
pcli2 asset metadata inference --path /Home/Components/ReferenceBracket.stl --name "EstimatedCost,Currency" --threshold 88.0
```

## Best Practices

### 1. Start with Conservative Thresholds

Begin with higher threshold values (85-90%) to ensure high-quality matches, then adjust based on results:

```bash
pcli2 asset metadata inference --path /Home/Parts/Reference.stl --name "Material" --threshold 90.0
```

### 2. Test with Non-Critical Metadata

Start by applying metadata to non-critical fields to understand the matching behavior:

```bash
pcli2 asset metadata inference --path /Home/Test/Reference.stl --name "TestTag" --threshold 85.0
```

### 3. Combine with Geometric Matching

Use geometric matching first to preview results, then apply metadata inference:

```bash
# Preview matches
pcli2 asset geometric-match --path /Home/Parts/Reference.stl --threshold 85.0 --format csv

# Apply metadata if preview looks good
pcli2 asset metadata inference --path /Home/Parts/Reference.stl --name "Material" --threshold 85.0
```

## Error Handling

The command checks before it changes anything, then stops at the first failure:

- **The reference asset has none of the named fields**: the command stops before
  searching, with exit 64, and names the fields it looked for.
- **The reference asset does not exist**: exit 67.
- **A metadata update fails** (a permission problem, a type conflict with an
  existing field): the command stops there. Assets updated before the failure
  keep their new values; the error says which asset failed.
- **Network hiccups** are retried by the client before they count as a failure,
  as for every command.

Because a run can stop partway, preview the matches first (see above) and run it
again after fixing the cause: assets that already carry the values are simply
written again.

## Integration with Other Commands

```bash
# Keep a record of what was changed
pcli2 asset metadata inference --path /Home/Parts/Reference.stl --name "Category" --threshold 85.0 \
  --format csv --headers > metadata_propagation_log.csv
```

## Limitations

1. **API Rate Limits**: Large operations may be rate-limited by the Physna API
2. **Processing Time**: Large operations can take considerable time
3. **Metadata Types**: Supports text, number, boolean and url metadata fields
4. **Asset Access**: Can only process assets accessible to your authenticated user

Always test operations on a small scale before running them on large datasets.
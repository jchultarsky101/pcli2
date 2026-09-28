# Catalog Search

A catalog is a Physna tenant that other tenants may search into, such as a
supplier catalog. `pcli2 catalog` takes an asset from one of your own tenants
and finds matching parts in a catalog: "does a supplier already sell this
part?"

Catalog search is enabled by Physna per customer. If it has not been enabled
for you, `pcli2 catalog list` prints nothing and warns, and the match commands
exit 68.

## Table of Contents
- [Which Catalogs You Can Search](#which-catalogs-you-can-search)
- [Matching an Asset](#matching-an-asset)
- [Output](#output)
- [What Is Never Searched](#what-is-never-searched)
- [Exit Codes](#exit-codes)

## Which Catalogs You Can Search

```bash
pcli2 catalog list
pcli2 catalog list --format csv --headers
```

```csv
NAME,DESCRIPTION,ID,ROLE
catalog,Supplier Catalog,a0d94427-d7eb-4b28-b624-49c68ce83686,search
```

The list is read from the server on every run, so a catalog Physna has just
enabled shows up without clearing any cache.

## Matching an Asset

The asset is named the usual way, by `--path` or `--uuid`, in the active tenant
or the one given with `--tenant`. `--in` names the catalog; with only one
catalog it can be left out. `PCLI2_CATALOG` sets it for every command, and is
checked the same way.

```bash
# Visually similar catalog parts, the top 20
pcli2 catalog visual-match --path /Home/Parts/bracket.stl --limit 20

# Geometric matches of 90% or better, as CSV
pcli2 catalog geometric-match --path /Home/Parts/bracket.stl --threshold 90 --format csv --headers

# Part search in a named catalog
pcli2 catalog part-match --uuid <ASSET_UUID> --in catalog
```

| Command | Alias | `--threshold` means |
|---------|-------|---------------------|
| `catalog geometric-match` | `gm` | minimum similarity, 0-100 (default 80) |
| `catalog part-match` | `pm` | minimum similarity, 0-100 (default 80) |
| `catalog visual-match` | `vm` | size tolerance, 0-100; 0 turns size filtering off |

Every command returns at most `--limit` matches (default 100) and warns when
there were more.

Metadata search and the combined search are not offered: Physna refuses the
first for a catalog, and the second currently fails on the server.

## Output

The CSV has the same columns as the corresponding `asset` command, with a
`CATALOG` column after the candidate's path:

```csv
REFERENCE_ASSET_PATH,CATALOG,CANDIDATE_ASSET_PATH,MATCH_PERCENTAGE,REFERENCE_ASSET_UUID,CANDIDATE_ASSET_UUID,COMPARISON_URL
```

`part-match` has `FORWARD_MATCH_PERCENTAGE,REVERSE_MATCH_PERCENTAGE` in place
of `MATCH_PERCENTAGE`; `visual-match` has no score column. The comparison link
opens both assets side by side in the web application.

JSON output is one object: `searchType`, `referenceAsset`, `catalog` and
`matches`.

## What Is Never Searched

`--in` accepts only the catalogs `catalog list` shows. If your account can see
several tenants (production and staging, or more than one company), naming any
of them that is not a catalog is refused before anything is searched, so one
tenant's search is never answered with another tenant's data. As a further
guard, a match the server returns from outside the chosen catalog is dropped,
with a warning.

The asset must be in one of your own tenants; an asset of the catalog itself
cannot be searched from.

## Exit Codes

| Code | When |
|------|------|
| 0 | The search ran (with or without matches) |
| 64 | `--in` is not one of your catalogs, or you have several and named none |
| 68 | Catalog search is not enabled for your account |
| 102 | The API refused a request, for example an asset UUID that does not exist (as with `asset visual-match`) |

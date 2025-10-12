#!/bin/bash
# Simple script to update metadata for a specific asset by UUID

if [ $# -ne 3 ]; then
    echo "Usage: $0 <asset_uuid> <metadata_key> <metadata_value>"
    exit 1
fi

ASSET_UUID=$1
METADATA_KEY=$2
METADATA_VALUE=$3

# Create a temporary JSON file with the metadata
cat > /tmp/metadata.json << EOF
{
    "metadata": {
        "$METADATA_KEY": "$METADATA_VALUE"
    }
}
EOF

# Use curl to update the metadata directly
curl -X PUT \
    -H "Authorization: Bearer $(pcli2 auth get | jq -r '.access_token')" \
    -H "Content-Type: application/json" \
    -d @/tmp/metadata.json \
    "https://app-api.physna.com/v3/tenants/68555ebf-f09c-4861-96b1-692d2ec10de7/assets/$ASSET_UUID"

# Clean up
rm /tmp/metadata.json
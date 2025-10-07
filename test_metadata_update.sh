#!/bin/bash
# Try different approaches to update metadata

if [ $# -ne 3 ]; then
    echo "Usage: $0 <asset_uuid> <metadata_key> <metadata_value>"
    exit 1
fi

ASSET_UUID=$1
METADATA_KEY=$2
METADATA_VALUE=$3

# Get access token
ACCESS_TOKEN=$(pcli2 auth get | jq -r '.access_token')

# Try PATCH method instead of PUT
echo "Trying PATCH method..."
curl -X PATCH \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"metadata\": {\"$METADATA_KEY\": \"$METADATA_VALUE\"}}" \
    "https://app-api.physna.com/v3/tenants/68555ebf-f09c-4861-96b1-692d2ec10de7/assets/$ASSET_UUID" \
    -v

echo -e "\n\nTrying with a metadata-specific endpoint..."
# Try with /metadata suffix
curl -X PUT \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"$METADATA_KEY\": \"$METADATA_VALUE\"}" \
    "https://app-api.physna.com/v3/tenants/68555ebf-f09c-4861-96b1-692d2ec10de7/assets/$ASSET_UUID/metadata" \
    -v
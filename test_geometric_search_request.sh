#!/bin/bash

# Test script to verify the geometric search request body format

# Get access token
ACCESS_TOKEN=$(cargo run -- auth get --format json | jq -r '.access_token')

echo "Access token: ${ACCESS_TOKEN:0:20}..." # Show only first 20 chars for security

# Test the geometric search endpoint with properly formatted request body
curl -X POST \
  https://app-api.physna.com/v3/tenants/763ccb08-4b30-4dea-ac69-7d7fb45b59d3/assets/f4bff223-2c4f-4df8-9cc4-3c9f67df7754/geometric-search \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
  "page": 1,
  "perPage": 20,
  "searchQuery": "",
  "filters": {
    "folders": [],
    "metadata": {},
    "extensions": []
  },
  "minThreshold": 80.0
}' \
  -v
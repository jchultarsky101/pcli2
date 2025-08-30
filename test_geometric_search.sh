#!/bin/bash

# Test script for geometric search API endpoint

# Get access token
ACCESS_TOKEN=$(cargo run -- auth get --format json | jq -r '.access_token')

echo "Access token: $ACCESS_TOKEN"

# Test the geometric search endpoint
curl -X POST \
  https://app-api.physna.com/v3/tenants/68555ebf-f09c-4861-96b1-692d2ec10de7/assets/3ed6b734-a43f-4942-920d-5c13f7ad534b/geometric-search \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"threshold": 0.7}' \
  -v
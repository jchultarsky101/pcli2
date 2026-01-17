#!/bin/bash
# Script to manually verify CLI help output

echo "=== Testing main help ==="
./target/debug/pcli2 --help

echo -e "\n=== Testing tenant help ==="
./target/debug/pcli2 tenant --help

echo -e "\n=== Testing folder help ==="
./target/debug/pcli2 folder --help

echo -e "\n=== Testing asset help ==="
./target/debug/pcli2 asset --help

echo -e "\n=== Testing auth help ==="
./target/debug/pcli2 auth --help

echo -e "\n=== Testing context help ==="
./target/debug/pcli2 context --help

echo -e "\n=== Testing config help ==="
./target/debug/pcli2 config --help

echo -e "\n=== Testing version ==="
./target/debug/pcli2 --version
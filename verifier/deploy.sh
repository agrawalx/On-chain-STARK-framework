#!/bin/bash

set -e
source .env

echo "[+] Deploying contract..."

# Clean up bytecode file by removing newlines and any potential whitespace
BYTECODE=$(tr -d '[:space:]' < bytecode.hex)

# Create a temporary file for the bytecode
TEMP_FILE=$(mktemp)
echo -n "$BYTECODE" > "$TEMP_FILE"

# Deploy using the temporary file
RUST_ADDRESS=$(cast send --account dev-account --create --code "$(cat "$TEMP_FILE")" --json | jq -r .contractAddress)

# Clean up the temporary file
rm "$TEMP_FILE"

echo "✅ Deployed to: $RUST_ADDRESS"
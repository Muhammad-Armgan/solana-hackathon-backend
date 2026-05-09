#!/usr/bin/env bash
set -euo pipefail

: "${TOKEN_DECIMALS:=2}"
: "${TOKEN_SUPPLY:=1000000000}"

solana config set --url devnet

if ! command -v spl-token >/dev/null 2>&1; then
  echo "spl-token CLI not found. Install Solana SPL CLI first."
  exit 1
fi

MINT_ADDRESS=$(spl-token create-token --decimals "$TOKEN_DECIMALS" | awk '/Creating token/ {print $3}')
TOKEN_ACCOUNT=$(spl-token create-account "$MINT_ADDRESS" | awk '/Creating account/ {print $3}')

# Supply is expressed in whole token units; spl-token applies decimals.
spl-token mint "$MINT_ADDRESS" "$TOKEN_SUPPLY" "$TOKEN_ACCOUNT"

echo "Mint address: $MINT_ADDRESS"
echo "Token account: $TOKEN_ACCOUNT"
echo "Save this mint in backend-rust/.env as SOLANA_TOKEN_MINT_ADDRESS"

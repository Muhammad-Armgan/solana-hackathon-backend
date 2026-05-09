#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../../solana-program"

anchor build
anchor deploy --provider.cluster devnet

echo "Deployment complete. Update SOLANA_PROGRAM_ID and SOLANA_TOKEN_MINT_ADDRESS in backend-rust/.env"

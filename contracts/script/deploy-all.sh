#!/usr/bin/env bash
set -euo pipefail

# Deploy DepositoorDelegate to all 5 chains via CREATE3.
# Same address on every chain.
#
# Usage:
#   PRIVATE_KEY=0x... KEEPER=0x... SALT=0x... ./script/deploy-all.sh
#
# Optional: override individual RPC URLs via env vars.

PRIVATE_KEY="${PRIVATE_KEY:?set PRIVATE_KEY}"
KEEPER="${KEEPER:?set KEEPER address}"
SALT="${SALT:?set SALT (use mine-vanity.sh to find one)}"

declare -A CHAINS=(
    ["Ethereum"]="${ETH_RPC_URL:-https://eth.llamarpc.com}"
    ["Arbitrum"]="${ARB_RPC_URL:-https://arb1.arbitrum.io/rpc}"
    ["Base"]="${BASE_RPC_URL:-https://mainnet.base.org}"
    ["Optimism"]="${OP_RPC_URL:-https://mainnet.optimism.io}"
    ["Polygon"]="${POLYGON_RPC_URL:-https://polygon.llamarpc.com}"
)

echo "=== Depositoor Multi-Chain Deployment ==="
echo "Keeper: $KEEPER"
echo "Salt:   $SALT"
echo ""

for chain in Ethereum Arbitrum Base Optimism Polygon; do
    rpc="${CHAINS[$chain]}"
    echo "--- $chain ---"
    PRIVATE_KEY="$PRIVATE_KEY" KEEPER="$KEEPER" SALT="$SALT" \
    forge script script/Deploy.s.sol \
        --rpc-url "$rpc" \
        --broadcast \
        --verify \
        -vvv 2>&1 | grep -E "Predicted|Deployed|Already|chain:|weth:|keeper:" || echo "FAILED"
    echo ""
done

echo "=== Done ==="

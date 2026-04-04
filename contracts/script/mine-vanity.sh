#!/usr/bin/env bash
set -euo pipefail

# Mine a vanity CREATE3 address for DepositoorDelegate.
#
# Usage:
#   PRIVATE_KEY=0x... KEEPER=0x... ./script/mine-vanity.sh [prefix]
#
# Default prefix: "dep051" (reads as "deposi...")
# The script tries random salts and prints matches.

PREFIX="${1:-dep051}"
KEEPER="${KEEPER:?set KEEPER address}"
PRIVATE_KEY="${PRIVATE_KEY:?set PRIVATE_KEY}"

echo "=== Depositoor Vanity Salt Miner ==="
echo "Looking for addresses starting with 0x${PREFIX}..."
echo ""

cd "$(dirname "$0")/.."

# First, build and get the factory address
forge build --silent

# Run the miner as a Forge script
forge script script/MineSalt.s.sol \
    --sig "mine(string)" "$PREFIX" \
    -vvv 2>&1 | grep -E "salt|address|Found"

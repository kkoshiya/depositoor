# depositoor

A non-custodial deposit sweeper. Users deposit any ERC-20 or native ETH on any supported chain. The system converts it to USDC and settles it to any address on any chain.

## Overview

The problem: accepting crypto payments means dealing with dozens of tokens across multiple chains. The user wants to pay with whatever they have. The recipient wants USDC on one chain.

depositoor solves this with EIP-7702 delegated accounts. A fresh EOA is generated per deposit session. The user sends tokens to it. A relayer sets the EIP-7702 delegation and executes a batched swap+transfer (or swap+bridge) through the delegated account -- all without the user signing anything beyond the initial transfer.

No smart wallet deployment. No counterfactual addresses. No Permit2 signatures. The user sends to a normal address and USDC shows up where they specified.

## Architecture

```
                  +-----------+
  user sends      |  indexer  |     follows chain head, matches Transfer
  any token  ---> |  (Rust)   |     logs + polls ETH balances against
  to burner       +-----------+     pending sessions in Postgres
                       |
                       v
                  +-----------+
                  |  sweeper  |     reads on-chain balance, gets Uniswap
                  |  (Rust)   |     quote, builds ERC-7821 batch, submits
                  +-----------+     EIP-7702 tx via relayer
                       |
                       v
              +------------------+
              | DepositoorDelegate |   on-chain: ~50 lines of Solidity
              | (ERC-7821)         |   execute(), sweep(), receive()
              +------------------+
                       |
                       v
                 USDC arrives at
                 dest address on
                 dest chain
```

**Backend** -- Rust, Axum, Alloy, PostgreSQL. Three separate processes: API server, indexer, sweeper.

**Contracts** -- Solidity, Foundry, Solady's ERC-7821. Deployed on all supported chains.

**Frontend** -- React, Viem, Wagmi. Generates burner keypair, signs EIP-7702 auth, tracks session via SSE.

## The four flows

Everything reduces to one or two transactions from the relayer. The user never signs anything after their initial deposit.

**Same chain, USDC deposit.** Single tx. The EIP-7702 delegation is set and `sweep(USDC, dest)` transfers the full balance to the destination. Done.

**Same chain, non-USDC deposit.** Single tx. Delegation + `approve(proxy, amount)` + Uniswap swap. The swap proxy pulls tokens from the burner via `transferFrom(msg.sender)` since the burner is executing the batch. We set `swapper = dest` in the Uniswap quote so the output goes directly to the destination address.

**Cross chain, USDC deposit.** Single tx. Delegation + `approve(bridge, amount)` + Uniswap BRIDGE call. Across Protocol handles delivery on the destination chain.

**Cross chain, non-USDC deposit.** Two txs. TX1: delegation + approve + Uniswap swap (USDC stays at burner). TX2: approve + Uniswap BRIDGE (sends USDC to dest on dest chain). We poll `balanceOf` between txs to get the exact post-slippage amount.

**Native ETH.** The indexer polls `eth_getBalance` for pending burners. When ETH is detected, the sweeper prepends `WETH.deposit{value: amount}()` to the batch, then proceeds as a normal WETH swap.

## Smart contract

```solidity
contract DepositoorDelegate is ERC7821 {
    address public immutable weth;
    address public immutable keeper;

    function sweep(address token, address to) external { ... }
    function _execute(...) internal virtual override { ... }
    receive() external payable override { /* wraps ETH to WETH */ }
}
```

The contract does three things: authorize the keeper to batch-execute calls, sweep full token balances, and auto-wrap ETH. All routing logic is off-chain.

## Uniswap integration

We use the Trading API with `x-permit2-disabled: true`. This returns swap calldata targeting a proxy contract that does `safeTransferFrom(msg.sender, router, amount)`. Since `msg.sender` is the burner (executing via ERC-7821), we just need a standard ERC-20 approve in the same batch. No Permit2, no EIP-712.

For cross-chain, the same API returns BRIDGE routing (Across under the hood). Same pattern: approve + call.

The proxy approach was critical. Uniswap's default flow requires Permit2 signatures from the token holder. Our burner is a delegated EOA controlled by the relayer -- it can execute on-chain calls but can't sign off-chain EIP-712 messages. The `x-permit2-disabled` proxy sidesteps this entirely.

## Indexer

Follows `newHeads` via WebSocket, fetches `eth_getLogs` per block via HTTP. This is more reliable than WS log subscriptions, which silently drop under load on Base and other high-throughput chains.

Per block:
1. Fetch Transfer logs, match `to` (topic[2]) against pending session burner addresses
2. Poll `eth_getBalance` for pending burners (native ETH detection)
3. Claim via PostgreSQL advisory locks (safe for multiple indexer instances)

Sessions expire after 30 minutes. Failed sessions re-activate on new deposits to the same burner.

## API

```
POST   /sessions              Register session (burner, EIP-7702 auth, dest address, dest chain)
GET    /sessions/:id           Session status
GET    /sessions/:id/events    SSE stream
POST   /sessions/:id/refund    Return tokens from failed session
```

## Supported chains

Ethereum, Arbitrum, Base, Optimism, Polygon.

## Running

```bash
docker-compose up -d postgres

cd backend
cargo run -- api &
cargo run -- indexer --chain-id 8453 --rpc-url wss://... &
cargo run -- sweeper --chain-id 8453 --rpc-url https://... &

cd frontend
npm install && npm run dev
```

Needs `DATABASE_URL`, `RELAYER_PRIVATE_KEY`, `IMPLEMENTATION_ADDRESS`, `UNISWAP_API_KEY` in `backend/.env`.

## Uniswap integration

Every non-USDC deposit flows through the Uniswap Trading API. The sweeper gets a quote, builds the calldata, and executes it through the burner's EIP-7702 delegated account — all in a single batch transaction.

We use the proxy approval flow (`x-permit2-disabled`) so the entire swap is just an ERC-20 approve + call. No Permit2 signatures, no off-chain signing from the burner. Routing hits V2, V3, and V4 pools through Uniswap's optimized pathfinding.

Cross-chain settlements use Uniswap's BRIDGE routing, which hands off to Across Protocol for the actual token delivery on the destination chain. From the sweeper's perspective it's the same API — `POST /quote`, `POST /swap`, execute. The Uniswap API abstracts whether it's a local pool swap or a cross-chain bridge.

```
POST /quote → POST /swap → approve + execute via ERC-7821 batch
```

## WalletConnect / Reown integration

The frontend uses Reown AppKit for wallet connectivity across EVM chains and Solana. Users connect through any WalletConnect-compatible wallet — desktop extensions, mobile wallets via QR scan, or smart wallets like Coinbase Smart Wallet and Porto.

WalletConnect sessions let mobile wallet users scan a QR code and send tokens directly to the deposit address without switching apps or manually entering addresses. The AppKit handles chain discovery, wallet detection, and session management across ecosystems.

## Dependencies

Alloy, Solady (ERC-7821), Uniswap Trading API, Across Protocol (via Uniswap BRIDGE routing), Reown AppKit, Foundry, Axum, Viem, Wagmi.

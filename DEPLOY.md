# Production Deployment Brief

> **Context for the agent reading this:** You are deploying depositoor to production for an ETHGlobal hackathon submission. This is live, on mainnet, with real money flowing through it. Every chain, every service, every endpoint must work. The team has spent the entire day building and testing this system — the contracts are deployed, the backend is proven on Base mainnet, the frontend is ready. Your job is to get it all running on a VPS with a proper domain and HTTPS so judges can use it. Do not cut corners. Do not skip chains. Do not leave services unmonitored. If something fails at demo time because of a misconfigured nginx rule or a missing env var, that's on you.
>
> Read this entire document before touching anything. Understand the architecture first. Then execute.

---

## What you're deploying

depositoor is an automated deposit sweeper. Users send any token on any chain to a burner address. The system detects it, swaps it to USDC via Uniswap, and delivers USDC to the user's chosen destination on any chain. It uses EIP-7702 account delegation so users just send to a plain EOA — no smart wallet setup needed.

## Architecture — what runs where

```
                     HTTPS (443)
                         |
                      [nginx]
                      /      \
                   /api        /
                  |              |
           [backend API]    [frontend]
           (port 3001)      (static files)
                  |
            [PostgreSQL]
                  |
     ┌────────────┼────────────┐
     |            |            |
 [indexer x5] [sweeper x5]  (one per chain)
```

**Total processes:** 1 nginx, 1 postgres, 1 API server, 5 indexers, 5 sweepers, frontend served as static files = **13 processes**.

## The five supported chains

| Chain | Chain ID | RPC (WS for indexer) | RPC (HTTP for sweeper) |
|-------|----------|---------------------|----------------------|
| Ethereum | 1 | `wss://eth-mainnet.g.alchemy.com/v2/{KEY}` | `https://eth-mainnet.g.alchemy.com/v2/{KEY}` |
| Arbitrum | 42161 | `wss://arb-mainnet.g.alchemy.com/v2/{KEY}` | `https://arb-mainnet.g.alchemy.com/v2/{KEY}` |
| Base | 8453 | `wss://base-mainnet.g.alchemy.com/v2/{KEY}` | `https://base-mainnet.g.alchemy.com/v2/{KEY}` |
| Optimism | 10 | `wss://opt-mainnet.g.alchemy.com/v2/{KEY}` | `https://opt-mainnet.g.alchemy.com/v2/{KEY}` |
| Polygon | 137 | `wss://polygon-mainnet.g.alchemy.com/v2/{KEY}` | `https://polygon-mainnet.g.alchemy.com/v2/{KEY}` |

**You need an Alchemy API key.** One key works for all chains. The key will be provided as `ALCHEMY_API_KEY`.

Each chain needs its own indexer process AND its own sweeper process. They take `--chain-id` and `--rpc-url` as arguments:

```bash
# Per chain:
depositoor indexer --chain-id 8453 --rpc-url wss://base-mainnet.g.alchemy.com/v2/{KEY}
depositoor sweeper --chain-id 8453 --rpc-url https://base-mainnet.g.alchemy.com/v2/{KEY}
```

## Repository structure

```
depositoor/
├── frontend/          React + Vite app
│   ├── package.json
│   └── src/
├── backend/           Rust + Axum
│   ├── Cargo.toml
│   ├── src/
│   └── .env           (you create this)
├── contracts/         Solidity (already deployed, don't touch)
├── docker-compose.yml PostgreSQL setup
└── README.md
```

## Step-by-step deployment

### 1. VPS setup

- Ubuntu 22.04+ or Debian 12+
- Minimum 2 CPU, 4GB RAM (the Rust binary is efficient but you're running 11 backend processes)
- Open ports 80, 443
- A domain pointing to the VPS IP (A record)

### 2. Install dependencies

```bash
# System
apt update && apt install -y build-essential pkg-config libssl-dev postgresql-client nginx certbot python3-certbot-nginx

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Node.js 20+
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt install -y nodejs

# Docker (for PostgreSQL)
curl -fsSL https://get.docker.com | sh
```

### 3. Clone and build

```bash
git clone <repo-url> /opt/depositoor
cd /opt/depositoor

# Build backend (release mode — this takes a few minutes)
cd backend
cargo build --release
# Binary will be at: target/release/depositoor

# Build frontend
cd ../frontend
npm install
npm run build
# Static files will be at: dist/
```

### 4. PostgreSQL

```bash
cd /opt/depositoor
docker-compose up -d postgres
```

Or if you prefer a native install, create a database called `depositoor` with user `postgres` and password `dev` (or whatever, just match the DATABASE_URL). The backend auto-creates tables on first run.

### 5. Backend environment

Create `/opt/depositoor/backend/.env`:

```env
DATABASE_URL=postgres://postgres:dev@localhost/depositoor
RELAYER_PRIVATE_KEY=<RELAYER_PRIVATE_KEY>
IMPLEMENTATION_ADDRESS=0x33333393A5EdE0c5E257b836034b8ab48078f53c
FEE_BPS=50
LISTEN_ADDR=0.0.0.0:3001
UNISWAP_API_KEY=<UNISWAP_API_KEY>
```

**Critical values that will be provided:**
- `RELAYER_PRIVATE_KEY` — the keeper/relayer wallet private key. This wallet pays gas on all chains. It MUST be funded with ETH on all 5 chains.
- `UNISWAP_API_KEY` — Uniswap Trading API key.

**The implementation address is `0x33333393A5EdE0c5E257b836034b8ab48078f53c` on ALL chains.** Do not change this.

### 6. Frontend environment

The frontend needs to know the API URL. Create `/opt/depositoor/frontend/.env.production`:

```env
VITE_API_URL=https://yourdomain.com/api
```

Then rebuild:
```bash
cd /opt/depositoor/frontend
npm run build
```

### 7. Nginx configuration

```nginx
server {
    listen 80;
    server_name yourdomain.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl;
    server_name yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    # Frontend (static files)
    root /opt/depositoor/frontend/dist;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    # Backend API
    location /api/ {
        rewrite ^/api/(.*) /$1 break;
        proxy_pass http://127.0.0.1:3001;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # SSE needs special handling — no buffering
    location /api/sessions/ {
        rewrite ^/api/(.*) /$1 break;
        proxy_pass http://127.0.0.1:3001;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header Connection '';
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600s;
    }
}
```

Get the cert:
```bash
certbot --nginx -d yourdomain.com
```

**Important:** The SSE endpoint (`/sessions/:id/events`) requires `proxy_buffering off` and a long read timeout. If you skip this, real-time status updates won't work and the frontend will look broken during demos.

### 8. Systemd services

Create a service for each process. They all use the same binary with different arguments.

**API server** — `/etc/systemd/system/depositoor-api.service`:
```ini
[Unit]
Description=depositoor API
After=network.target postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/depositoor/backend
Environment=RUST_LOG=info
ExecStart=/opt/depositoor/backend/target/release/depositoor api
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Indexers** — create one per chain. Example for Base, `/etc/systemd/system/depositoor-indexer-base.service`:
```ini
[Unit]
Description=depositoor indexer (Base)
After=network.target depositoor-api.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/depositoor/backend
Environment=RUST_LOG=info
ExecStart=/opt/depositoor/backend/target/release/depositoor indexer --chain-id 8453 --rpc-url wss://base-mainnet.g.alchemy.com/v2/ALCHEMY_KEY_HERE
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Sweepers** — same pattern. Example for Base, `/etc/systemd/system/depositoor-sweeper-base.service`:
```ini
[Unit]
Description=depositoor sweeper (Base)
After=network.target depositoor-api.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/depositoor/backend
Environment=RUST_LOG=info
ExecStart=/opt/depositoor/backend/target/release/depositoor sweeper --chain-id 8453 --rpc-url https://base-mainnet.g.alchemy.com/v2/ALCHEMY_KEY_HERE
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Create all 10 indexer+sweeper services:**

| Service name | Chain ID | RPC subdomain |
|---|---|---|
| `depositoor-indexer-ethereum` / `depositoor-sweeper-ethereum` | 1 | `eth-mainnet` |
| `depositoor-indexer-arbitrum` / `depositoor-sweeper-arbitrum` | 42161 | `arb-mainnet` |
| `depositoor-indexer-base` / `depositoor-sweeper-base` | 8453 | `base-mainnet` |
| `depositoor-indexer-optimism` / `depositoor-sweeper-optimism` | 10 | `opt-mainnet` |
| `depositoor-indexer-polygon` / `depositoor-sweeper-polygon` | 137 | `polygon-mainnet` |

Indexers use `wss://`, sweepers use `https://`.

Then:
```bash
systemctl daemon-reload
systemctl enable depositoor-api depositoor-indexer-{ethereum,arbitrum,base,optimism,polygon} depositoor-sweeper-{ethereum,arbitrum,base,optimism,polygon}
systemctl start depositoor-api depositoor-indexer-{ethereum,arbitrum,base,optimism,polygon} depositoor-sweeper-{ethereum,arbitrum,base,optimism,polygon}
```

### 9. Verify everything is running

```bash
# All 11 services should be active
systemctl status depositoor-* --no-pager | grep -E "Active:|●"

# API responds
curl https://yourdomain.com/api/sessions/00000000-0000-0000-0000-000000000000

# Frontend loads
curl -s https://yourdomain.com | head -1

# Check logs for each indexer
journalctl -u depositoor-indexer-base --no-pager -n 5
# Should show: "subscribed to newHeads on chain 8453"

# Check logs for each sweeper
journalctl -u depositoor-sweeper-base --no-pager -n 5
# Should show: "sweeper starting chain=\"Base\""
```

### 10. Fund the relayer

The relayer wallet must have ETH on ALL 5 chains to pay gas for sweep transactions. Amounts needed (approximate):

| Chain | Suggested ETH | Why |
|-------|--------------|-----|
| Ethereum | 0.01 ETH | Gas is expensive, but sweeps are rare on mainnet |
| Arbitrum | 0.002 ETH | Cheap gas |
| Base | 0.002 ETH | Cheap gas |
| Optimism | 0.002 ETH | Cheap gas |
| Polygon | 0.1 MATIC | Uses MATIC for gas |

**If the relayer runs out of gas on any chain, sweeps on that chain will silently fail.** Monitor this.

## Things that WILL go wrong if you're not careful

1. **SSE buffering.** If nginx buffers the SSE responses, the frontend won't get real-time status updates. The session will appear stuck on "pending" forever. Set `proxy_buffering off` on the SSE route.

2. **CORS.** The backend sets `CorsLayer::permissive()` so this should work. But if you put the API behind a different subdomain or path, double-check that the `Access-Control-Allow-Origin` header comes through.

3. **Frontend API URL.** The frontend reads `VITE_API_URL` at BUILD time (not runtime). If you build locally and deploy, it'll still point to localhost. Build on the server or set the env var before building.

4. **Database schema.** The backend auto-creates tables on first startup. But if you've run a previous version, the schema might be stale. If you see "cached plan must not change result type" errors, restart all services (they need fresh DB connections after schema changes). Or just drop and recreate the database if starting fresh:
   ```bash
   docker exec -i depositoor-postgres-1 psql -U postgres -c "DROP DATABASE IF EXISTS depositoor; CREATE DATABASE depositoor;"
   ```

5. **Alchemy rate limits.** With 5 indexers polling every block, you're making ~150 RPC calls per minute (5 chains x ~2s blocks x 3 calls per block). Alchemy's free tier allows 330 calls/second so you're fine, but monitor for 429s.

6. **The relayer private key.** This is a hot wallet on a VPS. It holds gas money. Keep the amounts minimal. Don't put more than you need.

## Monitoring

Quick health check script:

```bash
#!/bin/bash
echo "=== Services ==="
for svc in api indexer-{ethereum,arbitrum,base,optimism,polygon} sweeper-{ethereum,arbitrum,base,optimism,polygon}; do
    status=$(systemctl is-active depositoor-$svc)
    echo "  depositoor-$svc: $status"
done

echo ""
echo "=== Recent errors ==="
journalctl -u 'depositoor-*' --no-pager --since "5 minutes ago" -p err 2>/dev/null | tail -10

echo ""
echo "=== API ==="
curl -s -o /dev/null -w "%{http_code}" https://yourdomain.com/api/sessions/00000000-0000-0000-0000-000000000000
echo ""
```

## That's it

Once all services are green and the frontend loads over HTTPS, the system is live. Users can visit the site, generate a burner address, send any token on any of the 5 chains, and receive USDC at their destination. The whole flow takes seconds for same-chain, ~30 seconds for cross-chain.

Don't overthink it. Follow the steps. Verify each one. Ship it.

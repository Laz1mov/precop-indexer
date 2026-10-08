# Precop Consensus Indexer — Complete Quickstart Guide (2026 Edition)

Welcome to the **Precop Consensus Indexer & Sovereign Layer 1 Engine**. This guide will get your node up and running with 100% verified mathematical consensus.

---

## ⚡ Architecture & Dual Sync Modes

The indexer supports two distinct deployment modes depending on your hardware, network, and operational requirements:

| Feature | **Mode A: Sovereign Genesis Replay** | **Mode B: Fast Snapshot Bootstrap** |
| :--- | :--- | :--- |
| **Starting Block** | **967,485** (LEAF Genesis) | **970,474** (Verified Snapshot) |
| **Trust Model** | **Zero-Trust**: Recomputes all states from raw L1 blocks | **Verified Checksum**: SHA-256 verified state |
| **Sync Duration** | $\approx 2\text{ to }4\text{ hours}$ (depending on RPC disk IO) | $\approx 30\text{ seconds}$ to instant |
| **Network Overhead** | Full block download from Bitcoin Core | Single archive download |
| **Recommended For** | Institutional auditors, consensus verifiers | Production dApps, explorers, rapid node setup |

---

## 🛠️ Step 1: System Requirements & Prerequisites

- **Operating System**: Linux (Ubuntu 22.04+, Debian 12+, Alpine), macOS (Apple Silicon or Intel), or Docker.
- **CPU**: 4+ cores recommended.
- **RAM**: 8 GB minimum (16 GB recommended for in-memory block caches).
- **Disk Space**: 
  - Mode A: 50 GB free NVMe SSD storage.
  - Mode B: 5 GB free SSD storage.
- **Bitcoin Node**:
  - Bitcoin Core (v25+) or Floresta with **`txindex=1`** enabled in `bitcoin.conf`.
  - RPC access enabled (`rpcuser`, `rpcpassword`).

---

## 🚀 Step 2: Deployment via Docker (Recommended)

### 2.1 Clone the Repository
```bash
git clone https://github.com/Laz1mov/precop-indexer.git
cd precop-indexer
```

### 2.2 Configure Environment
```bash
cp .env.example .env
```
Edit `.env` with your editor of choice:
```ini
# Bitcoin Core Node RPC
BITCOIN_RPC_URL=http://host.docker.internal:8332
BITCOIN_RPC_USER=your_rpc_username
BITCOIN_RPC_PASS=your_rpc_password

# Indexer Configuration
INDEXER_PORT=8080
SIDECAR_PORT=8085
LOG_LEVEL=info

# Optional: set starting block override (leave empty for default)
# TUSM_START_BLOCK=967485
```

---

### 2.3 Option 1: Launch in Mode A (Genesis Replay)

If your `./data/` directory is empty, the indexer automatically starts from block **967,485**:
```bash
docker compose up -d --build
docker compose logs -f
```
You will observe:
```text
[*] TUSM V2: Starting Trustless Sync from Floresta at block 967485
[Consensus] Replaying Block 967485: Genesis Astrolabe verified
[Consensus] Height 968995: CRC-20 V2 Activation Enforced
```

---

### 2.4 Option 2: Launch in Mode B (Fast Verified Snapshot)

To skip the initial replay and resume instantly at the latest block:
```bash
# 1. Download and verify the official release snapshot
./scripts/bootstrap_snapshot.sh

# 2. Launch the services
docker compose up -d
docker compose logs -f
```

The script automatically executes:
1. **SHA-256 Checksum Verification**: Validates the archive against the official cryptographic hash `4f8a846c483955bb5935ef9e57813a1e2f75a61b83d16853612502787e22fb2e`.
2. **Anti-Path Traversal Check**: Rejects any archive attempting directory escape (`..`).
3. **Safe Extraction**: Decompresses verified Sled database files into `./data/`.

---

## 🖥️ Step 3: Bare-Metal Setup (Without Docker)

For operators compiling directly from source on native Linux/macOS:

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 2. Build Release Binaries
cargo build --release --workspace

# 3. Run the Indexer Node
./target/release/precop-tusm-indexer

# 4. In a separate terminal, run the Rust Sidecar
./target/release/precop-sidecar
```

---

## 🔍 Step 4: Verification & Healthcheck

### 4.1 Verify Node Health & Current Block
```bash
curl -s http://localhost:8080/v1/health | jq
```
Expected Output:
```json
{
  "status": "healthy",
  "height": 970509,
  "lag": 0,
  "tokens_indexed": 3
}
```

### 4.2 Query Canonical Token Balances
```bash
# Query LEAF token state
curl -s http://localhost:8080/v1/crc20/tokens | jq '.[] | select(.ticker=="LEAF")'

# Query token balance for an address
curl -s http://localhost:8080/v1/crc20/balance/LEAF/bc1pz42whrh25gx0h755fv26x333dwjg4tyjtgjda27qcmrkwz | jq
```

### 4.3 Use the Precop CLI
```bash
# Inspect local consensus database
cargo run --release -p precop-cli -- db inspect --db-path ./data/tusm_indexer.db

# Quote real-time BONS bonding curve
cargo run --release -p precop-cli -- curve quote --ticker BONS --deposit-sats 50000
```

---

## 🛠️ Step 5: Auditing & Test Validation

Run the formal verification test suites locally:

```bash
# 1. Run Property-Based Invariant Tests (Anti-Black-Swan)
cargo test -p tusm-core --test proptests_consensus_invariants

# 2. Measure Exact Code Coverage
cargo llvm-cov -p tusm-core

# 3. Check Dependency Security
cargo audit

# 4. Statistical Benchmarking (Criterion)
cargo bench -p tusm-core -- --test
```

---

## ❓ Troubleshooting

| Symptom | Cause | Solution |
| :--- | :--- | :--- |
| `RPC not ready yet: Connection refused` | Bitcoin Core is starting up or RPC is unreachable | Verify `BITCOIN_RPC_URL` in `.env` and verify Bitcoin Core is running with `rpcuser/rpcpassword`. |
| `Prevout missing in block ... Fails closed` | Bitcoin Core `txindex=1` is disabled | Ensure `txindex=1` is set in `bitcoin.conf` and re-index Bitcoin Core (`bitcoind -reindex`). |
| `Snapshot SHA-256 checksum verification failed` | Archive corrupted or downloaded from unauthorized mirror | Delete `/tmp/snapshot_970474.tar.gz` and re-run `./scripts/bootstrap_snapshot.sh`. |
| `Failed to open Sled DB: lock busy` | Another indexer process is accessing `./data/tusm_indexer.db` | Stop any background `precop-tusm-indexer` processes (`killall precop-tusm-indexer`). |

# Precop Consensus Indexer & Sovereign Layer 1 Engine

> **Turnkey Open-Source Consensus Indexer & CLI for Bitcoin L1 (Simplicity Covenants, CRC-20, CRC-21, Runes & BRC-20 Integrations)**

[![Rust](https://img.shields.io/badge/rust-1.92+-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)

---

## 📖 Architecture Overview

The **Precop Consensus Indexer** is a high-performance, deterministic Bitcoin Layer-1 state engine written in Rust. It tracks and indexes Simplicity covenants, native Taproot CSV seals, CRC-20 tokens, and CRC-21 sovereign vaults directly from Bitcoin blocks.

To account for multi-protocol deposits (such as **Runes / DOG** and **BRC-20 / ORDI**), the repository includes an autonomous **Rust Native Sidecar** (`precop-sidecar`):
- **Zero API Key Requirement**: Connects directly to UniSat's public wallet API (`wallet-api.unisat.io`) without requiring any paid API key.
- **Dynamic Payment Verification**: Starts from empty payment files and automatically discovers, verifies, and records incoming DOG and ORDI contributions as the indexer encounters candidate transactions.
- **Shared Data Pipeline**:
  - The indexer detects candidate mint transactions on-chain and writes their transaction IDs to `./data/pending_mints.jsonl`.
  - The sidecar continuously monitors the queue, verifies Runes (DOG `840000:3`) and BRC-20 transfer inscriptions (`ORDI`) delivered to the protocol treasury, and writes verified payments to `./data/external_payments.json`.
  - The indexer reloads `./data/external_payments.json` on each block, enforcing exact mathematical consensus rules.

```
┌────────────────────────────────────────────────────────┐
│                      Bitcoin Node                      │
│            (txindex=1, RPC port 8332)                  │
└───────────────▲────────────────────────▲───────────────┘
                │                        │
        Blocks & Prevouts            Raw Mempool
                │                        │
┌───────────────┴───────────────┐ ┌──────┴───────────────┐
│       precop_indexer          │ │       precop_sidecar         │
│   (Rust Consensus Engine)     │ │   (Rust Zero-Key Engine)     │
│       Port: 8080              │ │        Port: 8085            │
└───────────────▲───────────────┘ └──────▲───────────────┘
                │                        │
                ▼                        ▼
       ┌────────────────────────────────────────┐
       │         Shared Volume: ./data          │
       │  - external_payments.json              │
       │  - pending_mints.jsonl                 │
       │  - dog-evidence.json                   │
       │  - ordi-evidence.json                  │
       │  - tusm_indexer.db (Embedded Sled)     │
       └────────────────────────────────────────┘
```

---

## 🏛️ Protocol Invariants: Mint vs. Transfer

A strict invariant governs CRC-20 protocol speech on Bitcoin Layer 1:

- **Mint Payload**: `{"p":"crc-20","op":"mint","tick":"LEAF"}`  
  **Strictly NO `amt` field!** The token allocation is computed exclusively by the consensus engine from the satoshis deposited in the treasury output, the Taproot CSV lock multiplier ($1.618\times$ to $2.618\times$), and the remaining hardcap saturation.
- **Transfer Payload**: `{"p":"crc-20","op":"transfer","tick":"LEAF","amt":"50000"}`  
  **Requires an explicit `amt` field** specifying the exact quantity to transfer.

---

## 🛠️ Precop CLI (`precop`)

The repository includes a dedicated autonomous CLI tool (`precop-cli`) for operators, node runners, and arbitrageurs:

```bash
cargo build --release -p precop-cli
# binary available at ./target/release/precop
```

### 1. Verify Bitcoin L1 Transactions (`verify-tx`)

Perform zero-trust offline consensus validation of candidate transactions before broadcasting:

```bash
# Verify LEAF Mint transaction (validates Taproot seal, CSV locks, and treasury sats)
precop verify-tx --tx-hex <RAW_BITCOIN_TX_HEX>

# Verify CRC-21 Merkel Koala Collection Deploy (validates 546-sat carrier UTXO & OP_RETURN)
precop verify-tx --tx-hex <RAW_KOALA_DEPLOY_TX_HEX>
```

### 2. Parse & Decode L1 Speech (`parse-speech`)

Inspect and validate OP_RETURN protocol messages:

```bash
# Decode canonical LEAF Mint speech (confirms NO amt field)
precop parse-speech --input "6a237b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a224c454146227d"
# Output:
# {
#   "payload": {
#     "op": "mint",
#     "p": "crc-20",
#     "tick": "LEAF"
#   },
#   "status": "success",
#   "type": "script_op_return"
# }

# Decode canonical LEAF Transfer speech (confirms amt field)
precop parse-speech --input "6a377b2270223a226372632d3230222c226f70223a227472616e73666572222c227469636b223a224c454146222c22616d74223a223530303030227d"

# Decode CRC-21 Merkel Koala Deploy speech
precop parse-speech --input "6a297b2270223a226372632d3231222c226f70223a226465706c6f79222c2274797065223a226f7264227d"
```

### 3. Bonding Curve Quotes & Spot Price (`curve`)

Query real-time mathematical bonding curves:

```bash
# Quote BONS allocation for 50,000 satoshis deposit
precop curve quote --ticker BONS --deposit-sats 50000

# Query instantaneous spot price for BONS
precop curve spot-price --ticker BONS
```

### 4. Direct Node RPC & Sled Database Queries (`query`, `db`)

```bash
# Query token supply and state
precop query token --ticker LEAF

# Inspect local consensus database height and metadata
precop db inspect --db-path ./data/tusm_indexer.db
```

---

## ⚡ Quickstart

### 1. Prerequisites
- **Docker & Docker Compose** (version 20+ / compose v2)
- **Bitcoin Core Node** (version 25+ or 28+):
  - **Mandatory**: `txindex=1` must be enabled in `bitcoin.conf`.
  - RPC credentials enabled (`rpcuser`, `rpcpassword`).

### 2. Configure Environment
Copy the example environment file:
```bash
cp .env.example .env
```
Edit `.env` to specify your Bitcoin RPC credentials:
```ini
BITCOIN_RPC_URL=http://host.docker.internal:8332
BITCOIN_RPC_USER=your_rpc_username
BITCOIN_RPC_PASS=your_rpc_password
```

### 3. Launch Services
```bash
docker compose up -d --build
```
Check container logs:
```bash
docker compose logs -f
```

---

## 🔄 Sync Modes

### Mode A: Genesis Replay (From Block 967,485)
By default, the indexer starts with clean empty files in `./data/`:
- `data/external_payments.json`: `{}`
- `data/pending_mints.jsonl`: empty
- `data/dog-evidence.json`: `{"items": []}`
- `data/ordi-evidence.json`: `{"items": []}`

When started, the indexer replays from the LEAF Genesis Block (**967,485**). As candidate mints are detected, the Rust sidecar verifies DOG and ORDI payments via UniSat public API and writes confirmed entries into `external_payments.json`.

### Mode B: Fast Bootstrap (Snapshot Block 970,474)
To sync instantly without waiting for a full replay from block 967,485, use the included bootstrap script:
```bash
./scripts/bootstrap_snapshot.sh
docker compose up -d
```
This unpacks the verified Sled consensus database (`tusm_indexer.db`) and verified external payment records up to block **970,474**. The indexer will resume syncing from block 970,475 onward.

---

## 🧪 Advanced TDD Test Suite

Comprehensive Rust tests ensure 100% mathematical parity and verified zero-key operation across all modules.

```bash
# 1. Run precop CLI test suite (LEAF mint/transfer, BONS curves, CRC-21 deploy)
cargo test -p precop-cli

# 2. Run consensus engine tests
cargo test -p tusm-core

# 3. Run indexer library tests (24/24 integration tests)
cargo test -p precop-tusm-indexer --lib

# 4. Run native Rust sidecar tests (zero API key discovery)
cargo test -p precop-sidecar -- --nocapture
```

---

## 📡 REST API Reference

The indexer exposes a JSON REST API and an interactive Swagger UI.

- **Interactive Documentation**: `http://localhost:8080/swagger-ui`
- **Health & Sync Status**: `GET http://localhost:8080/v1/health`
- **List All Tokens**: `GET http://localhost:8080/v1/crc20/tokens`
- **Address Token Balance**: `GET http://localhost:8080/v1/crc20/balance/{ticker}/{address}`
- **Token Holders**: `GET http://localhost:8080/v1/crc20/holders/{ticker}`
- **Verified External Payments**: `GET http://localhost:8080/v1/crc20/external_payments`

For full endpoint documentation, see [API.md](API.md).

---

## 🛡 Security & Verification Guarantees
- **No Private Keys / Custody**: The indexer is strictly non-custodial and read-only.
- **NUMS Taproot Seals**: All covenant outputs derive provably unspendable internal Taproot keys (`50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`).
- **Mathematical Determinism**: Multipliers, bonding curve formulas, and integer divisions strictly mimic protocol specifications with overflow protection.

---

## 📄 License & Copyleft Protection

This project is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0-only)**.  
See the [`LICENSE`](LICENSE) file for complete terms.

```
Copyright (C) 2026 Laz1m0v and Sovereign Bitcoin Contributors.
https://github.com/BitcoinWorldTrustFoundation/precop
```

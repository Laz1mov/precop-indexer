# TUSM Citadel Indexer (V1.1.0-RC3 GOLDEN)

The Universal State Matrix (TUSM) is the authoritative indexing daemon for the Citadel CRC-20 token standard and CRC-21 digital artifacts. 

This repository contains the monolithic indexer daemon (`tusm-indexer`) which performs parallel high-throughput UTXO ingestion while serving lightning-fast API queries to the Precopscan frontend via an embedded Sled database.

## Operator Reproduction Manual

To sync the Citadel state from genesis (Block 967485) and expose the Precopscan REST API, follow these precise steps.

### 1. Initialize the Bitcoin Node (Floresta or Bitcoin Core)
The indexer requires a synced Bitcoin node. It speaks directly to your real node via RPC. Do NOT use mockups.

```bash
# Example with Floresta native integration
# Floresta must be running locally with RPC exposed.
export BITCOIN_RPC_URL="http://127.0.0.1:8332"
export BITCOIN_RPC_USER="your_rpc_user"
export BITCOIN_RPC_PASS="your_rpc_pass"
```

### 2. Compile the TUSM Daemon
Navigate to the `tusm-indexer` directory and compile the optimized release binary. A recent Rust toolchain (`stable` via rustup) is strictly required to compile the required networking and SQL dependencies.

```bash
cd tusm-indexer
cargo build --release
```

### 3. Launch the Monolithic Daemon
Executing the daemon will automatically spawn two asynchronous threads:
- **The Indexer Loop**: Connects to `127.0.0.1:8332`, scans blocks from 967485, and populates the embedded Sled database.
- **The Axum API Server**: Binds to `0.0.0.0:8080` to serve the Precopscan API.

```bash
# This will create and populate `tusm_indexer.db` in the current directory
./target/release/precop-tusm-indexer
```

### 4. Test the Precopscan API
Once the sync reaches the genesis distributions, you can dynamically query any token balance. The API reads directly from the lock-free Sled instances.

```bash
curl -s "http://127.0.0.1:8080/api/v1/crc20/balance/LEAF/bc1phuuuwf8q3c89ly77m622n59gsv80p9vnt6f4vtr8z345t5xsnldsjks0j9" | jq
```

Expected Output:
```json
{
  "ticker": "LEAF",
  "address": "bc1phuuuwf8q3c89ly77m622n59gsv80p9vnt6f4vtr8z345t5xsnldsjks0j9",
  "balance": "100000000",
  "schema": "v1_liquid"
}
```

# Precop Consensus Indexer — API Specification

The Precop Consensus Indexer exposes a high-throughput, low-latency REST API for querying Bitcoin L1 consensus state, balances, covenants, and multi-asset transfers.

---

## 🚀 Precop Indexer (Port 8080)

### 1. System & Health
#### `GET /v1/health`
Returns the current scanner sync state, scanned block height, Bitcoin network tip, and lag.

**Response `200 OK`**:
```json
{
  "status": "synced",
  "scanned_height": 970474,
  "tip_height": 970474,
  "lag": 0
}
```

---

### 2. Tokens (CRC-20 / Simplicity)
#### `GET /v1/crc20/tokens`
Lists all active token tickers deployed on-chain with their parameters.

**Response `200 OK`**:
```json
[
  {
    "ticker": "LEAF",
    "supply": "10000000000000000",
    "max_supply": "10000000000000000",
    "limit_per_mint": "10000000000000000",
    "decimals": 8,
    "version": 1,
    "halted": false
  },
  {
    "ticker": "CRC",
    "supply": "54429647130000",
    "max_supply": "100000000000000000",
    "limit_per_mint": "10000000000000000",
    "decimals": 8,
    "version": 1,
    "halted": false
  }
]
```

#### `GET /v1/crc20/token/{ticker}`
Returns state details for a specific token.

---

### 3. Balances & Holders
#### `GET /v1/crc20/balance/{ticker}/{address}`
Returns the liquid and total token balance for an address or Taproot covenant script.

**Response `200 OK`**:
```json
{
  "ticker": "LEAF",
  "address": "bc1pttj4n5w5e3f43p6u97k2h6p497a9d0n3p9m...",
  "balance_atoms": "3249952380953",
  "balance_formatted": "32499.52380953"
}
```

#### `GET /v1/crc20/holders/{ticker}`
Returns the paginated holder list with balances.

---

### 4. External Multi-Protocol Payments
#### `GET /v1/crc20/external_payments`
Returns all verified external cross-protocol payments (Runes DOG, BRC-20 ORDI, Inscriptions) registered in the consensus engine.

**Response `200 OK`**:
```json
{
  "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256": {
    "asset": "DOG",
    "amount": 5000000000,
    "amount_display": "50000.0",
    "rate": 47913,
    "protocol": "runes",
    "runeid": "840000:3",
    "symbol": "DOG•GO•TO•THE•MOON",
    "status": "confirmed"
  },
  "d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd": {
    "asset": "ORDI",
    "amount": 1000000000000000000,
    "amount_display": "1.0",
    "decimals": 18,
    "protocol": "brc-20",
    "ticker": "ordi",
    "status": "confirmed"
  }
}
```

---

## 🛠 UniSat Autonomous Sidecar (Port 8085)

### 1. Sidecar Status
#### `GET /health`
Returns sidecar health and count of verified payments.

### 2. Manual Transaction Verification
#### `POST /v1/verify`
Triggers immediate real-time polymorphic verification of a candidate transaction.

**Request**:
```json
{
  "txid": "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256"
}
```

**Response `200 OK`**:
```json
{
  "verified": true,
  "entry": {
    "asset": "DOG",
    "amount": 5000000000,
    "rate": 47913,
    "protocol": "runes",
    "status": "confirmed"
  }
}
```

### 3. Enriched Wallet Bridge
#### `GET /v1/wallet/{address}`
Queries aggregated UTXOs, Runes, and BRC-20 assets for an address directly from UniSat public wallet API without API keys.

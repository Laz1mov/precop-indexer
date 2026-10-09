# AI Playbook & Prompt Engineering Manual for Precop L1

This playbook provides battle-tested, high-precision prompt templates for developers, node operators, quantitative trading engineers, and auditors using Large Language Models (Claude 3.7/Sonnet, GPT-4o, Gemini 1.5 Pro) with the **Precop Consensus Indexer**.

All calculations, formulas, transaction geometries, and constants in this playbook are derived strictly from the canonical consensus implementation in [`tusm-core/src/legacy_v1_router.rs`](tusm-core/src/legacy_v1_router.rs) and verified against Bitcoin Mainnet up to block **970,509**.

---

## 🎯 Playbook Overview

| Template | Objective | Target Model Role | Consensus Focus |
| :--- | :--- | :--- | :--- |
| **Prompt 1** | Zero-Trust Bitcoin L1 Transaction Validator | Elite Bitcoin Consensus Judge | LEAF / CRC-20 Thermodynamic Point Curve |
| **Prompt 2** | CRC-20 Astrolabe & Proof-of-Burn Validator | Cryptographic Seal Verifier | CRC (BTC / DOG Payments & 120 Canonical Mints) |
| **Prompt 3** | Exact BONS Bonding Curve Mathematical Engine | Quantitative Rust Protocol Engineer | BONS $35k USD Integral Curve ($0.00074 → $0.00296) |
| **Prompt 4** | Autonomous Bitcoin Ingestion & Scanner Pipeline | Rust Systems & Concurrency Engineer | Block Streaming, Taproot Script Matching, Sled Batching |
| **Prompt 5** | Sled KV State Consistency & Reorganization Rollbacks | Systems Database & Consensus Specialist | Sled Transactional Trees, Fork Point Rollbacks & UTXO Set Reversion |

---

## 📐 Fundamental Invariant: Atoms vs. Token Units

Across all Precop Layer 1 protocols (CRC-20, CRC-21, Astrolabe, BONS, LEAF, CRC):
- **Precision:** Exactly **8 decimals** ($10^8$ atoms per 1.00000000 token unit).
- **Atoms Formulation:** All internal state variables (`supply`, `max_supply`, `limit_per_mint`, `allocated_atoms`) are strictly represented as **unsigned 128-bit integers ($u128$)**.
- **Conversion Rule:**
  $$\text{Atoms} = \text{Tokens} \times 10^8, \quad \text{Tokens} = \frac{\text{Atoms}}{10^8}$$
- **Amount Parsing (`LegacyV1Router::parse_amount`):**
  - Integer string `"1000"` $\to 1000 \times 10^8 = 100,000,000,000$ atoms.
  - Decimal string `"1.5"` $\to 1 \times 10^8 + 50,000,000 = 150,000,000$ atoms.
  - Python compatibility rule: Fractional part is padded with zeros up to 8 places (`frac.ljust(8, "0")`) and never truncated.

---

## 📋 Prompt 1: Zero-Trust Bitcoin L1 Transaction Validator (LEAF Thermodynamic)

Use this prompt to verify raw Bitcoin transaction hex against the canonical LEAF thermodynamic bonding ruleset:

```markdown
You are an expert Bitcoin L1 Consensus Judge specialized in the Precop Protocol and the Legacy V1 Router (`tusm-core/src/legacy_v1_router.rs`).

### INPUT
Raw Bitcoin Transaction Hex:
```hex
<INSERT_RAW_TX_HEX_HERE>
```

Canonical Protocol State (LEAF Deployment):
- Token Ticker: `LEAF`
- Protocol Standard: `crc-20` (Thermodynamic Bonding Curve / Legacy Migration v3)
- Deploy TXID: `546cc042d0f396a0d8ad67b6987d9d5c09619e6962738347ca1611a1d1841b67` (Block 967,485)
- Decimals: 8 (1 LEAF = 100,000,000 atoms = 10^8 atoms)
- Max Supply: 1,000,000,000.00 LEAF = 100,000,000,000,000,000 atoms (10^17 atoms)
- Point Limit (`lim` / `point_limit`): 2,100,000,000 points
- Saturation Status (at block 970,509): 100% Saturated (consumed_points = 2,100,000,000, supply = 100,000,000,000,000,000 atoms)
- Treasury Address: `bc1phuuulh7fs5zrm48ethfyqvt860fxsaxuq643telqn06yz4u3c70spyleaf`
  (Script: `5120bf39cfdfc985043dd4f95dd2403167d3d26874dc06ab15e7e09bf4415791c79f`)
- BIP-341 NUMS Internal Public Key ($H$): `50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`

Payment Asset Weights & Scaling:
- BTC: weight = 3,333,333, unit_scale = 100,000,000 sats (1 sat = 1 atom of BTC)
- ORDI: weight = 286, unit_scale = 1,000,000,000,000,000,000 (10^18 atoms)
- LEAF (legacy ico-20 migration burn to Satoshi sink 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa): weight = 1, unit_scale = 100,000,000 atoms

Lock Multipliers (Denominator = 10,000,000):
- 144 blocks CSV: multiplier = 16,180,339 (1.6180339x)
- 1000 blocks CSV: multiplier = 20,000,000 (2.0000000x)
- 2100 blocks CSV: multiplier = 22,720,196 (2.2720196x)
- 6767 blocks CSV: multiplier = 26,180,339 (2.6180339x)

### VERIFICATION PROCEDURE
Perform step-by-step verification:
1. **Decode Transaction Structure**:
   - Extract vin[0..n], vout[0..m], locktime, and input witness stacks.
   - Extract the owner x-only public key from vin[0] prevout.

2. **Validate OP_RETURN Speech**:
   - Locate the OP_RETURN scriptPubkey.
   - Parse JSON payload: MUST contain `"p":"crc-20"` and `"tick":"LEAF"`.
   - IF `"op":"mint"`, verify that NO `"amt"` field is present.
   - IF `"op":"transfer"`, verify that an explicit `"amt"` field is present and valid.

3. **Verify Taproot Seal Output**:
   - Find output with value >= 330 satoshis matching `LegacyV1Router::derive_taproot_for_lock`:
     Leaf script: `<lock> OP_CSV OP_DROP <owner_xonly_pubkey> OP_CHECKSIG`
     Tweaked with BIP-341 NUMS point `50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`.
   - Identify the matched CSV lock delay (144, 1000, 2100, or 6767).

4. **Verify Treasury & Payment Asset**:
   - Identify deposited payment atoms (sats sent to Treasury or burnt legacy LEAF).
   - If payment is BTC, ensure Treasury output matches `bc1phuuulh...`.

5. **Compute Exact Point & Atom Allocation (Integer Floor Math)**:
   - `requested_points = floor((payment_atoms * weight * lock_multiplier) / (unit_scale * 10,000,000))`
   - `accepted_points = min(requested_points, point_limit - current_consumed_points)`
   - IF `accepted_points == 0` (e.g. saturation cap reached): REJECT with reason `"reserve_exhausted"`.
   - `new_consumed_points = current_consumed_points + accepted_points`
   - `supply_after = floor((new_consumed_points * max_supply_atoms) / point_limit)`
   - `allocated_atoms = supply_after - current_supply_atoms`

6. **Output Structured Verdict**:
```json
{
  "verdict": "ACCEPTED" | "REJECTED",
  "reason": "...",
  "matched_lock": 2100,
  "lock_multiplier": 22720196,
  "payment_asset": "BTC",
  "payment_atoms": 100000,
  "requested_points": 757339,
  "accepted_points": 757339,
  "allocated_atoms": 36063761904,
  "allocated_tokens": "360.63761904 LEAF",
  "new_consumed_points": 757339,
  "new_supply_atoms": 36063761904
}
```
```

---

## 🔬 Prompt 2: CRC-20 Astrolabe & Proof-of-Burn Validator (CRC Token)

Use this prompt to verify CRC token mint transactions against the canonical 120-mint consensus ledger:

```markdown
You are an expert Bitcoin L1 Consensus Judge specialized in CRC-20 Astrolabe Proof-of-Burn consensus (`tusm-core/src/legacy_v1_router.rs`).

### INPUT
Raw Bitcoin Transaction Hex:
```hex
<INSERT_RAW_TX_HEX_HERE>
```

Canonical Protocol State (CRC Token):
- Token Ticker: `CRC`
- Protocol Standard: `crc-20` (Proof-of-Burn Astrolabe)
- Decimals: 8 (1 CRC = 100,000,000 atoms)
- Max Supply: 1,000,000,000.00 CRC = 100,000,000,000,000,000 atoms (10^17 atoms)
- Minted Supply (at block 970,509): 162,053,369.45 CRC = 16,205,336,945,176,514 atoms (120/120 canonical mints verified)
- CRC Treasury Script: `5120382b79806cc956d6f88236a8fdd70b7bfce89c2745b04f8f694df2302a853f92`
- DOG Canonical Exchange Rate: 47,917 (authenticated via RFC 2104 HMAC-SHA256)
- BTC Canonical Rate: 3,333,333

Lock Multipliers for CRC:
- 144 blocks CSV: multiplier = 8,090,170
- 1000 blocks CSV: multiplier = 10,000,000
- 2100 blocks CSV: multiplier = 11,360,100
- 6767 blocks CSV: multiplier = 13,090,170

### MATHEMATICAL FORMULAS
1. **BTC Payment Allocation**:
   $$\text{allocated\_atoms} = \left\lfloor \frac{\text{sats} \times \text{btc\_rate} \times 2 \times \text{lock\_multiplier}}{100,000,000,000} \right\rfloor$$
2. **DOG Payment Allocation**:
   $$\text{allocated\_atoms} = \left\lfloor \frac{\text{dog\_units} \times \text{applied\_rate} \times \text{lock\_multiplier}}{50,000,000} \right\rfloor$$
3. **Supply Saturation & Mint Cap**:
   - `allocated_atoms = min(allocated_atoms, limit_per_mint)`
   - `final_allocated = min(allocated_atoms, max_supply - current_supply)`

Evaluate the transaction hex and output a detailed audit report.
```

---

## 📈 Prompt 3: Exact BONS Bonding Curve Mathematical Engine

Use this prompt to compute exact bonding curve prices, slippage, and optimal satoshi deposit size for BONS tokens:

```markdown
You are a Quantitative DeFi Analyst specialized in Precop Sub-Millisecond Bonding Curves and 256-bit safe integer arithmetic.

### CANONICAL BONS BONDING CURVE PARAMETERS
From `tusm-core/src/legacy_v1_router.rs`:
- Token: `BONS` (Covenant Migration)
- Decimals: 8 (1 BONS = 100,000,000 atoms)
- Total Saturated Cap ($S_{\text{max}}$): 21,000,000.00 BONS = 2,100,000,000,000,000 atoms
- Target USD Raised: $35,000.00 = 3,500,000,000,000 USD atoms (8 decimals)
- BTC/USD Oracle Price: $90,200.00 (1 satoshi = 90,200 USD base units / 10^8)
- Initial Price ($P_0$): $0.00074 USD / BONS (scaled: 7,400,000,000,000)
- Final Price ($P_1$): $0.00296 USD / BONS (scaled: 29,600,000,000,000)
- Price Delta: 22,200,000,000,000
- Maximum Satoshi Deposit per TX: 150,000 satoshis (capped at consensus level)

### EXACT RUST FORMULATION (LegacyV1Router)
```rust
let target_usd_atoms = U256::from(3_500_000_000_000u64);
let p_sats = U256::from(payment_atoms);
let usd_atoms = p_sats * U256::from(90_200u64);
let sold_usd_atoms = U256::from(current_consumed_points);

if sold_usd_atoms < target_usd_atoms {
    let rem_usd = target_usd_atoms - sold_usd_atoms;
    let usd_capped = std::cmp::min(usd_atoms, rem_usd);
    let mid_usd = sold_usd_atoms + (usd_capped / U256::from(2u64));
    
    let price_scaled = U256::from(7_400_000_000_000u64) 
        + (U256::from(22_200_000_000_000u64) * mid_usd) / target_usd_atoms;
        
    let token_atoms = (usd_capped * U256::from(10_000_000_000_000_000u64)) / price_scaled;
    let allocated = std::cmp::min(token_atoms.as_u128(), max_supply.saturating_sub(current_supply));
}
```

### TASK
Given `<DEPOSIT_SATS>` satoshis and `<CURRENT_CONSUMED_USD_ATOMS>`:
1. Calculate `usd_capped`, `mid_usd`, and `price_scaled`.
2. Compute `token_atoms` and convert to token units (`token_atoms / 1e8`).
3. Compute the effective execution price in satoshis per BONS.
4. Calculate slippage relative to the instantaneous spot price.
5. Provide executable Rust unit test code integrating `tusm-core::legacy_v1_router::LegacyV1Router` verifying exact bitwise allocation and consensus bounds.
```

---

## 🌐 Prompt 4: Autonomous Rust Bitcoin Ingestion & Scanner Pipeline

Use this prompt to build or optimize asynchronous block streaming and Taproot script scanning in `tusm-indexer`:

```markdown
You are a Senior Rust Systems & Network Engineer specialized in Bitcoin Core RPC and high-throughput Tokio pipelines.

### GOAL
Implement an asynchronous, fault-tolerant block streaming and transaction scanning engine for `tusm-indexer/src/scanner_engine.rs` and `bridge_client.rs`.

### REQUIREMENTS
1. Connect to Bitcoin Core JSON-RPC (`bitcoincore-rpc`) with credential authentication and exponential backoff on connection drops.
2. Prefetch blocks in parallel using a bounded sliding window (e.g. 32 blocks) without exhausting system memory.
3. For each block, extract all transactions:
   - Identify OP_RETURN scriptPubKeys and parse protocol speeches (`crc-20` / `crc-21`).
   - Enforce the invariant: if `"op":"mint"`, verify that NO `"amt"` field is present.
   - Match Taproot seal outputs (>= 330 satoshis) against the BIP-341 NUMS internal key `50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`.
4. Apply state transitions inside an atomic `sled::transaction` block.
5. Handle SIGINT and SIGTERM gracefully, flushing the Sled database to disk before exiting.
6. Provide clean, production-grade Rust code adhering to `#![forbid(unsafe_code)]`.
```

---

## 🛠️ Prompt 5: Sled KV State Consistency & Reorganization Rollbacks

Use this prompt to verify Sled database consistency, detect chain forks, and implement atomic rollback routines:

```markdown
You are a Systems Database and Blockchain Reliability Specialist maintaining the Sled embedded key-value storage engine in `tusm-indexer/src/state_db.rs`.

### TASK
Implement and test a zero-corruption block reorganization rollback engine.

### SPECIFICATION & DATA LAYOUT
- Storage Engine: Embedded Sled (`sled::Db`, `sled::Tree`).
- Trees:
  - `block_meta`: block height $\to$ block hash, timestamp, and undo-log pointer.
  - `balances`: `<ticker>:<address>` $\to$ `u128` atoms.
  - `sigma_leaf`: `<ticker>` $\to$ serialized `SigmaLeaf` state (supply, consumed points).
  - `utxos`: `<txid>:<vout>` $\to$ spent status and prevout metadata.
  - `reorg_history`: `<height>` $\to$ serialized transactional delta mutations.

### INVARIANTS TO ENFORCE
1. Reorg Detection: When Bitcoin Core returns a new block whose `prev_block_hash` does not match the local tip hash, locate the common ancestor.
2. Atomic Rollback: In a single Sled transaction, unwind each reverted block in reverse chronological order:
   - Re-credit debited balances and subtract minted token atoms.
   - Restore previous `consumed_points` and `supply` in `sigma_leaf`.
   - Re-open spent UTXOs and delete created UTXOs.
3. Verification: Assert that after rolling back $N$ blocks and replaying the winning branch, state hashes converge exactly.
4. Output a comprehensive Rust test module proving rollback safety across deep (>= 6 blocks) reorgs.
```

---

## 🧪 Automated Prompt Verification & CI/CD with Promptfoo

This repository includes a production-ready **Promptfoo** evaluation harness ([`promptfooconfig.yaml`](promptfooconfig.yaml)) ensuring that consensus prompts and AI models adhere strictly to Precop Layer 1 rules without hallucinations.

### 1. Run the Test Suite (Local / Deterministic WASM Oracle)

Execute the 5-test consensus evaluation matrix directly in your terminal:

```bash
npx promptfoo eval --no-cache
```

### 2. Evaluated Test Invariants

| Test Case | Attack / Invariant Vector | Expected Verdict | Assertion |
| :--- | :--- | :--- | :--- |
| **Genesis Deploy** | Block 967,485 Mainnet LEAF deploy TX | `ACCEPTED` | Validates `event_type === 'deploy'` and zero initial atoms |
| **Speech Tampering** | Hostile mint containing forbidden `"amt"` field | `REJECTED` | Catches `"amt"` key violation immediately |
| **Point Exhaustion** | Mint attempted after `2,100,000,000` point hardcap | `REJECTED` | Enforces `reserve_exhausted` boundary clamp |
| **Malformed Payload** | Non-hex corrupted transaction bytes | `REJECTED` | Zero unhandled panics or crashes |
| **Non-Protocol UTXO** | Bitcoin transaction lacking OP_RETURN speech | `REJECTED` | Rejects non-conforming transactions |

### 3. Evaluate Live LLMs (Claude, GPT-4o, Gemini)

To test whether external commercial frontier models adhere to this playbook:

```bash
# Evaluate Claude 3.7 Sonnet
export ANTHROPIC_API_KEY="your-key"
npx promptfoo eval --providers anthropic:messages:claude-3-7-sonnet-20250219

# Evaluate GPT-4o
export OPENAI_API_KEY="your-key"
npx promptfoo eval --providers openai:gpt-4o

# Evaluate Gemini 1.5 Pro
export GEMINI_API_KEY="your-key"
npx promptfoo eval --providers google:gemini-1.5-pro
```

### 4. Interactive Web UI

Launch the Promptfoo browser dashboard to review matrices, assertion traces, and token costs:

```bash
npx promptfoo view
```


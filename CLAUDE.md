# CLAUDE.md — AI Agent & Developer Protocol Manual for Precop Indexer

## 🧠 Persona & System Context

You are an **Elite Bitcoin Core, Taproot Script, and Rust Systems Engineer** acting as a core maintainer of the **Precop Consensus Indexer** (`precop-indexer`).
Your expertise encompasses:
- Bitcoin Layer 1 protocol internals (BIP-340 Schnorr, BIP-341 Taproot, BIP-342 Tapscript, OP_CSV relative timelocks, OP_RETURN data pushes, UTXO sets, and chain reorganizations).
- Industrial-grade Systems Rust (Edition 2021, `#![forbid(unsafe_code)]`, zero-allocation parsers, SIMD/256-bit safe arithmetic, memory safety without GC).
- Embedded high-throughput key-value persistence with **Sled** (`sled::Db`, `sled::Tree`, transactional ACID commits).
- Bitcoin Core JSON-RPC block streaming (`bitcoincore-rpc`) with high-concurrency Tokio pipelines and backpressure.

---

## 🏗️ Workspace Architecture & Crate Topology

```
precop-indexer/
├── tusm-core/          # Pure deterministic consensus logic (#![forbid(unsafe_code)])
│   ├── src/
│   │   ├── legacy_v1_router.rs   # Core L1 consensus state machine, bonding math & Taproot logic
│   │   ├── crc_canonical.rs      # Canonical mint history & historical proof-of-burn verifiers
│   │   ├── crc21_v2.rs           # CRC-21 Sovereign Taproot Vault & Parent-Child UTXO rules
│   │   ├── astrolabe.rs          # Astrolabe proof-of-burn verification engine
│   │   ├── attestation.rs        # RFC 2104 HMAC-SHA256 & BIP-340 Schnorr oracle attestations
│   │   ├── law.rs                # Axiomatic consensus laws & mathematical bounds
│   │   └── prevout.rs            # Outpoint resolution & Prevout cache models
│   └── tests/                    # Proptests, geometry tests, and 120-mint golden vectors
├── tusm-indexer/       # High-throughput Bitcoin L1 indexing daemon & REST API
│   ├── src/
│   │   ├── main.rs               # Daemon CLI entrypoint (CLI flags, signal handling)
│   │   ├── scanner_engine.rs     # Block-by-block scanner, script evaluator & state transitioner
│   │   ├── state_db.rs           # Sled embedded database engine & atomic rollback logs
│   │   ├── bridge_client.rs      # Bitcoin Core RPC client with exponential backoff
│   │   ├── block_bridge.rs       # Parallel block prefetching pipeline
│   │   ├── api.rs                # High-performance Actix-web REST API (/v1/crc20, /v1/crc21, /v1/health)
│   │   ├── crc21_engine.rs       # Sovereign Taproot Vault state tracker & CBOR envelope parser
│   │   └── vault_watch.rs        # Active CSV timelock monitor for covenant vault UTXOs
│   └── bin/                      # Verification and diagnostic binaries (import_snapshot, query_db, etc.)
├── tusm-sidecar/       # Autonomous UniSat sidecar & inscription watcher
├── precop-cli/         # Autonomous operator CLI tool (`precop`)
├── tusm-judge/         # Standalone WASM/native consensus evaluation judge
└── tusm-witness/       # Witness stack parser and Simplicity script analyzer
```

---

## 🔒 Axiomatic Consensus Rules & Non-Negotiable Invariants

When writing, auditing, or refactoring code in `precop-indexer`:

### 1. Absolute Zero-Unsafe Invariant
- The core crate `tusm-core` enforces `#![forbid(unsafe_code)]`.
- Never introduce `unsafe` blocks, raw pointer dereferences, or undefined memory behavior anywhere in the consensus pipeline.

### 2. Protocol Speech Invariant (Mint vs. Transfer)
- **Mint Speech (`"op":"mint"`)**:
  - Format: `{"p":"crc-20","op":"mint","tick":"<TICKER>"}` (or `crc-21`).
  - 🚨 **CRITICAL RULE**: The JSON payload **MUST NEVER CONTAIN AN `"amt"` FIELD**.
  - If `"amt"` is present in a mint speech, the indexer must immediately reject the transaction as invalid.
  - Token allocation is **strictly covenant-derived**: calculated deterministically from the satoshis deposited in the Treasury output, the matched CSV lock multiplier, and remaining supply saturation.
- **Transfer Speech (`"op":"transfer"`)**:
  - Format: `{"p":"crc-20","op":"transfer","tick":"<TICKER>","amt":"<AMOUNT>"}`.
  - An explicit `"amt"` field is **MANDATORY**.

### 3. Taproot (BIP-341/342) Covenant Derivation & Seal Geometry
- **Internal Public Key ($H$)**: Must use the unspendable BIP-341 NUMS (Nothing-Up-My-Sleeve) x-only public key:
  `50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`
- **Tapscript Leaf Geometry**:
  `OP_PUSHBYTES_<len> <lock_blocks_le> OP_CSV OP_DROP <owner_xonly_pubkey> OP_CHECKSIG`
- **Taproot Tweak**: Tweaked output key $Q = H + \text{TaggedHash}_{\text{TapTweak}}(H \parallel \text{MerkleRoot}) \cdot G$.
- **Seal Output Value**: Must be at least **330 satoshis** (dust protection threshold).
- **Exact Timelock Multipliers (Denominator = $10,000,000$)**:
  - `144` blocks CSV $\to$ multiplier `16_180_339` ($1.6180339\times$)
  - `1000` blocks CSV $\to$ multiplier `20_000_000` ($2.0000000\times$)
  - `2100` blocks CSV $\to$ multiplier `22_720_196` ($2.2720196\times$)
  - `6767` blocks CSV $\to$ multiplier `26_180_339` ($2.6180339\times$)

### 4. Mathematical Precision: Atoms vs. Token Units
- **Fixed-Point Precision**: Exactly **8 decimal places** ($10^8$ atoms per 1.00000000 token unit).
- **State Integer Types**: All internal balances, supplies, and point limits are unsigned 128-bit integers (`u128`).
- **Bonding Curve Integral Math**: Intermediate multiplication during curve computation must use `primitive_types::U256` to completely prevent arithmetic overflow before downcasting to `u128`.
- **Clamping**: Always use `saturating_add`, `saturating_sub`, and clamp strictly to `max_supply`.
- **Parsing String Amounts**:
  - Decimal strings must be parsed without floating-point inaccuracies. Fractional parts must be padded up to 8 digits (e.g., `"1.5"` $\to 150,000,000$ atoms). Never cast directly through `f64`.

### 5. Sled KV Storage & Reorganization Atomicity
- All state changes for a block must be executed inside a `sled::transaction` block.
- For every block ingested, record reverse operations (undo-log) in the `reorg_history` tree so the chain tip can be cleanly rolled back upon detecting a reorganization.

---

## 🛠️ Developer & Operator Cheatsheet

### 1. Compiling & Running the Workspace
```bash
# Build the entire workspace in release mode
cargo build --workspace --release

# Run the full indexer daemon connecting to Bitcoin Core RPC
cargo run -p precop-tusm-indexer --release -- \
  --rpc-url http://127.0.0.1:8332 \
  --rpc-user your_rpc_username \
  --rpc-pass your_rpc_password \
  --start-block 967485 \
  --db-path ./data/tusm_indexer.db
```

### 2. Testing & Formal Verification
```bash
# Run all workspace unit and integration tests
cargo test --workspace

# Run canonical 120-mint exact convergence tests
cargo test -p tusm-core --test tdd_crc_template_matching

# Run property-based invariant fuzzing (Anti-Black-Swan proptests)
cargo test -p tusm-core --test proptests_consensus_invariants

# Check strict Clippy rules (zero warnings permitted)
cargo clippy --workspace --lib --tests -- -D warnings

# Measure exact code coverage with LLVM
cargo llvm-cov -p tusm-core

# Execute mutation testing on consensus arithmetic
cargo mutants --package tusm-core

# Security dependency audit
cargo audit
```

### 3. Consensus LLM Evaluation (Promptfoo)
```bash
# Run the automated 5-test consensus evaluation matrix
npx promptfoo eval --no-cache

# Launch the visual dashboard
npx promptfoo view
```

---

## 🤖 Elite LLM System Prompts for Indexer Engineering

Use these tailored system prompts when instructing AI assistants to implement, review, or debug indexer modules:

### Prompt A: "Bitcoin L1 Taproot Script & Covenant Validator"
```text
System: You are an expert Bitcoin Core and Taproot Script consensus engineer working on `tusm-core/src/legacy_v1_router.rs`.
Task: Implement and verify the Taproot seal validator for Bitcoin transaction inputs and outputs.
Constraints:
1. Enforce #![forbid(unsafe_code)].
2. Derive the Taproot tweaked output using internal NUMS key `50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`.
3. Support CSV lock durations: 144, 1000, 2100, and 6767 blocks with their canonical 7-decimal multipliers over denominator 10,000,000.
4. If the OP_RETURN scriptPubkey contains "op":"mint", assert that NO "amt" key exists. Return an explicit RejectReason::MintWithAmtForbidden if found.
5. Compute token atom allocation using 256-bit safe integer arithmetic (U256).
```

### Prompt B: "Sled State Database & Chain Reorganization Engine"
```text
System: You are a Systems Database and Blockchain Reliability Specialist working on `tusm-indexer/src/state_db.rs` and `scanner_engine.rs`.
Task: Implement a fault-tolerant block reorganization rollback mechanism.
Requirements:
1. When Bitcoin Core signals a new block with a different prev_block_hash, detect the fork point.
2. In a single atomic Sled transaction, pop blocks from the tip down to the fork point.
3. For each reverted block, apply the inverse mutations to:
   - `balances` tree (re-credit debits, subtract minted atoms).
   - `sigma_leaf` tree (restore previous consumed_points and supply).
   - `utxos` tree (re-open spent prevouts, prune newly created UTXOs).
4. Return an explicit Result<(), ReorgError> without unwrapping.
```

### Prompt C: "Consensus Invariant Prover (Property-Based Testing)"
```text
System: You are a Formal Methods and Cryptography Researcher writing proptest suites in `tusm-core/tests/proptests_consensus_invariants.rs`.
Task: Write a generative property test verifying that for any arbitrary sequence of 1,000 random mints, transfers, and burns:
1. Total circulating supply never exceeds `max_supply`.
2. Sum of all individual account balances matches `current_supply` bit-for-bit.
3. Negative balances are structurally unrepresentable (u128 floor).
4. No integer overflow or panic occurs under u128::MAX boundary inputs.
```

### Prompt D: "High-Throughput Bitcoin RPC Streaming Pipeline"
```text
System: You are a Senior Network Systems Engineer working on `tusm-indexer/src/bridge_client.rs` and `block_bridge.rs`.
Task: Optimize the block fetching pipeline using Tokio asynchronous channels and bitcoincore-rpc.
Requirements:
1. Implement a sliding-window block prefetcher (window size: 32 blocks).
2. Handle Bitcoin Core RPC rate limits and connection drops with exponential backoff and jitter.
3. Stream decoded blocks into `scanner_engine.rs` via a bounded mpsc channel.
4. Ensure graceful shutdown on SIGINT/SIGTERM without leaving Sled in an uncommitted state.
```

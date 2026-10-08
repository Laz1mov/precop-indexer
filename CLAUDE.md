# CLAUDE.md — AI Agent & Developer Guidelines for Precop Indexer

## Project Overview

The **Precop Consensus Indexer** (`precop-indexer`) is an industrial-grade Bitcoin Layer 1 state engine written in Rust. It tracks Simplicity covenants, native Taproot CSV seals, CRC-20 tokens, and CRC-21 sovereign vaults directly from Bitcoin blocks.

---

## 🏛️ Architectural Invariants & Non-Negotiable Rules

When modifying, auditing, or generating code in this repository:

1. **Strict Safe Rust (`#![forbid(unsafe_code)]`)**:
   - `tusm-core` MUST NEVER contain `unsafe` blocks.
   - All state transitions, pointer arithmetic, and parsers must rely purely on safe Rust and compiler borrow-checking.

2. **Protocol Speech Invariant (Mint vs. Transfer)**:
   - **Mint**: `{"p":"crc-20","op":"mint","tick":"<TICKER>"}` — **NEVER INCLUDE `amt`!** Token allocation is strictly covenant-derived from the satoshis in the treasury output and the CSV lock multiplier.
   - **Transfer**: `{"p":"crc-20","op":"transfer","tick":"<TICKER>","amt":"<AMOUNT>"}` — **REQUIRES `amt`!**

3. **Supply Monotonicity & Overflow Defense**:
   - Always use saturated arithmetic (`saturating_add`, `saturating_sub`) or `primitive_types::U256` for bonding curve math.
   - Total supply must never exceed `max_supply`.

4. **Cryptographic Attestation & Answering**:
   - Off-chain attestations require true RFC 2104 HMAC-SHA256 (`compute_hmac_rfc2104`).
   - Oracle rate feeds require BIP-340 Schnorr signatures against `CANONICAL_ORACLE_PUBKEY`.

---

## 💻 Essential Developer Commands

```bash
# 1. Build the full workspace
cargo build --workspace

# 2. Run all tests across the workspace
cargo test --workspace

# 3. Run Property-Based Invariant Tests (Anti-Black-Swan proptests)
cargo test -p tusm-core --test proptests_consensus_invariants

# 4. Measure exact code coverage
cargo llvm-cov -p tusm-core

# 5. Security audit of dependencies
cargo audit

# 6. Statistical benchmarking (Criterion)
cargo bench -p tusm-core -- --test

# 7. Strict Clippy check
cargo clippy -p tusm-core --lib -- -D warnings
```

---

## 🤖 Prompt Engineering Strategies for LLMs

When prompting AI assistants to work on this repository, use the following patterns:

### Pattern A: "Zero-Trust Transaction Auditor"
```text
System: You are a strict Bitcoin L1 Consensus Verifier. Your task is to evaluate raw Bitcoin transaction hexadecimal payloads against the Precop Sovereign Consensus Specification.
Input: <RAW_TX_HEX>
Instructions:
1. Decode inputs, outputs, and OP_RETURN scriptPubKeys.
2. Check if a valid Taproot seal output (>= 330 sats) exists, derived via `LegacyV1Router::derive_taproot_for_lock`:
   Leaf: `<lock> OP_CSV OP_DROP <owner_xonly_pubkey> OP_CHECKSIG`, internal key: BIP-341 NUMS (`50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`).
3. If OP_RETURN contains "op":"mint", verify that NO "amt" key is present in JSON.
4. Calculate token allocation in atoms (u128, 8 decimals) using matched CSV lock multiplier (144 -> 16180339, 1000 -> 20000000, 2100 -> 22720196, 6767 -> 26180339 / 10000000) and point curve formulas.
5. Output formal Verdict: ACCEPTED or REJECTED(Reason) with exact atom delta.
```

### Pattern B: "Consensus Invariant Prover"
```text
System: You are an Applied Cryptography Researcher specializing in Rust formal verification.
Task: Write a proptest theorem in tusm-core/tests/ proving that state property X holds across 10,000 randomized state vectors without panicking.
Requirements: Use #![forbid(unsafe_code)], U256 safe math, and handle boundary values (0, u128::MAX).
```

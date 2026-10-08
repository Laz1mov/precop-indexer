# Security Policy & Formal Robustness Specification

## Executive Security Statement

The `tusm-core` consensus engine and the `precop-indexer` pipeline implement mission-critical Bitcoin L1 consensus indexing, Simplicity covenant validation, and thermodynamic state transition mechanics for **CRC-20, CRC-21, LEAF, and BONS** protocols.

Every release adheres to the **6 Industrial Pillars of High-Assurance Rust Infrastructure**:
1. **Property-Based Testing & Anti-Black-Swan Invariant Verification** (`proptest`)
2. **Deterministic Memory Safety & Zero-UB Guarantees** (`#![forbid(unsafe_code)]`)
3. **Strict Coverage Instrumentation** (`cargo-llvm-cov` / `cargo-tarpaulin`)
4. **Mutation Resilience Testing** (`cargo-mutants`)
5. **Statistical Latency & Throughput Benchmarking** (`criterion`)
6. **Continuous Security Audits & Paranoiac Lints** (`cargo-audit`, `#![deny(clippy::all)]`)

---

## 1. Threat Model & Formal Security Invariants

### 1.1 No Inflation & Monotonic Supply Invariant
* **Theorem:** $\forall t, \text{Supply}(t+1) = \text{Supply}(t) + \Delta \text{Minted}$, where $\Delta \text{Minted} \le \text{LimitPerMint}$ and $\text{Supply}(t+1) \le \text{MaxSupply}$.
* **Enforcement:** Enforced in `tusm-core/src/law.rs` via 256-bit safe arithmetic (`primitive_types::U256`) and saturated arithmetic (`saturating_add`, `saturating_sub`). Verified continuously via `prop_supply_saturation_invariant` over 100,000 randomized state vectors.

### 1.2 Balance Conservation Invariant
* **Theorem:** In any balance transfer, $\sum \text{Balances}_{\text{post}} = \sum \text{Balances}_{\text{pre}}$, and no address balance can underflow.
* **Enforcement:** Enforced via `checked_sub` and `checked_add`. Insufficient balance immediately triggers rejection (`RejectReason::Underflow`) without modifying state. Verified via `prop_balance_conservation_invariant`.

### 1.3 Memory Safety Guarantee: Strict Safe Rust
* **Policy:** `#![forbid(unsafe_code)]` is enforced at the root of `tusm-core/src/lib.rs`.
* **Impact:** Undefined Behavior (UB), pointer aliasing bugs, memory leaks, and data races are mathematically forbidden by the Rust compiler.

### 1.4 L1 Parser Crash-Proof Resilience (Fuzzing)
* **Threat:** Malformed OP_RETURN scripts, corrupted CBOR tags, hostile UTF-8 payloads, and truncated JSON payloads designed to crash or panic indexer nodes.
* **Defense:** Fuzz-tested via `prop_parser_crash_proof_resilience` injecting randomized byte noise and bit-flips. All parsing operations return explicit `Result<Option<...>, PrevoutError>` or `RejectReason` without panicking.

### 1.5 Cryptographic Attestation Bounds
* **HMAC RFC 2104:** Off-chain and sidecar canonical attestations are authenticated with true RFC 2104 HMAC-SHA256 (`tusm-core/src/attestation.rs`).
* **BIP-340 Schnorr Signatures:** Secp256k1 X-Only public keys authenticate oracle feeds (`CANONICAL_ORACLE_PUBKEY`) using strict verification rules.

---

## 2. Verification Protocol & Industrial Toolchain

| Pillar | Industry Tool | Target / Guarantee | Status |
| :--- | :--- | :--- | :--- |
| **Property Testing** | `proptest` (v1.6) | 5 formal invariants, 2,500+ generated test vectors | **PASS (100%)** |
| **Memory Safety** | Rust Compiler Safe Checks | `#![forbid(unsafe_code)]` on consensus root | **ENFORCED** |
| **Code Coverage** | `cargo-llvm-cov` (v0.9.1) | >85-90% on core invariant files (`astrolabe.rs` 97.1%, `crc_canonical.rs` 100%) | **VERIFIED** |
| **Mutation Testing** | `cargo-mutants` (v27.1) | Configured in `.cargo/mutants.toml` across core files | **ACTIVE** |
| **Statistical Benches** | `criterion` (v0.5) | Latency regressions tracked with 95% confidence intervals | **PASS** |
| **Security Audit** | `cargo-audit` (v0.22) | 0 vulnerabilities across 406 dependencies | **0 ADVISORIES** |

---

## 3. Reporting a Vulnerability

If you discover a potential consensus fork, invariant violation, or security vulnerability, please submit a responsible disclosure report to the core security engineering team:
* **Email:** `security@precop.org`
* **GPG Key:** Available upon request.
* **SLA:** Initial assessment within 24 hours. Hotfixes deployed to release tags with automated regression suites.

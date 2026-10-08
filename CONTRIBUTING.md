# Contributing to Precop Consensus Indexer

Thank you for your interest in contributing to the **Precop Consensus Indexer and Sovereign Layer 1 Engine**.

This repository implements consensus-critical Bitcoin Layer 1 indexing, Simplicity covenants, and thermodynamic state transition mechanics for CRC-20, CRC-21, LEAF, and BONS protocols. Because bugs in consensus engines can lead to chain desynchronization, the project enforces strict quality and mathematical verification standards.

---

## 🏛️ Consensus Development Rules

1. **Strictly Safe Rust**:
   The consensus library (`tusm-core`) enforces `#![forbid(unsafe_code)]`. Any PR introducing `unsafe` in consensus paths will be automatically rejected.

2. **Property-Based Testing (Anti-Black-Swan)**:
   Every new state transition, numerical formula, or parsing rule must be accompanied by property-based tests (`proptest`) generating thousands of pseudo-random hostile vectors.

3. **High Code Coverage (>85-90%)**:
   Consensus modules must maintain high line and branch coverage. Validate before submitting:
   ```bash
   cargo llvm-cov -p tusm-core
   ```

4. **Zero Compiler & Clippy Warnings**:
   Code must compile cleanly without warnings:
   ```bash
   cargo clippy -p tusm-core --lib -- -D warnings
   ```

5. **No Regressions on Canonical Convergence**:
   Existing canonical convergence vectors (CRC `162,053,369.45 CRC` 120/120 mints, LEAF `100,000,000.00 LEAF`, BONS `21,000,000.00 BONS`) must remain bit-exact.

---

## 🛠️ Development Setup

1. **Install Toolchain**:
   ```bash
   rustup toolchain install stable
   rustup component add clippy llvm-tools-preview
   ```

2. **Install Security Tools**:
   ```bash
   cargo install cargo-llvm-cov cargo-audit cargo-mutants
   ```

3. **Run the Full Test Suite**:
   ```bash
   cargo test --workspace
   ```

4. **Run Property Invariants**:
   ```bash
   cargo test -p tusm-core --test proptests_consensus_invariants
   ```

5. **Run Security Audit**:
   ```bash
   cargo audit
   ```

---

## 🚀 Submitting Pull Requests

1. Fork the repository on GitHub.
2. Create a scoped feature branch (`git checkout -b feature/consensus-improvement`).
3. Commit your changes with clear, semantic commit messages.
4. Ensure all CI jobs pass locally (`cargo test`, `cargo clippy`, `cargo audit`).
5. Open a Pull Request on GitHub targeting `main`.

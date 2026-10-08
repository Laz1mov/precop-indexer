# AI Playbook & Prompt Engineering Manual for Precop L1

This playbook provides battle-tested, high-precision prompt templates for developers, node operators, and trading engineers using LLMs (Claude 3.7/Sonnet, GPT-4o, Gemini 1.5 Pro) with the **Precop Consensus Indexer**.

---

## 🎯 Playbook Overview

| Template | Objective | Target Model Role |
| :--- | :--- | :--- |
| **Prompt 1** | Zero-Trust Transaction Validation | Elite Bitcoin Consensus Judge |
| **Prompt 2** | Bonding Curve Arbitrage & Slippage Math | Quantitative DeFi Engineer |
| **Prompt 3** | Indexer REST API Query & Client Generator | Fullstack TypeScript/Python Engineer |
| **Prompt 4** | Node Health & Reorganization Diagnostics | Site Reliability & DevOps Specialist |

---

## 📋 Prompt 1: Zero-Trust Bitcoin L1 Transaction Validator

Use this prompt when providing raw transaction hex to verify if it satisfies Precop consensus rules before broadcasting to the Bitcoin mempool:

```markdown
You are an expert Bitcoin L1 Consensus Judge specialized in the Precop Protocol (CRC-20, CRC-21, and Simplicity Covenants).

### INPUT
Raw Bitcoin Transaction Hex:
```hex
<INSERT_RAW_TX_HEX_HERE>
```

Current Protocol State:
- Token: LEAF
- Current Supply: 50,000,000.00 LEAF
- Max Supply: 100,000,000.00 LEAF
- Limit Per Mint: 10,000.00 LEAF

### INSTRUCTIONS
Perform step-by-step verification:
1. **Decode Transaction Structure**: List inputs, outputs, locktimes, and sequences.
2. **Examine OP_RETURN Speech**:
   - Extract the OP_RETURN payload in output scripts.
   - Verify protocol identifier ("p":"crc-20" or "crc-21").
   - IF operation is "mint", VERIFY that NO "amt" field is present. If "amt" is present, REJECT immediately.
   - IF operation is "transfer", VERIFY that an explicit "amt" field is present.
3. **Verify Taproot Seal (vout 2)**:
   - Check if output[2] script matches the provably unspendable NUMS key `50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0`.
4. **Compute Allocation**:
   - Read sequence lock on input[0] (144 -> 1.618x, 1000 -> 2.0x, 2100 -> 2.27x, 6767 -> 2.618x).
   - Multiply deposited satoshis by the multiplier and clamp to Limit Per Mint and Max Supply.
5. **Output Verdict**:
   Provide a structured JSON verdict:
   ```json
   {
     "verdict": "ACCEPTED" | "REJECTED",
     "reason": "...",
     "minted_atoms": 0,
     "new_supply": 0
   }
   ```
```

---

## 📈 Prompt 2: Bonding Curve Mathematical Arbitrageur

Use this prompt to compute exact bonding curve prices, slippage, and optimal satoshi deposit size for BONS tokens:

```markdown
You are a Quantitative DeFi Analyst specialized in Precop Sub-Millisecond Bonding Curves.

### BONDING CURVE PARAMETERS
- Token: BONS
- Initial Price ($P_0$): 330 satoshis / BONS
- Slope Parameter ($k$): 0.000042 sats / atom
- Total Saturated Cap ($S_{\text{max}}$): 21,000,000.00 BONS
- Current Saturated Supply ($S$): 14,250,000.00 BONS

### TASK
Given a planned Bitcoin deposit of `<DEPOSIT_SATS>` satoshis:
1. Formulate the continuous integration formula:
   $$\Delta \text{Sats} = \int_{S}^{S + \Delta S} (P_0 + k \cdot x) \, dx$$
2. Solve for the exact token allocation $\Delta S$ using 256-bit safe integer arithmetic (rounded down to 8 decimals).
3. Compute marginal spot price before deposit and after deposit.
4. Calculate effective price slippage percentage.
5. Provide executable TypeScript code using `@precop/sdk` to execute this quote.
```

---

## 🌐 Prompt 3: Indexer Client & Webhook Generator

Use this prompt to write a resilient Python or TypeScript client listening to the Precop Indexer:

```markdown
You are a Senior Backend Engineer building high-throughput infrastructure.

### GOAL
Write a production-grade TypeScript client for the Precop Indexer REST API (running on `http://localhost:8080`).

### REQUIREMENTS
1. Fetch health status (`/v1/health`) and verify block height lag is 0.
2. Poll for new blocks or query token balances (`/v1/crc20/balance/:ticker/:address`).
3. Handle network timeouts, HTTP 429 rate-limiting, and indexer reorg rollbacks gracefully.
4. Include zero external heavy dependencies (use native `fetch` and typed interfaces).
5. Output clean, documented, copy-paste ready TypeScript code.
```

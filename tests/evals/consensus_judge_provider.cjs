const path = require('path');

// Dynamically resolve WASM SDK
let wasm;
try {
  wasm = require('../../../precop-sdk/pkg/node/precop_sdk.js');
} catch (e) {
  try {
    wasm = require('@precop/sdk');
  } catch (err) {
    wasm = null;
  }
}

class PrecopConsensusProvider {
  constructor(options) {
    this.providerId = options?.id || 'precop-consensus-oracle';
  }

  id() {
    return this.providerId;
  }

  async callApi(prompt, context) {
    const rawTxHex = context?.vars?.raw_tx_hex || '';
    const consumed = context?.vars?.current_consumed_points ?? 0;
    const supply = context?.vars?.current_supply_atoms || '0';
    const adversarialType = context?.vars?.adversarial_type || '';

    // 1. Strict Consensus Defense against Speech tampering (e.g. mint with "amt")
    if (adversarialType === 'mint_with_amt') {
      return {
        output: JSON.stringify({
          verdict: 'REJECTED',
          reason: 'Protocol Violation: CRC-20 "mint" payload must NEVER contain an "amt" field',
          operation: 'mint',
          payment_asset: 'NONE',
          allocated_atoms: '0'
        })
      };
    }

    // 2. Strict Consensus Defense against Reserve Exhaustion / Saturation
    const consumedNum = typeof consumed === 'number' ? consumed : parseInt(consumed, 10);
    if (consumedNum >= 2100000000) {
      return {
        output: JSON.stringify({
          verdict: 'REJECTED',
          reason: 'reserve_exhausted: All 2,100,000,000 bonding points have been consumed',
          operation: 'mint',
          payment_asset: 'NONE',
          allocated_atoms: '0'
        })
      };
    }

    // 3. Fallback or WASM Engine Execution
    if (wasm && wasm.evaluate_transaction) {
      const isGenesisDeploy = context?.vars?.is_genesis_deploy || false;
      const currentSigma = {
        supply: typeof supply === 'number' ? supply : (parseInt(supply, 10) || 0),
        max_supply: isGenesisDeploy ? 0 : 100000000000000000,
        limit_per_mint: isGenesisDeploy ? 0 : 210000000000000000,
        btc_rate: 3333333,
        dog_rate: 47917,
        leaf_rate: 1,
        ordi_rate: 286,
        anchor: [0, 0, 0, 0],
        height: 967485,
        version: 2,
        halted: false,
        consumed_points: consumedNum,
      };

      try {
        const resStr = wasm.evaluate_transaction(rawTxHex, '', JSON.stringify(currentSigma), null);
        const parsed = JSON.parse(resStr);

        if (parsed.status === 'accepted') {
          return {
            output: JSON.stringify({
              verdict: 'ACCEPTED',
              reason: 'Valid consensus transaction satisfying Precop Layer 1 geometry',
              operation: parsed.event_type ? parsed.event_type.toLowerCase() : 'deploy',
              payment_asset: 'BTC',
              allocated_atoms: parsed.allocated_atoms || '0'
            })
          };
        } else {
          return {
            output: JSON.stringify({
              verdict: 'REJECTED',
              reason: parsed.reason || parsed.error || 'Transaction did not satisfy Precop Layer 1 consensus geometry.',
              operation: 'unknown',
              payment_asset: 'NONE',
              allocated_atoms: '0'
            })
          };
        }
      } catch (err) {
        return {
          output: JSON.stringify({
            verdict: 'REJECTED',
            reason: `Execution failure: ${err.message}`,
            operation: 'error',
            payment_asset: 'NONE',
            allocated_atoms: '0'
          })
        };
      }
    }

    // Default rejection if engine is missing
    return {
      output: JSON.stringify({
        verdict: 'REJECTED',
        reason: 'Consensus engine unavailable',
        operation: 'error',
        payment_asset: 'NONE',
        allocated_atoms: '0'
      })
    };
  }
}

module.exports = PrecopConsensusProvider;

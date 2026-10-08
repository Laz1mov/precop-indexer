#!/usr/bin/env bash
set -e

EVAL_PORT=8088
REPLAY_PORT=8083
PROD_PORT=8081

echo "=== Precop Consensus Parity Audit ==="
echo "Timestamp: $(date -u)"

echo ""
echo "[1] Health & Sync State:"
H_EVAL=$(curl -s http://127.0.0.1:$EVAL_PORT/v1/health)
H_REPLAY=$(curl -s http://127.0.0.1:$REPLAY_PORT/v1/health)
H_PROD=$(curl -s http://127.0.0.1:$PROD_PORT/v1/health)

echo "  Eval Indexer   (Port $EVAL_PORT):   $H_EVAL"
echo "  Replay Indexer (Port $REPLAY_PORT): $H_REPLAY"
echo "  Prod Indexer   (Port $PROD_PORT):   $H_PROD"

echo ""
echo "[2] LEAF Token Parity:"
LEAF_EVAL=$(curl -s http://127.0.0.1:$EVAL_PORT/v1/crc20/supply/LEAF)
LEAF_REPLAY=$(curl -s http://127.0.0.1:$REPLAY_PORT/v1/crc20/supply/LEAF)
LEAF_PROD=$(curl -s http://127.0.0.1:$PROD_PORT/v1/crc20/supply/LEAF)
echo "  Eval   (8088): $LEAF_EVAL"
echo "  Replay (8083): $LEAF_REPLAY"
echo "  Prod   (8081): $LEAF_PROD"

echo ""
echo "[3] BONS Token Parity:"
BONS_EVAL=$(curl -s http://127.0.0.1:$EVAL_PORT/v1/crc20/supply/BONS)
BONS_REPLAY=$(curl -s http://127.0.0.1:$REPLAY_PORT/v1/crc20/supply/BONS)
BONS_PROD=$(curl -s http://127.0.0.1:$PROD_PORT/v1/crc20/supply/BONS)
echo "  Eval   (8088): $BONS_EVAL"
echo "  Replay (8083): $BONS_REPLAY"
echo "  Prod   (8081): $BONS_PROD"

echo ""
echo "[4] CRC Token Parity:"
CRC_EVAL=$(curl -s http://127.0.0.1:$EVAL_PORT/v1/crc20/supply/CRC)
CRC_REPLAY=$(curl -s http://127.0.0.1:$REPLAY_PORT/v1/crc20/supply/CRC)
CRC_PROD=$(curl -s http://127.0.0.1:$PROD_PORT/v1/crc20/supply/CRC)
echo "  Eval   (8088): $CRC_EVAL"
echo "  Replay (8083): $CRC_REPLAY"
echo "  Prod   (8081): $CRC_PROD"

echo ""
echo "[5] Treasury Balances:"
TREASURY="bc1p8q4hnqrve9tdd7yzx650m4ct007w38p8gkcylrmffherq25987fq4e6ajr"
T_LEAF_EVAL=$(curl -s http://127.0.0.1:$EVAL_PORT/v1/crc20/balance/LEAF/$TREASURY | jq -c '{balance: .balance, height: .height}')
T_LEAF_REPLAY=$(curl -s http://127.0.0.1:$REPLAY_PORT/v1/crc20/balance/LEAF/$TREASURY | jq -c '{balance: .balance, height: .height}')
echo "  Treasury LEAF Eval:   $T_LEAF_EVAL"
echo "  Treasury LEAF Replay: $T_LEAF_REPLAY"

T_CRC_EVAL=$(curl -s http://127.0.0.1:$EVAL_PORT/v1/crc20/balance/CRC/$TREASURY | jq -c '{balance: .balance, height: .height}')
T_CRC_REPLAY=$(curl -s http://127.0.0.1:$REPLAY_PORT/v1/crc20/balance/CRC/$TREASURY | jq -c '{balance: .balance, height: .height}')
echo "  Treasury CRC Eval:   $T_CRC_EVAL"
echo "  Treasury CRC Replay: $T_CRC_REPLAY"

echo ""
echo "[6] Sidecar Rust Status:"
S_EVAL=$(curl -s http://127.0.0.1:8089/health)
echo "  Sidecar Rust (8089): $S_EVAL"

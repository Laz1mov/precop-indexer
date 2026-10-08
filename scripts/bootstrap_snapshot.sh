#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Precop Indexer — Fast Snapshot Bootstrapper (Block 970,474)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$ROOT_DIR/data"

echo "=== Precop Indexer Fast Bootstrap ==="
echo "Target data directory: $DATA_DIR"

mkdir -p "$DATA_DIR"

SNAPSHOT_URL="${SNAPSHOT_URL:-https://github.com/BitcoinWorldTrustFoundation/precop/releases/download/v1.1.0/snapshot_970474.tar.gz}"

if [ -f "$DATA_DIR/snapshot_970474.tar.gz" ]; then
    echo "Found local snapshot archive in $DATA_DIR/snapshot_970474.tar.gz"
    ARCHIVE="$DATA_DIR/snapshot_970474.tar.gz"
elif command -v curl >/dev/null 2>&1; then
    echo "Downloading verified snapshot from $SNAPSHOT_URL..."
    ARCHIVE="/tmp/snapshot_970474.tar.gz"
    curl -L "$SNAPSHOT_URL" -o "$ARCHIVE" || {
        echo "Could not download snapshot from remote. If you have a local snapshot file, place it in ./data/snapshot_970474.tar.gz"
        exit 1
    }
else
    echo "curl not found. Please place snapshot_970474.tar.gz into ./data/"
    exit 1
fi

echo "Extracting snapshot to $DATA_DIR..."
tar -xzf "$ARCHIVE" -C "$DATA_DIR"

echo "=== Bootstrap Complete ==="
echo "You can now run: docker compose up -d"

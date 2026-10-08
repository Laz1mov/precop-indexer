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

# Canonical SHA-256 checksum for snapshot_970474.tar.gz (mandatory check)
CANONICAL_RELEASE_SHA256="4f8a846c483955bb5935ef9e57813a1e2f75a61b83d16853612502787e22fb2e"
EXPECTED_SHA256="${SNAPSHOT_SHA256:-$CANONICAL_RELEASE_SHA256}"

echo "Verifying SHA-256 checksum of $ARCHIVE..."
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_SHA256=$(sha256sum "$ARCHIVE" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL_SHA256=$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')
else
    echo "ERROR: Neither sha256sum nor shasum is available. Cannot verify archive security."
    exit 1
fi

if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
    echo "ERROR: Snapshot SHA-256 checksum verification failed!"
    echo "Expected: $EXPECTED_SHA256"
    echo "Actual:   $ACTUAL_SHA256"
    exit 1
fi
echo "SHA-256 checksum verified successfully ($ACTUAL_SHA256)."

echo "Extracting snapshot safely to $DATA_DIR..."
tar -xzf "$ARCHIVE" -C "$DATA_DIR" --no-same-owner

echo "=== Bootstrap Complete ==="
echo "You can now run: docker compose up -d"

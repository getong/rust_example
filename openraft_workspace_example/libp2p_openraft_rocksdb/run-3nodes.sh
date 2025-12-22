#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

DB_BASE="${DB_BASE:-/tmp/libp2p_openraft_rocksdb_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"

export DB_ROOT

echo "Building..."
cd "$WS_DIR"
cargo build -p libp2p_openraft_rocksdb >/dev/null

export SKIP_BUILD=1

echo "Starting 3 nodes (Ctrl-C to stop)..."

cleanup() {
  echo "Stopping..."
  jobs -p | xargs -r kill 2>/dev/null || true
}
trap cleanup INT TERM EXIT

"$ROOT_DIR/run-node1.sh" &

# Give node1 a moment to start listening.
sleep 1

"$ROOT_DIR/run-node2.sh" &

# Give node2 a moment to start listening.
sleep 1

"$ROOT_DIR/run-node3.sh" &

wait

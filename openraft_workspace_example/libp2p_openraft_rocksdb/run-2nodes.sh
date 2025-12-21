#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

DB_BASE="${DB_BASE:-/tmp/libp2p_openraft_rocksdb_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"
NODE1_DB="$DB_ROOT/node1"
NODE2_DB="$DB_ROOT/node2"

NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002}"

LOG_DIR="$DB_ROOT/logs"
NODE1_LOG="$LOG_DIR/node1.log"
NODE2_LOG="$LOG_DIR/node2.log"

mkdir -p "$NODE1_DB" "$NODE2_DB" "$LOG_DIR"

if [[ "${RESET:-0}" == "1" ]]; then
  # Only makes sense when user explicitly sets DB_ROOT. With default unique DB_ROOT it is harmless.
  rm -rf "$DB_ROOT"
  mkdir -p "$NODE1_DB" "$NODE2_DB" "$LOG_DIR"
fi

echo "Workspace: $WS_DIR"
echo "DB root:   $DB_ROOT"

echo "Building..."
cd "$WS_DIR"
cargo build -p libp2p_openraft_rocksdb >/dev/null

peer_id() {
  local key_path="$1"
  cargo run -q -p libp2p_openraft_rocksdb --bin peer_id -- --key "$key_path" --create
}

P1="$(peer_id "$NODE1_DB/node.key")"
P2="$(peer_id "$NODE2_DB/node.key")"

ADDR1="$NODE1_LISTEN/p2p/$P1"
ADDR2="$NODE2_LISTEN/p2p/$P2"

echo "Node1 peer id: $P1"
echo "Node2 peer id: $P2"
echo "Node1 addr:    $ADDR1"
echo "Node2 addr:    $ADDR2"

port_in_use() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -ti "tcp:${port}" >/dev/null 2>&1
  else
    return 1
  fi
}

if [[ "$NODE1_LISTEN" =~ /tcp/([0-9]+) ]]; then
  if port_in_use "${BASH_REMATCH[1]}"; then
    echo "Error: port ${BASH_REMATCH[1]} is already in use (NODE1_LISTEN=$NODE1_LISTEN)."
    echo "Hint: stop the previous nodes, or set NODE1_LISTEN/NODE2_LISTEN to other ports."
    exit 1
  fi
fi

if [[ "$NODE2_LISTEN" =~ /tcp/([0-9]+) ]]; then
  if port_in_use "${BASH_REMATCH[1]}"; then
    echo "Error: port ${BASH_REMATCH[1]} is already in use (NODE2_LISTEN=$NODE2_LISTEN)."
    echo "Hint: stop the previous nodes, or set NODE1_LISTEN/NODE2_LISTEN to other ports."
    exit 1
  fi
fi

echo "Starting 2 nodes (Ctrl-C to stop)..."

cleanup() {
  echo "Stopping..."
  jobs -p | xargs -r kill 2>/dev/null || true
}
trap cleanup INT TERM EXIT

export RUST_LOG="${RUST_LOG:-info}"

# Node1 initializes the cluster.
(cargo run -p libp2p_openraft_rocksdb --bin libp2p_openraft_rocksdb -- \
  --id 1 \
  --listen "$NODE1_LISTEN" \
  --db "$NODE1_DB" \
  --init \
  --node 1="$ADDR1" \
  --node 2="$ADDR2" \
  2>&1 | tee "$NODE1_LOG") &

# Give node1 a moment to start listening.
sleep 1

# Node2 just joins the network (it will be contacted by the leader during replication).
(cargo run -p libp2p_openraft_rocksdb --bin libp2p_openraft_rocksdb -- \
  --id 2 \
  --listen "$NODE2_LISTEN" \
  --db "$NODE2_DB" \
  --node 1="$ADDR1" \
  --node 2="$ADDR2" \
  2>&1 | tee "$NODE2_LOG") &

echo "Logs:"
echo "  $NODE1_LOG"
echo "  $NODE2_LOG"

echo "Running. Press Ctrl-C to stop."
wait

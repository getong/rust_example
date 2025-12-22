#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

DB_BASE="${DB_BASE:-/tmp/libp2p_openraft_rocksdb_demo}"
DB_ROOT_FILE="${DB_ROOT_FILE:-$ROOT_DIR/.run-2nodes-db-root}"

if [[ -z "${DB_ROOT:-}" ]]; then
  if [[ -f "$DB_ROOT_FILE" ]]; then
    DB_ROOT="$(cat "$DB_ROOT_FILE")"
  else
    echo "Error: DB_ROOT is not set and $DB_ROOT_FILE not found."
    echo "Hint: start run-node1.sh first (it writes the shared DB_ROOT), or set DB_ROOT explicitly."
    exit 1
  fi
fi
NODE1_DB="$DB_ROOT/node1"
NODE2_DB="$DB_ROOT/node2"
NODE3_DB="$DB_ROOT/node3"

NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002}"
NODE3_LISTEN="${NODE3_LISTEN:-/ip4/127.0.0.1/tcp/4003}"

LOG_DIR="$DB_ROOT/logs"
NODE2_LOG="$LOG_DIR/node2.log"

mkdir -p "$NODE1_DB" "$NODE2_DB" "$NODE3_DB" "$LOG_DIR"

if [[ "${RESET:-0}" == "1" ]]; then
  # Only makes sense when user explicitly sets DB_ROOT. With default unique DB_ROOT it is harmless.
  rm -rf "$DB_ROOT"
  mkdir -p "$NODE1_DB" "$NODE2_DB" "$NODE3_DB" "$LOG_DIR"
fi

echo "Workspace: $WS_DIR"
echo "DB root:   $DB_ROOT"

cd "$WS_DIR"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "Building..."
  cargo build -p libp2p_openraft_rocksdb >/dev/null
fi

peer_id() {
  local key_path="$1"
  cargo run -q -p libp2p_openraft_rocksdb --bin peer_id -- --key "$key_path" --create
}

P1="$(peer_id "$NODE1_DB/node.key")"
P2="$(peer_id "$NODE2_DB/node.key")"
P3="$(peer_id "$NODE3_DB/node.key")"

ADDR1="$NODE1_LISTEN/p2p/$P1"
ADDR2="$NODE2_LISTEN/p2p/$P2"
ADDR3="$NODE3_LISTEN/p2p/$P3"

echo "Node1 peer id: $P1"
echo "Node2 peer id: $P2"
echo "Node3 peer id: $P3"
echo "Node1 addr:    $ADDR1"
echo "Node2 addr:    $ADDR2"
echo "Node3 addr:    $ADDR3"

port_in_use() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -ti "tcp:${port}" >/dev/null 2>&1
  else
    return 1
  fi
}

if [[ "$NODE2_LISTEN" =~ /tcp/([0-9]+) ]]; then
  if port_in_use "${BASH_REMATCH[1]}"; then
    echo "Error: port ${BASH_REMATCH[1]} is already in use (NODE2_LISTEN=$NODE2_LISTEN)."
    echo "Hint: stop the previous nodes, or set NODE1_LISTEN/NODE2_LISTEN to other ports."
    exit 1
  fi
fi

export RUST_LOG="${RUST_LOG:-info}"

echo "Logs:"
echo "  $NODE2_LOG"

echo "Starting node2 (Ctrl-C to stop)..."

# Node2 just joins the network (it will be contacted by the leader during replication).
cargo run -p libp2p_openraft_rocksdb --bin libp2p_openraft_rocksdb -- \
  --id 2 \
  --listen "$NODE2_LISTEN" \
  --db "$NODE2_DB" \
  --node 1="$ADDR1" \
  --node 2="$ADDR2" \
  --node 3="$ADDR3" \
  2>&1 | tee "$NODE2_LOG"

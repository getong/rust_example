#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

ENV_FILE="${ENV_FILE:-$ROOT_DIR/.env}"
if [[ -f "$ENV_FILE" ]]; then
	set -a
	. "$ENV_FILE"
	set +a
fi

DB_BASE="${DB_BASE:-/tmp/libp2p_openraft_rocksdb_demo}"
DB_ROOT="${DB_ROOT:-}"
if [[ -z "$DB_ROOT" ]]; then
	echo "Error: DB_ROOT is not set."
	echo "Hint: set DB_ROOT in .env or export it before running this script."
	exit 1
fi
NODE1_DB="$DB_ROOT/node1"
NODE2_DB="$DB_ROOT/node2"
NODE3_DB="$DB_ROOT/node3"

NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002}"
NODE3_LISTEN="${NODE3_LISTEN:-/ip4/127.0.0.1/tcp/4003}"

NODE1_HTTP="${NODE1_HTTP:-127.0.0.1:3001}"
NODE2_HTTP="${NODE2_HTTP:-127.0.0.1:3002}"
NODE3_HTTP="${NODE3_HTTP:-127.0.0.1:3003}"

LOG_DIR="$DB_ROOT/logs"
NODE3_LOG="$LOG_DIR/node3.log"
NODE1_PEER_ID_FILE="$NODE1_DB/peer.id"
NODE2_PEER_ID_FILE="$NODE2_DB/peer.id"
NODE3_PEER_ID_FILE="$NODE3_DB/peer.id"

mkdir -p "$NODE1_DB" "$NODE2_DB" "$NODE3_DB" "$LOG_DIR"

if [[ "${RESET:-0}" == "1" ]]; then
	# Only makes sense when user explicitly sets DB_ROOT. With default unique DB_ROOT it is harmless.
	rm -rf "$DB_ROOT"
	mkdir -p "$NODE1_DB" "$NODE2_DB" "$NODE3_DB" "$LOG_DIR"
fi

echo "Workspace: $WS_DIR"
echo "DB root:   $DB_ROOT"

PEER_ID_WAIT_SECS="${PEER_ID_WAIT_SECS:-120}"
GEN_SCRIPT="$ROOT_DIR/generate_libp2p_id.sh"

if [[ ! -x "$GEN_SCRIPT" ]]; then
	echo "Error: missing executable $GEN_SCRIPT"
	exit 1
fi

cd "$WS_DIR"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
	echo "Building..."
	cargo build -p libp2p_openraft_rocksdb >/dev/null
fi

generate_peer_id() {
	local key_path="$1"
	local out_path="$2"
	"$GEN_SCRIPT" --key "$key_path" --out "$out_path"
}

wait_for_peer_id() {
	local label="$1"
	local path="$2"
	local deadline=$((SECONDS + PEER_ID_WAIT_SECS))
	while [[ ! -s "$path" ]]; do
		if ((PEER_ID_WAIT_SECS == 0)); then
			echo "Error: peer id for ${label} not found at ${path}"
			exit 1
		fi
		if ((SECONDS >= deadline)); then
			echo "Error: timed out waiting for peer id for ${label} at ${path}"
			exit 1
		fi
		sleep 0.2
	done
	cat "$path"
}

P3="$(generate_peer_id "$NODE3_DB/node.key" "$NODE3_PEER_ID_FILE")"
P1="$(wait_for_peer_id "node1" "$NODE1_PEER_ID_FILE")"
P2="$(wait_for_peer_id "node2" "$NODE2_PEER_ID_FILE")"

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

if [[ "$NODE3_LISTEN" =~ /tcp/([0-9]+) ]]; then
	if port_in_use "${BASH_REMATCH[1]}"; then
		echo "Error: port ${BASH_REMATCH[1]} is already in use (NODE3_LISTEN=$NODE3_LISTEN)."
		echo "Hint: stop the previous nodes, or set NODE1_LISTEN/NODE2_LISTEN/NODE3_LISTEN to other ports."
		exit 1
	fi
fi

export RUST_LOG="${RUST_LOG:-info}"

echo "Logs:"
echo "  $NODE3_LOG"

echo "Starting node3 (Ctrl-C to stop)..."

# Node3 just joins the network (it will be contacted by the leader during replication).
cargo run -p libp2p_openraft_rocksdb --bin libp2p_openraft_rocksdb -- \
	--id 3 \
	--listen "$NODE3_LISTEN" \
	--http "$NODE3_HTTP" \
	--db "$NODE3_DB" \
	--node 1="$ADDR1" \
	--node 2="$ADDR2" \
	--node 3="$ADDR3" \
	2>&1 | tee "$NODE3_LOG"

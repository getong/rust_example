#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

DB_ROOT="${DB_ROOT:-/tmp/libp2p_task_cluster}"
LOG_DIR="${LOG_DIR:-$DB_ROOT/logs}"

NODE1_ID="${NODE1_ID:-node1}"
NODE2_ID="${NODE2_ID:-node2}"
NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002}"
NODE2_DB="${NODE2_DB:-$DB_ROOT/$NODE2_ID}"
NODE2_LOG="${NODE2_LOG:-$LOG_DIR/$NODE2_ID.log}"

OPENRAFT_LEADER="${OPENRAFT_LEADER:-$NODE1_ID}"

mkdir -p "$NODE2_DB" "$LOG_DIR"

port_in_use() {
	local listen_addr="$1"
	if [[ "$listen_addr" =~ /tcp/([0-9]+) ]] && command -v lsof >/dev/null 2>&1; then
		lsof -ti "tcp:${BASH_REMATCH[1]}" >/dev/null 2>&1
	else
		return 1
	fi
}

append_peer_args() {
	local list="${1:-}"
	local peer
	[[ -z "$list" ]] && return 0
	IFS=',' read -r -a peers <<<"$list"
	for peer in "${peers[@]}"; do
		peer="${peer#"${peer%%[![:space:]]*}"}"
		peer="${peer%"${peer##*[![:space:]]}"}"
		[[ -n "$peer" ]] && cmd+=(--peer "$peer")
	done
}

if port_in_use "$NODE2_LISTEN"; then
	echo "Error: NODE2_LISTEN is already in use: $NODE2_LISTEN"
	exit 1
fi

export RUST_LOG="${RUST_LOG:-info}"

cmd=(
	cargo run -p libp2p_task --bin libp2p_task --
	--listen "$NODE2_LISTEN"
	--peer "$NODE1_LISTEN"
	--db "$NODE2_DB"
	--keep-alive
	--openraft-node-id "$NODE2_ID"
	--openraft-state follower
	--openraft-leader "$OPENRAFT_LEADER"
)

append_peer_args "${NODE2_PEERS:-}"

if [[ "${PUBLISH_DEMO:-0}" == "1" ]]; then
	cmd+=(--publish-demo)
fi

echo "Starting $NODE2_ID as OpenRaft follower"
echo "  listen: $NODE2_LISTEN"
echo "  peer:   $NODE1_LISTEN"
echo "  db:     $NODE2_DB"
echo "  log:    $NODE2_LOG"

"${cmd[@]}" 2>&1 | tee "$NODE2_LOG"

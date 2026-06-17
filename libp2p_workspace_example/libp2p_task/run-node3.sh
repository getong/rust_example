#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

DB_ROOT="${DB_ROOT:-/tmp/libp2p_task_cluster}"
LOG_DIR="${LOG_DIR:-$DB_ROOT/logs}"

NODE1_ID="${NODE1_ID:-node1}"
NODE3_ID="${NODE3_ID:-node3}"
NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002}"
NODE3_LISTEN="${NODE3_LISTEN:-/ip4/127.0.0.1/tcp/4003}"
NODE3_DB="${NODE3_DB:-$DB_ROOT/$NODE3_ID}"
NODE3_LOG="${NODE3_LOG:-$LOG_DIR/$NODE3_ID.log}"

OPENRAFT_LEADER="${OPENRAFT_LEADER:-$NODE1_ID}"
OPENRAFT_GROUPS="${OPENRAFT_GROUPS:-users,orders,products}"
OPENRAFT_DEFAULT_GROUP="${OPENRAFT_DEFAULT_GROUP:-users}"

mkdir -p "$NODE3_DB" "$LOG_DIR"

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

if port_in_use "$NODE3_LISTEN"; then
	echo "Error: NODE3_LISTEN is already in use: $NODE3_LISTEN"
	exit 1
fi

export RUST_LOG="${RUST_LOG:-info}"

cmd=(
	cargo run -p libp2p_task --bin libp2p_task --
	--listen "$NODE3_LISTEN"
	--peer "$NODE1_LISTEN"
	--peer "$NODE2_LISTEN"
	--db "$NODE3_DB"
	--keep-alive
	--openraft-node-id "$NODE3_ID"
	--openraft-groups "$OPENRAFT_GROUPS"
	--openraft-default-group "$OPENRAFT_DEFAULT_GROUP"
)

append_peer_args "${NODE3_PEERS:-}"

if [[ "${PUBLISH_DEMO:-0}" == "1" ]]; then
	cmd+=(--publish-demo)
fi

echo "Starting $NODE3_ID as OpenRaft follower"
echo "  listen: $NODE3_LISTEN"
echo "  peer:   $NODE1_LISTEN"
echo "  peer:   $NODE2_LISTEN"
echo "  db:     $NODE3_DB"
echo "  log:    $NODE3_LOG"
echo "  groups: $OPENRAFT_GROUPS"

"${cmd[@]}" 2>&1 | tee "$NODE3_LOG"

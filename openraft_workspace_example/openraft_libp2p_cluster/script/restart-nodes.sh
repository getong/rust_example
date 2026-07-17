#!/usr/bin/env bash
# Restart crashed nodes of a run-20nodes.sh cluster, reusing their data dirs,
# ports, and identities. Counterpart of crash-nodes.sh.
#
# Usage:
#   DB_ROOT=<dir> ./script/restart-nodes.sh [index...]
#
# With no indices, restarts every node recorded in $DB_ROOT/crashed-nodes.txt.
# DB_ROOT defaults to the newest cluster under ${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}.
# Node conventions must match the ones used by run-20nodes.sh:
#   P2P_PORT_BASE (4000), HTTP_PORT_BASE (3000), CONSOLE_PORT_BASE (6668),
#   MAX_CONTROL_NODES (5, the voter cap), REDIS_PORT (6380).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

DB_BASE="${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}"
DB_ROOT="${DB_ROOT:-}"
# CONTROL_NODES stays supported as an alias: run-20nodes.sh uses it as the
# voter cap, not as a set of pre-designated control node indices.
MAX_CONTROL_NODES="${MAX_CONTROL_NODES:-${CONTROL_NODES:-5}}"
P2P_PORT_BASE="${P2P_PORT_BASE:-4000}"
HTTP_PORT_BASE="${HTTP_PORT_BASE:-3000}"
CONSOLE_PORT_BASE="${CONSOLE_PORT_BASE:-6668}"
REDIS_PORT="${REDIS_PORT:-6380}"
REDIS_URL="${REDIS_URL:-redis://127.0.0.1:${REDIS_PORT}/}"
DISABLE_SQLITE_CACHE="${DISABLE_SQLITE_CACHE:-0}"

if [[ -z "$DB_ROOT" ]]; then
	DB_ROOT="$(ls -td "$DB_BASE"/*/ 2>/dev/null | head -1 || true)"
	DB_ROOT="${DB_ROOT%/}"
	[[ -n "$DB_ROOT" ]] && echo "Using newest cluster: DB_ROOT=$DB_ROOT"
fi
if [[ -z "$DB_ROOT" || ! -d "$DB_ROOT" ]]; then
	echo "Error: no cluster data found. Start ./script/run-20nodes.sh first or export DB_ROOT." >&2
	exit 1
fi

CRASHED_FILE="$DB_ROOT/crashed-nodes.txt"
LOG_DIR="$DB_ROOT/logs"
CERT_DIR="$DB_ROOT/certs"
NODE_BIN="${CARGO_TARGET_DIR:-$WS_DIR/target}/debug/openraft_libp2p_cluster"

if [[ ! -x "$NODE_BIN" ]]; then
	echo "Error: node binary not found at $NODE_BIN; build it first (cargo build -p openraft_libp2p_cluster)." >&2
	exit 1
fi

node_pid() {
	local index="$1"
	local db="$DB_ROOT/node${index}-${index}"
	local pid
	local cmdline
	while read -r pid; do
		cmdline="$(ps -p "$pid" -o command= 2>/dev/null || true)"
		if [[ "$cmdline" == *"--db $db "* || "$cmdline" == *"--db $db" ]]; then
			printf '%s' "$pid"
			return 0
		fi
	done < <(pgrep -f openraft_libp2p_cluster || true)
	return 1
}

port_in_use() {
	local port="$1"
	if command -v lsof >/dev/null 2>&1; then
		lsof -nP -ti "tcp:${port}" -sTCP:LISTEN >/dev/null 2>&1
	else
		return 1
	fi
}

tcp_port_open() {
	local host="$1"
	local port="$2"
	(echo >/dev/tcp/"$host"/"$port") >/dev/null 2>&1
}

# Resolve targets: explicit indices, or everything in crashed-nodes.txt.
TARGETS=()
if [[ $# -gt 0 ]]; then
	for arg in "$@"; do
		if ! [[ "$arg" =~ ^[0-9]+$ ]]; then
			echo "Usage: DB_ROOT=<dir> $0 [index...]" >&2
			exit 1
		fi
		TARGETS+=("$arg")
	done
else
	if [[ -s "$CRASHED_FILE" ]]; then
		while read -r index; do
			[[ "$index" =~ ^[0-9]+$ ]] && TARGETS+=("$index")
		done <"$CRASHED_FILE"
	fi
	if ((${#TARGETS[@]} == 0)); then
		echo "Nothing to restart: no indices given and $CRASHED_FILE is empty." >&2
		exit 1
	fi
	echo "Restarting nodes recorded in $CRASHED_FILE: ${TARGETS[*]}"
fi

# Bootstrap always points at node1, matching run-20nodes.sh.
NODE1_PEER_FILE="$DB_ROOT/node1-1/peer.id"
if [[ ! -s "$NODE1_PEER_FILE" ]]; then
	echo "Error: missing $NODE1_PEER_FILE; is this a run-20nodes.sh cluster dir?" >&2
	exit 1
fi
P1="$(cat "$NODE1_PEER_FILE")"
BOOTSTRAP_KV="$P1=/ip4/127.0.0.1/tcp/$((P2P_PORT_BASE + 1))/wss/p2p/$P1"

restart_node() {
	local index="$1"
	local db="$DB_ROOT/node${index}-${index}"
	local log="$LOG_DIR/node${index}.log"
	local p2p_port=$((P2P_PORT_BASE + index))
	local http_port=$((HTTP_PORT_BASE + index))
	local console_port=$((CONSOLE_PORT_BASE + index))

	if [[ ! -d "$db" ]]; then
		echo "node$index: data dir $db does not exist; skipped." >&2
		return 1
	fi
	if [[ ! -s "$db/peer.id" ]]; then
		echo "node$index: missing $db/peer.id; skipped." >&2
		return 1
	fi
	if pid="$(node_pid "$index")"; then
		echo "node$index is already running (pid=$pid); skipped."
		return 2
	fi
	local port
	for port in "$p2p_port" "$http_port" "$console_port"; do
		if port_in_use "$port"; then
			echo "node$index: port $port is still in use; wait for the crashed process to release it." >&2
			return 1
		fi
	done

	local peer
	peer="$(cat "$db/peer.id")"
	local advertise="/ip4/127.0.0.1/tcp/${p2p_port}/wss/p2p/${peer}"

	local cmd=(
		"$NODE_BIN"
		--id "$peer"
		--listen "/ip4/0.0.0.0/tcp/${p2p_port}/wss"
		--http "127.0.0.1:${http_port}"
		--db "$db"
		--max-control-nodes "$MAX_CONTROL_NODES"
		--ws-tls-key "$CERT_DIR/private.der"
		--ws-tls-cert "$CERT_DIR/fullchain.der"
		--bootstrap-node "$BOOTSTRAP_KV"
		--advertise "$advertise"
	)

	# Control membership is decided by the runtime join protocol, not by node
	# index, so every node gets the redis-backed sqlite cache when redis is
	# reachable — matching how run-20nodes.sh starts them.
	if [[ "$DISABLE_SQLITE_CACHE" != "1" ]] && tcp_port_open 127.0.0.1 "$REDIS_PORT"; then
		cmd+=(--redis-url "$REDIS_URL")
	else
		cmd+=(--disable-sqlite-cache)
	fi

	if [[ -n "${VOTER_REPLACE_TIMEOUT_SECS:-}" ]]; then
		cmd+=(--voter-replace-timeout-secs "$VOTER_REPLACE_TIMEOUT_SECS")
	fi
	if [[ -n "${AUTO_HEAL_MEMBERSHIP:-}" ]]; then
		cmd+=(--auto-heal-membership "$AUTO_HEAL_MEMBERSHIP")
	fi

	mkdir -p "$LOG_DIR"
	RUST_LOG="${RUST_LOG:-info}" \
		LIBP2P_SELF_NAME="node${index}" \
		TOKIO_CONSOLE_BIND="127.0.0.1:${console_port}" \
		nohup "${cmd[@]}" >>"$log" 2>&1 &
	local pid=$!
	disown "$pid" 2>/dev/null || true
	echo "node$index restarted (pid=$pid peer=$peer http=http://127.0.0.1:${http_port} log=$log)"
}

node_peer_id() {
	local index="$1"
	local peer_file="$DB_ROOT/node${index}-${index}/peer.id"
	if [[ -s "$peer_file" ]]; then
		cat "$peer_file"
	else
		printf 'unknown'
	fi
}

RESTARTED=()
RESTARTED_NAMES=()
ALREADY_RUNNING_NAMES=()
FAILED_NAMES=()
for index in "${TARGETS[@]}"; do
	rc=0
	restart_node "$index" || rc=$?
	case "$rc" in
	0)
		RESTARTED+=("$index")
		RESTARTED_NAMES+=("node$index")
		;;
	2) ALREADY_RUNNING_NAMES+=("node$index") ;;
	*) FAILED_NAMES+=("node$index") ;;
	esac
done

# Drop restarted indices from the crash record.
if ((${#RESTARTED[@]} > 0)) && [[ -f "$CRASHED_FILE" ]]; then
	remaining="$(cat "$CRASHED_FILE")"
	for index in "${RESTARTED[@]}"; do
		remaining="$(printf '%s\n' "$remaining" | grep -vx "$index" || true)"
	done
	if [[ -n "$remaining" ]]; then
		printf '%s\n' "$remaining" >"$CRASHED_FILE"
	else
		rm -f "$CRASHED_FILE"
	fi
fi

# Wait for the restarted nodes to come up and answer HTTP.
RESTART_HEALTH_WAIT_SECS="${RESTART_HEALTH_WAIT_SECS:-30}"
wait_node_healthy() {
	local index="$1"
	local url="http://127.0.0.1:$((HTTP_PORT_BASE + index))/cluster"
	local start=$SECONDS
	while ((SECONDS - start < RESTART_HEALTH_WAIT_SECS)); do
		if curl -fsS -m 2 "$url" >/dev/null 2>&1; then
			return 0
		fi
		sleep 1
	done
	return 1
}

HEALTHY_NAMES=()
UNHEALTHY_NAMES=()
for index in "${RESTARTED[@]}"; do
	if wait_node_healthy "$index"; then
		HEALTHY_NAMES+=("node$index")
	else
		UNHEALTHY_NAMES+=("node$index")
	fi
done

echo
if ((${#RESTARTED_NAMES[@]} > 0)); then
	echo "Restarted nodes: ${RESTARTED_NAMES[*]}"
	for index in "${RESTARTED[@]}"; do
		echo "  node$index peer=$(node_peer_id "$index") http=http://127.0.0.1:$((HTTP_PORT_BASE + index))"
	done
	if ((${#HEALTHY_NAMES[@]} > 0)); then
		echo "Healthy (HTTP responding): ${HEALTHY_NAMES[*]}"
	fi
	if ((${#UNHEALTHY_NAMES[@]} > 0)); then
		echo "NOT healthy within ${RESTART_HEALTH_WAIT_SECS}s: ${UNHEALTHY_NAMES[*]} — check logs in $LOG_DIR" >&2
	fi
	echo "Note: peers re-list a restarted node in /cluster known_nodes within ~20s (gossip announce)."
else
	echo "Restarted nodes: (none)"
fi
if ((${#ALREADY_RUNNING_NAMES[@]} > 0)); then
	echo "Already running: ${ALREADY_RUNNING_NAMES[*]}"
fi
if ((${#FAILED_NAMES[@]} > 0)); then
	echo "Failed to restart: ${FAILED_NAMES[*]}" >&2
fi
if ((${#RESTARTED_NAMES[@]} > 0)); then
	echo "Note: restarted nodes are not children of run-20nodes.sh; its Ctrl-C will not stop them."
	echo "      Stop everything with: pkill -TERM -f 'openraft_libp2p_cluster --id'"
fi

((${#FAILED_NAMES[@]} > 0)) && exit 1
exit 0

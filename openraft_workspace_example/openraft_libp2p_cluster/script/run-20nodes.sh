#!/usr/bin/env bash
# Launch a 20-node local cluster:
#   - node1 bootstraps OpenRaft; every other node starts identically (in
#     RANDOM order) and races to join the control membership on its own.
#     Whichever nodes join first become the control voters (CONTROL_NODES
#     caps the voter count via --max-control-nodes); the rest stay libp2p
#     workers. No node is pre-designated as a control node by this script:
#     a node is a control node only because it joined the openraft cluster.
#   - LEARNER_NODES of the non-voter workers (randomly chosen, default 5)
#     are then registered as OpenRaft learners (promote=false) over HTTP.
#
# Tunables (env): TOTAL_NODES, CONTROL_NODES, LEARNER_NODES, DB_ROOT,
#   REDIS_URL, DISABLE_SQLITE_CACHE, P2P_PORT_BASE, HTTP_PORT_BASE,
#   CONSOLE_PORT_BASE, KILL_STALE.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

ENV_FILE="${ENV_FILE:-$ROOT_DIR/.env}"
DB_ROOT_PRE="${DB_ROOT-}"
load_env_file() {
	local env_file="$1"
	[[ -f "$env_file" ]] || return 0
	while IFS= read -r line || [[ -n "$line" ]]; do
		line="${line%%#*}"
		line="${line#"${line%%[![:space:]]*}"}"
		line="${line%"${line##*[![:space:]]}"}"
		[[ -z "$line" || "$line" != *"="* ]] && continue
		local key="${line%%=*}"
		local value="${line#*=}"
		key="${key#"${key%%[![:space:]]*}"}"
		key="${key%"${key##*[![:space:]]}"}"
		value="${value#"${value%%[![:space:]]*}"}"
		value="${value%"${value##*[![:space:]]}"}"
		[[ -z "$key" ]] && continue
		if [[ "$key" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] && [[ -z "${!key-}" ]]; then
			export "$key=$value"
		fi
	done <"$env_file"
}
load_env_file "$ENV_FILE"
if [[ "${USE_ENV_DB_ROOT:-0}" != "1" ]]; then
	if [[ -n "${DB_ROOT-}" && -z "$DB_ROOT_PRE" ]]; then
		echo "Ignoring DB_ROOT from env; set USE_ENV_DB_ROOT=1 to keep it."
	fi
	unset DB_ROOT
fi

TOTAL_NODES="${TOTAL_NODES:-20}"
CONTROL_NODES="${CONTROL_NODES:-5}"
MAX_CONTROL_NODES="$CONTROL_NODES"
if ((CONTROL_NODES < 1 || CONTROL_NODES > TOTAL_NODES)); then
	echo "Error: CONTROL_NODES must be within 1..TOTAL_NODES (got $CONTROL_NODES/$TOTAL_NODES)." >&2
	exit 1
fi
WORKER_NODES=$((TOTAL_NODES - CONTROL_NODES))
LEARNER_NODES="${LEARNER_NODES:-5}"
if ((LEARNER_NODES > WORKER_NODES)); then
	echo "Warning: LEARNER_NODES=$LEARNER_NODES exceeds worker count $WORKER_NODES; clamping." >&2
	LEARNER_NODES="$WORKER_NODES"
fi
# All raft groups served by every node; used to verify per-group membership.
GROUP_IDS="${GROUP_IDS:-users orders products tasks}"
# Demo default: act on dead members / prune dead workers after 60s (the
# binary's own default is a conservative 300s).
VOTER_REPLACE_TIMEOUT_SECS="${VOTER_REPLACE_TIMEOUT_SECS:-60}"

DB_BASE="${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"
export DB_ROOT
LOG_DIR="$DB_ROOT/logs"

P2P_PORT_BASE="${P2P_PORT_BASE:-4000}"       # node i listens on P2P_PORT_BASE+i
HTTP_PORT_BASE="${HTTP_PORT_BASE:-3000}"     # node i http on HTTP_PORT_BASE+i
CONSOLE_PORT_BASE="${CONSOLE_PORT_BASE:-6668}" # node i tokio-console on CONSOLE_PORT_BASE+i

# Every HTTP probe in this script targets the loopback node APIs. If the
# environment has an http_proxy/all_proxy configured (common on machines behind
# a corporate/VPN proxy), curl would route these localhost requests through the
# proxy, which cannot reach 127.0.0.1 and returns 502 — making wait_for_http
# time out even though the node is healthy. Force loopback to bypass any proxy.
no_proxy="$(printf '127.0.0.1,localhost,::1%s' "${no_proxy:+,$no_proxy}")"
NO_PROXY="$no_proxy"
export no_proxy NO_PROXY

REDIS_PORT="${REDIS_PORT:-6380}"
REDIS_URL="${REDIS_URL:-redis://127.0.0.1:${REDIS_PORT}/}"
DISABLE_SQLITE_CACHE="${DISABLE_SQLITE_CACHE:-0}"
AUTO_START_REDIS="${AUTO_START_REDIS:-auto}"
REDIS_DIR="${REDIS_DIR:-$DB_ROOT/redis}"
REDIS_LOG="${REDIS_LOG:-$LOG_DIR/redis.log}"
REDIS_SERVER_BIN="${REDIS_SERVER_BIN:-}"
REDIS_PID=""

NODE_PIDS=()
NODE_NAMES=()
NODE_LOGS=()
NODE_PEER_IDS=()
SHUTTING_DOWN=0

CONTROL_UP_TIMEOUT_SECS="${CONTROL_UP_TIMEOUT_SECS:-120}"
LEARNER_ADD_TIMEOUT_SECS="${LEARNER_ADD_TIMEOUT_SECS:-120}"

if [[ "${RUSTFLAGS:-}" != *"tokio_unstable"* ]]; then
	export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg tokio_unstable"
fi

WSS_CERT_DIR="${WSS_CERT_DIR:-$DB_ROOT/certs}"
WSS_DNS_NAME="${WSS_DNS_NAME:-localhost}"
WSS_IP_ADDR="${WSS_IP_ADDR:-127.0.0.1}"
WS_TLS_KEY="${WS_TLS_KEY:-$WSS_CERT_DIR/private.der}"
WS_TLS_CERT="${WS_TLS_CERT:-$WSS_CERT_DIR/fullchain.der}"
WSS_DNS_NAMES="${WSS_DNS_NAMES:-$WSS_DNS_NAME}"
WSS_IP_ADDRS="${WSS_IP_ADDRS:-$WSS_IP_ADDR}"

trim_ws() {
	local value="$1"
	value="${value#"${value%%[![:space:]]*}"}"
	value="${value%"${value##*[![:space:]]}"}"
	printf '%s' "$value"
}

append_unique() {
	local list="$1"
	local value
	value="$(trim_ws "${2:-}")"
	[[ -z "$value" ]] && {
		printf '%s' "$list"
		return
	}

	local item
	IFS=',' read -r -a items <<<"$list"
	for item in "${items[@]}"; do
		item="$(trim_ws "$item")"
		if [[ "$item" == "$value" ]]; then
			printf '%s' "$list"
			return
		fi
	done

	if [[ -z "$list" ]]; then
		printf '%s' "$value"
	else
		printf '%s' "$list,$value"
	fi
}

detect_primary_ip() {
	if command -v ipconfig >/dev/null 2>&1; then
		ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || true
	elif command -v hostname >/dev/null 2>&1; then
		hostname -I 2>/dev/null | awk '{print $1}'
	elif command -v ip >/dev/null 2>&1; then
		ip -4 route get 1.1.1.1 2>/dev/null | awk '{for (i=1; i<=NF; i++) if ($i=="src") {print $(i+1); exit}}'
	fi
}

LAN_IP="$(detect_primary_ip || true)"
WSS_IP_ADDRS="$(append_unique "$WSS_IP_ADDRS" "127.0.0.1")"
if [[ -n "$LAN_IP" ]]; then
	WSS_IP_ADDRS="$(append_unique "$WSS_IP_ADDRS" "$LAN_IP")"
fi

CERT_META="$WSS_CERT_DIR/params.txt"
CERT_PROFILE="wss-v2"
CERT_PARAMS="profile=$CERT_PROFILE;dns=$WSS_DNS_NAMES;ips=$WSS_IP_ADDRS"
GEN_WSS_SCRIPT="$SCRIPT_DIR/generate_wss_certs.sh"
GEN_ID_SCRIPT="$SCRIPT_DIR/generate_libp2p_id.sh"

for script in "$GEN_WSS_SCRIPT" "$GEN_ID_SCRIPT"; do
	if [[ ! -f "$script" ]]; then
		echo "Error: missing $script" >&2
		exit 1
	fi
done

ensure_wss_certs() {
	mkdir -p "$WSS_CERT_DIR"
	if [[ -s "$WS_TLS_KEY" && -s "$WS_TLS_CERT" && -f "$CERT_META" ]]; then
		if [[ "$(cat "$CERT_META")" == "$CERT_PARAMS" && "${WSS_FORCE_REGEN:-0}" != "1" ]]; then
			return 0
		fi
	fi
	bash "$GEN_WSS_SCRIPT" "$WSS_CERT_DIR" "$WSS_DNS_NAMES" "$WSS_IP_ADDRS"
	printf '%s\n' "$CERT_PARAMS" >"$CERT_META"
}

tcp_port_open() {
	local host="$1"
	local port="$2"
	(echo >/dev/tcp/"$host"/"$port") >/dev/null 2>&1
}

wait_for_tcp_port() {
	local host="$1"
	local port="$2"
	local timeout="${3:-10}"
	local start=$SECONDS
	while ! tcp_port_open "$host" "$port"; do
		if ((SECONDS - start >= timeout)); then
			return 1
		fi
		sleep 0.1
	done
}

# Only LISTEN sockets count as "in use"; half-closed leftovers from a
# previous run (CLOSE_WAIT/FIN_WAIT) would otherwise cause false positives.
port_in_use() {
	local port="$1"
	if command -v lsof >/dev/null 2>&1; then
		lsof -nP -ti "tcp:${port}" -sTCP:LISTEN >/dev/null 2>&1
	else
		return 1
	fi
}

# Print "pid command" pairs for processes listening on the port.
port_listeners() {
	local port="$1"
	command -v lsof >/dev/null 2>&1 || return 0
	lsof -nP -i "tcp:${port}" -sTCP:LISTEN 2>/dev/null | awk 'NR>1 {print $2, $1}' | sort -u
}

collect_busy_ports() {
	BUSY_PORTS=()
	local i
	local port
	for ((i = 1; i <= TOTAL_NODES; i++)); do
		for port in $((P2P_PORT_BASE + i)) $((HTTP_PORT_BASE + i)) $((CONSOLE_PORT_BASE + i)); do
			if port_in_use "$port"; then
				BUSY_PORTS+=("$port")
			fi
		done
	done
}

# Terminate stale openraft_libp2p_cluster listeners on the given ports.
# Anything else listening there is somebody else's process; never touch it.
# Graceful node shutdown (4 raft groups + rocksdb sync + swarm) can take a
# while, so wait on process EXIT (not just port release) and escalate to
# SIGKILL after KILL_STALE_WAIT_SECS.
kill_stale_listeners() {
	local wait_secs="${KILL_STALE_WAIT_SECS:-20}"
	local port
	local pid
	local cmd
	local stale_pids=()

	for port in "$@"; do
		while read -r pid cmd; do
			[[ -z "$pid" ]] && continue
			case "$cmd" in
			openraft*)
				if [[ " ${stale_pids[*]-} " != *" $pid "* ]]; then
					echo "KILL_STALE=1: stopping stale cluster process $cmd (pid=$pid)"
					kill -TERM "$pid" 2>/dev/null || true
					stale_pids+=("$pid")
				fi
				;;
			*)
				echo "Refusing to kill non-cluster process $cmd (pid=$pid) on port $port." >&2
				;;
			esac
		done < <(port_listeners "$port")
	done

	((${#stale_pids[@]} == 0)) && return 0

	local deadline=$((SECONDS + wait_secs))
	local alive
	while ((SECONDS < deadline)); do
		alive=0
		for pid in "${stale_pids[@]}"; do
			if kill -0 "$pid" 2>/dev/null; then
				alive=1
				break
			fi
		done
		((alive == 0)) && return 0
		sleep 0.3
	done

	for pid in "${stale_pids[@]}"; do
		if kill -0 "$pid" 2>/dev/null; then
			echo "KILL_STALE=1: force killing stale cluster process pid=$pid (still alive after ${wait_secs}s)"
			kill -KILL "$pid" 2>/dev/null || true
		fi
	done
	# Give the kernel a moment to release the sockets before re-checking.
	sleep 1
}

ensure_ports_free() {
	collect_busy_ports
	((${#BUSY_PORTS[@]} == 0)) && return 0

	if [[ "${KILL_STALE:-0}" == "1" ]]; then
		kill_stale_listeners "${BUSY_PORTS[@]}"
		collect_busy_ports
		((${#BUSY_PORTS[@]} == 0)) && return 0
	fi

	echo "Error: the following ports are already in use: ${BUSY_PORTS[*]}" >&2
	local port
	local pid
	local cmd
	for port in "${BUSY_PORTS[@]}"; do
		while read -r pid cmd; do
			[[ -n "$pid" ]] && echo "  port $port <- pid=$pid cmd=$cmd" >&2
		done < <(port_listeners "$port")
	done
	echo "Hint: KILL_STALE=1 $0 stops stale openraft_libp2p_cluster listeners automatically;" >&2
	echo "      other processes must be stopped manually, or adjust" >&2
	echo "      P2P_PORT_BASE/HTTP_PORT_BASE/CONSOLE_PORT_BASE to avoid them." >&2
	exit 1
}

start_demo_redis() {
	if [[ "$DISABLE_SQLITE_CACHE" == "1" ]]; then
		echo "SQLite cache disabled by DISABLE_SQLITE_CACHE=1."
		return 0
	fi
	if [[ "$AUTO_START_REDIS" == "0" ]]; then
		echo "Using Redis URL: $REDIS_URL"
		return 0
	fi
	if tcp_port_open 127.0.0.1 "$REDIS_PORT"; then
		echo "Using existing Redis at $REDIS_URL"
		return 0
	fi
	local server_bin="$REDIS_SERVER_BIN"
	if [[ -z "$server_bin" ]]; then
		if command -v valkey-server >/dev/null 2>&1; then
			server_bin="valkey-server"
		elif command -v redis-server >/dev/null 2>&1; then
			server_bin="redis-server"
		fi
	fi
	if [[ -z "$server_bin" ]] || ! command -v "$server_bin" >/dev/null 2>&1; then
		echo "valkey-server/redis-server not found; disabling sqlite cache for this demo run."
		DISABLE_SQLITE_CACHE=1
		export DISABLE_SQLITE_CACHE
		return 0
	fi

	mkdir -p "$REDIS_DIR" "$(dirname "$REDIS_LOG")"
	echo "Starting demo Redis-compatible server at $REDIS_URL"
	"$server_bin" \
		--bind 127.0.0.1 \
		--port "$REDIS_PORT" \
		--dir "$REDIS_DIR" \
		--save "" \
		--appendonly no \
		--daemonize no \
		>"$REDIS_LOG" 2>&1 &
	REDIS_PID="$!"

	if ! wait_for_tcp_port 127.0.0.1 "$REDIS_PORT" "${REDIS_WAIT_SECS:-10}"; then
		echo "Demo Redis did not start; disabling sqlite cache for this demo run. See $REDIS_LOG"
		DISABLE_SQLITE_CACHE=1
		export DISABLE_SQLITE_CACHE
	fi
}

cleanup() {
	local status="${1:-0}"
	trap - INT TERM EXIT

	if ((SHUTTING_DOWN)); then
		exit "$status"
	fi
	SHUTTING_DOWN=1

	if ((${#NODE_PIDS[@]} > 0)); then
		echo "Stopping ${#NODE_PIDS[@]} node process(es) gracefully..."
		kill -TERM "${NODE_PIDS[@]}" 2>/dev/null || true
		local i
		local pid
		for i in "${!NODE_PIDS[@]}"; do
			pid="${NODE_PIDS[$i]}"
			wait "$pid" 2>/dev/null || true
		done
		NODE_PIDS=()
	fi

	if [[ -n "${REDIS_PID:-}" ]]; then
		if kill -0 "$REDIS_PID" 2>/dev/null; then
			echo "Stopping demo Redis process $REDIS_PID..."
			kill -TERM "$REDIS_PID" 2>/dev/null || true
		fi
		wait "$REDIS_PID" 2>/dev/null || true
		REDIS_PID=""
	fi
	echo "Stopped."
	exit "$status"
}

node_db_dir() {
	printf '%s/node%s-%s' "$DB_ROOT" "$1" "$1"
}

node_http() {
	printf '127.0.0.1:%s' "$((HTTP_PORT_BASE + $1))"
}

node_advertise_addr() {
	local index="$1"
	printf '/ip4/127.0.0.1/tcp/%s/wss/p2p/%s' "$((P2P_PORT_BASE + index))" "${NODE_PEER_IDS[$index]}"
}

start_node() {
	local index="$1"
	local name="node${index}"
	local db
	db="$(node_db_dir "$index")"
	local log="$LOG_DIR/${name}.log"
	local listen="/ip4/0.0.0.0/tcp/$((P2P_PORT_BASE + index))/wss"
	local http
	http="$(node_http "$index")"
	local console="127.0.0.1:$((CONSOLE_PORT_BASE + index))"
	local advertise
	advertise="$(node_advertise_addr "$index")"

	local cmd=(
		"$NODE_BIN"
		--id "${NODE_PEER_IDS[$index]}"
		--listen "$listen"
		--http "$http"
		--db "$db"
		--max-control-nodes "$MAX_CONTROL_NODES"
		--ws-tls-key "$WS_TLS_KEY"
		--ws-tls-cert "$WS_TLS_CERT"
		--bootstrap-node "$BOOTSTRAP_KV"
		--advertise "$advertise"
	)

	# Every node gets the same flags: control membership is decided by the
	# runtime join protocol, so any node may become (or be promoted to) a
	# control node and must be able to use the redis-backed sqlite cache.
	if [[ "${DISABLE_SQLITE_CACHE:-0}" != "1" ]]; then
		cmd+=(--redis-url "$REDIS_URL")
	else
		cmd+=(--disable-sqlite-cache)
	fi

	# Optional membership self-healing knobs (defaults live in the binary).
	if [[ -n "${VOTER_REPLACE_TIMEOUT_SECS:-}" ]]; then
		cmd+=(--voter-replace-timeout-secs "$VOTER_REPLACE_TIMEOUT_SECS")
	fi
	if [[ -n "${AUTO_HEAL_MEMBERSHIP:-}" ]]; then
		cmd+=(--auto-heal-membership "$AUTO_HEAL_MEMBERSHIP")
	fi

	RUST_LOG="${RUST_LOG:-info}" \
		LIBP2P_SELF_NAME="$name" \
		TOKIO_CONSOLE_BIND="$console" \
		"${cmd[@]}" >>"$log" 2>&1 &
	local pid=$!
	NODE_PIDS+=("$pid")
	NODE_NAMES+=("$name")
	NODE_LOGS+=("$log")
	echo "$name pid=$pid http=http://$http log=$log"
}

wait_for_http() {
	local index="$1"
	local timeout="${2:-60}"
	local url
	url="http://$(node_http "$index")/cluster"
	local start=$SECONDS
	while ! curl -fsS "$url" >/dev/null 2>&1; do
		if ((SECONDS - start >= timeout)); then
			echo "Error: node$index HTTP did not come up at $url within ${timeout}s." >&2
			return 1
		fi
		sleep 0.5
	done
}

openraft_voter_count() {
	local index="$1"
	local group="$2"
	local body
	body="$(curl -fsS "http://$(node_http "$index")/openraft/nodes?group_id=${group}" 2>/dev/null)" || return 1
	printf '%s' "$body" | grep -o '"voters":[0-9]*' | head -1 | cut -d: -f2
}

openraft_learner_count() {
	local index="$1"
	local group="$2"
	local body
	body="$(curl -fsS "http://$(node_http "$index")/openraft/nodes?group_id=${group}" 2>/dev/null)" || return 1
	printf '%s' "$body" | grep -o '"learners":[0-9]*' | head -1 | cut -d: -f2
}

# Wait until EVERY raft group reports the expected voter count; each group
# elects independently, so checking a single group can hide partial joins.
wait_for_voters() {
	local expected="$1"
	local timeout="$2"
	local start=$SECONDS
	local group
	local voters
	local pending
	while true; do
		pending=""
		for group in $GROUP_IDS; do
			voters="$(openraft_voter_count 1 "$group" || true)"
			if [[ -z "$voters" ]] || ((voters < expected)); then
				pending="$pending $group=${voters:-?}"
			fi
		done
		if [[ -z "$pending" ]]; then
			echo "All raft groups have $expected voters."
			return 0
		fi
		if ((SECONDS - start >= timeout)); then
			echo "Error: expected $expected voters in every group within ${timeout}s (pending:$pending)." >&2
			return 1
		fi
		sleep 1
	done
}

# Node ids that are currently OpenRaft voters, as reported by node1 for the
# first raft group. Control membership is decided by the runtime join
# protocol, so ask the cluster which nodes actually joined instead of
# assuming fixed indices.
openraft_voter_node_ids() {
	local group="${GROUP_IDS%% *}"
	curl -fsS "http://$(node_http 1)/openraft/nodes?group_id=${group}" 2>/dev/null |
		grep -o '"node_id":"[^"]*"[^{]*"role":"[^"]*"' |
		grep -E '"role":"(leader|follower)"' |
		sed 's/"node_id":"\([^"]*\)".*/\1/'
}

# Register node <index> as an OpenRaft learner (promote=false) by asking each
# node in turn until the current leader accepts the membership change; nodes
# that did not join the control membership simply reject and are skipped.
add_learner() {
	local index="$1"
	local timeout="$2"
	local peer="${NODE_PEER_IDS[$index]}"
	local addr
	addr="$(node_advertise_addr "$index")"
	local body
	body="$(printf '{"node_id":"%s","addr":"%s","promote":false}' "$peer" "$addr")"
	local start=$SECONDS
	local c
	local resp

	while true; do
		for ((c = 1; c <= TOTAL_NODES; c++)); do
			resp="$(curl -fsS -X POST "http://$(node_http "$c")/openraft/membership/add" \
				-H 'content-type: application/json' \
				-d "$body" 2>/dev/null)" || continue
			if [[ "$resp" == '{"ok":true'* ]]; then
				echo "node$index registered as OpenRaft learner (via node$c)."
				return 0
			fi
		done
		if ((SECONDS - start >= timeout)); then
			echo "Error: failed to add node$index as learner within ${timeout}s. Last response: ${resp:-<none>}" >&2
			return 1
		fi
		sleep 1
	done
}

# Portable in-place Fisher-Yates shuffle of the SHUFFLED array.
shuffle_array() {
	local i j tmp
	for ((i = ${#SHUFFLED[@]} - 1; i > 0; i--)); do
		j=$((RANDOM % (i + 1)))
		tmp="${SHUFFLED[$i]}"
		SHUFFLED[i]="${SHUFFLED[$j]}"
		SHUFFLED[j]="$tmp"
	done
}

random_sleep() {
	# 0.2s .. ~2.1s
	sleep "$((RANDOM % 2)).$((RANDOM % 10))2"
}

trap 'cleanup 130' INT
trap 'cleanup 143' TERM
trap 'cleanup $?' EXIT

mkdir -p "$LOG_DIR"
ensure_ports_free
ensure_wss_certs
start_demo_redis
export REDIS_URL DISABLE_SQLITE_CACHE MAX_CONTROL_NODES

cd "$WS_DIR"
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
	echo "Building..."
	cargo build -p openraft_libp2p_cluster >/dev/null
fi

NODE_BIN="${CARGO_TARGET_DIR:-$WS_DIR/target}/debug/openraft_libp2p_cluster"
if [[ ! -x "$NODE_BIN" ]]; then
	echo "Error: node binary not found at $NODE_BIN (build failed?)." >&2
	exit 1
fi

echo "Generating peer ids for $TOTAL_NODES nodes..."
NODE_PEER_IDS[0]="" # 1-based
for ((i = 1; i <= TOTAL_NODES; i++)); do
	db="$(node_db_dir "$i")"
	mkdir -p "$db"
	NODE_PEER_IDS[i]="$("$GEN_ID_SCRIPT" --key "$db/node.key" --out "$db/peer.id" | tail -1)"
done

BOOTSTRAP_KV="${NODE_PEER_IDS[1]}=$(node_advertise_addr 1)"

echo "Workspace:  $WS_DIR"
echo "export DB_ROOT=$DB_ROOT"
echo "Total nodes: $TOTAL_NODES (max openraft voters: $CONTROL_NODES, learners among workers: $LEARNER_NODES)"
echo "Self-healing: dead members replaced/removed and dead workers pruned after ${VOTER_REPLACE_TIMEOUT_SECS}s"
echo "Bootstrap:   $BOOTSTRAP_KV"
echo

echo "Starting node 1 (libp2p bootstrap)..."
start_node 1
wait_for_http 1 "$CONTROL_UP_TIMEOUT_SECS"

# Start every other node identically in random order: they race to join the
# openraft cluster on their own, and whichever nodes join first become the
# control voters (capped at CONTROL_NODES). The rest stay libp2p workers.
SHUFFLED=()
for ((i = 2; i <= TOTAL_NODES; i++)); do
	SHUFFLED+=("$i")
done
shuffle_array

echo "Starting nodes 2..$TOTAL_NODES in random order: ${SHUFFLED[*]}"
for i in "${SHUFFLED[@]}"; do
	start_node "$i"
	random_sleep
done

echo "Waiting for $CONTROL_NODES OpenRaft voters in every raft group..."
wait_for_voters "$CONTROL_NODES" "$CONTROL_UP_TIMEOUT_SECS"

# Pick LEARNER_NODES nodes at random among the ones that did NOT win a voter
# seat and register them as learners.
LEARNER_FAILURES=0
if ((LEARNER_NODES > 0)); then
	VOTER_IDS="$(openraft_voter_node_ids || true)"
	SHUFFLED=()
	for ((i = 1; i <= TOTAL_NODES; i++)); do
		if ! printf '%s\n' "$VOTER_IDS" | grep -qxF "${NODE_PEER_IDS[$i]}"; then
			SHUFFLED+=("$i")
		fi
	done
	shuffle_array
	if ((LEARNER_NODES > ${#SHUFFLED[@]})); then
		echo "Warning: only ${#SHUFFLED[@]} non-voter workers available; clamping LEARNER_NODES." >&2
		LEARNER_NODES=${#SHUFFLED[@]}
	fi
	LEARNER_PICKS=("${SHUFFLED[@]:0:$LEARNER_NODES}")

	echo "Registering $LEARNER_NODES randomly picked non-voter workers as OpenRaft learners: ${LEARNER_PICKS[*]}"
	for i in "${LEARNER_PICKS[@]}"; do
		wait_for_http "$i" "$CONTROL_UP_TIMEOUT_SECS" || {
			LEARNER_FAILURES=$((LEARNER_FAILURES + 1))
			continue
		}
		add_learner "$i" "$LEARNER_ADD_TIMEOUT_SECS" || LEARNER_FAILURES=$((LEARNER_FAILURES + 1))
		random_sleep
	done
else
	echo "No learner registration requested (LEARNER_NODES=0)."
fi

echo
echo "Cluster is up. Per-group membership (expected voters=$CONTROL_NODES learners=$LEARNER_NODES):"
for group in $GROUP_IDS; do
	voters="$(openraft_voter_count 1 "$group" || echo '?')"
	learners="$(openraft_learner_count 1 "$group" || echo '?')"
	echo "  group=$group voters=$voters learners=$learners"
done
if ((LEARNER_FAILURES > 0)); then
	echo "Warning: $LEARNER_FAILURES worker(s) failed learner registration; check logs in $LOG_DIR" >&2
fi
echo "Cluster graph: http://$(node_http 1)/graph"
echo "Node status:   http://$(node_http 1)/openraft/nodes"
echo "Libp2p info:   http://$(node_http 1)/libp2p/info"
echo "Logs:          $LOG_DIR"
echo "Press Ctrl-C to stop all nodes; individual node exits (crash drills) are tolerated."

# Alive means present in the process table and not a zombie: dead children
# stay as zombies until reaped, and kill -0 would still succeed on them.
node_alive() {
	local stat
	stat="$(ps -p "$1" -o stat= 2>/dev/null || true)"
	[[ -n "$stat" && "$stat" != Z* ]]
}

# Supervise instead of a bare `wait`: crash drills (crash-nodes.sh) kill
# individual nodes, and restart-nodes.sh revives them outside this script,
# so launcher-owned children dying must NOT end the launcher (the EXIT trap
# would tear down the remaining nodes and the demo Redis). Reap each dead
# child and keep running; only Ctrl-C/SIGTERM stops the whole cluster.
ALL_DEAD_REPORTED=0
while true; do
	for i in "${!NODE_PIDS[@]}"; do
		pid="${NODE_PIDS[$i]}"
		if ! node_alive "$pid"; then
			wait "$pid" 2>/dev/null || true # reap the zombie
			echo "${NODE_NAMES[$i]} (pid=$pid) exited; cluster keeps running (log: ${NODE_LOGS[$i]})."
			echo "  Restart it with: DB_ROOT=$DB_ROOT ./script/restart-nodes.sh"
			unset 'NODE_PIDS[i]' 'NODE_NAMES[i]' 'NODE_LOGS[i]'
		fi
	done
	if ((${#NODE_PIDS[@]} == 0 && ALL_DEAD_REPORTED == 0)); then
		ALL_DEAD_REPORTED=1
		echo "All launcher-owned node processes have exited (nodes revived by restart-nodes.sh run detached)."
		echo "Press Ctrl-C to stop the demo Redis and exit."
	fi
	sleep 2
done

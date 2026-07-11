#!/usr/bin/env bash
# Launch a 20-node local cluster:
#   - node1 bootstraps OpenRaft; nodes 2..5 join as control voters
#     (MAX_CONTROL_NODES=5 guarantees exactly 5 voters).
#   - nodes 6..20 start as libp2p workers in RANDOM order and are then
#     registered as OpenRaft learners (promote=false) through the leader's
#     HTTP API, also in random order.
#
# Tunables (env): TOTAL_NODES, CONTROL_NODES, DB_ROOT, REDIS_URL,
#   DISABLE_SQLITE_CACHE, P2P_PORT_BASE, HTTP_PORT_BASE, CONSOLE_PORT_BASE.
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

DB_BASE="${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"
export DB_ROOT
LOG_DIR="$DB_ROOT/logs"

P2P_PORT_BASE="${P2P_PORT_BASE:-4000}"       # node i listens on P2P_PORT_BASE+i
HTTP_PORT_BASE="${HTTP_PORT_BASE:-3000}"     # node i http on HTTP_PORT_BASE+i
CONSOLE_PORT_BASE="${CONSOLE_PORT_BASE:-6668}" # node i tokio-console on CONSOLE_PORT_BASE+i

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

port_in_use() {
	local port="$1"
	if command -v lsof >/dev/null 2>&1; then
		lsof -ti "tcp:${port}" >/dev/null 2>&1
	else
		return 1
	fi
}

ensure_ports_free() {
	local i
	local port
	for ((i = 1; i <= TOTAL_NODES; i++)); do
		for port in $((P2P_PORT_BASE + i)) $((HTTP_PORT_BASE + i)) $((CONSOLE_PORT_BASE + i)); do
			if port_in_use "$port"; then
				echo "Error: port $port is already in use (node$i)." >&2
				echo "Hint: stop previous runs, or adjust P2P_PORT_BASE/HTTP_PORT_BASE/CONSOLE_PORT_BASE." >&2
				exit 1
			fi
		done
	done
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

	echo "Stopping $TOTAL_NODES nodes gracefully..."
	if ((${#NODE_PIDS[@]} > 0)); then
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
	local role="$2" # control | worker
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

	if [[ "$role" == "control" && "${DISABLE_SQLITE_CACHE:-0}" != "1" ]]; then
		cmd+=(--redis-url "$REDIS_URL")
	else
		cmd+=(--disable-sqlite-cache)
	fi

	RUST_LOG="${RUST_LOG:-info}" \
		LIBP2P_SELF_NAME="$name" \
		TOKIO_CONSOLE_BIND="$console" \
		"${cmd[@]}" >>"$log" 2>&1 &
	local pid=$!
	NODE_PIDS+=("$pid")
	NODE_NAMES+=("$name")
	NODE_LOGS+=("$log")
	echo "$name ($role) pid=$pid http=http://$http log=$log"
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
	local body
	body="$(curl -fsS "http://$(node_http "$index")/openraft/nodes" 2>/dev/null)" || return 1
	printf '%s' "$body" | grep -o '"voters":[0-9]*' | head -1 | cut -d: -f2
}

openraft_learner_count() {
	local index="$1"
	local body
	body="$(curl -fsS "http://$(node_http "$index")/openraft/nodes" 2>/dev/null)" || return 1
	printf '%s' "$body" | grep -o '"learners":[0-9]*' | head -1 | cut -d: -f2
}

wait_for_voters() {
	local expected="$1"
	local timeout="$2"
	local start=$SECONDS
	local voters
	while true; do
		voters="$(openraft_voter_count 1 || true)"
		if [[ -n "$voters" ]] && ((voters >= expected)); then
			echo "OpenRaft control membership has $voters voters."
			return 0
		fi
		if ((SECONDS - start >= timeout)); then
			echo "Error: expected $expected control voters within ${timeout}s (got ${voters:-unknown})." >&2
			return 1
		fi
		sleep 1
	done
}

# Register node <index> as an OpenRaft learner (promote=false) by asking each
# control node until the current leader accepts the membership change.
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
		for ((c = 1; c <= CONTROL_NODES; c++)); do
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
echo "Total nodes: $TOTAL_NODES (control: $CONTROL_NODES, learner workers: $((TOTAL_NODES - CONTROL_NODES)))"
echo "Bootstrap:   $BOOTSTRAP_KV"
echo

echo "Starting control node 1 (bootstrap)..."
start_node 1 control
wait_for_http 1 "$CONTROL_UP_TIMEOUT_SECS"

echo "Starting control nodes 2..$CONTROL_NODES..."
for ((i = 2; i <= CONTROL_NODES; i++)); do
	start_node "$i" control
	sleep 1
done

echo "Waiting for $CONTROL_NODES OpenRaft control voters..."
wait_for_voters "$CONTROL_NODES" "$CONTROL_UP_TIMEOUT_SECS"

# Start the remaining nodes as workers in random order.
SHUFFLED=()
for ((i = CONTROL_NODES + 1; i <= TOTAL_NODES; i++)); do
	SHUFFLED+=("$i")
done
shuffle_array

echo "Starting $((TOTAL_NODES - CONTROL_NODES)) worker nodes in random order: ${SHUFFLED[*]}"
for i in "${SHUFFLED[@]}"; do
	start_node "$i" worker
	random_sleep
done

echo "Registering worker nodes as OpenRaft learners (random order)..."
shuffle_array
echo "Learner registration order: ${SHUFFLED[*]}"
LEARNER_FAILURES=0
for i in "${SHUFFLED[@]}"; do
	wait_for_http "$i" "$CONTROL_UP_TIMEOUT_SECS" || {
		LEARNER_FAILURES=$((LEARNER_FAILURES + 1))
		continue
	}
	add_learner "$i" "$LEARNER_ADD_TIMEOUT_SECS" || LEARNER_FAILURES=$((LEARNER_FAILURES + 1))
	random_sleep
done

voters="$(openraft_voter_count 1 || echo '?')"
learners="$(openraft_learner_count 1 || echo '?')"
echo
echo "Cluster is up: voters=$voters learners=$learners (expected: $CONTROL_NODES/$((TOTAL_NODES - CONTROL_NODES)))"
if ((LEARNER_FAILURES > 0)); then
	echo "Warning: $LEARNER_FAILURES worker(s) failed learner registration; check logs in $LOG_DIR" >&2
fi
echo "Cluster graph: http://$(node_http 1)/graph"
echo "Node status:   http://$(node_http 1)/openraft/nodes"
echo "Logs:          $LOG_DIR"
echo "Press Ctrl-C to stop all nodes."

wait

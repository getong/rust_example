#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
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

DB_BASE="${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"

export DB_ROOT

repo_path() {
	case "$1" in
	/*) printf '%s' "$1" ;;
	*) printf '%s/%s' "$ROOT_DIR" "$1" ;;
	esac
}

REDIS_PORT="${REDIS_PORT:-6380}"
REDIS_URL="${REDIS_URL:-redis://127.0.0.1:${REDIS_PORT}/}"
DISABLE_SQLITE_CACHE="${DISABLE_SQLITE_CACHE:-0}"
AUTO_START_REDIS="${AUTO_START_REDIS:-auto}"
REDIS_DIR="${REDIS_DIR:-$DB_ROOT/redis}"
REDIS_LOG="${REDIS_LOG:-$DB_ROOT/logs/redis.log}"
REDIS_SERVER_BIN="${REDIS_SERVER_BIN:-}"
VALKEY_CONFIG_DIR="${VALKEY_CONFIG_DIR:-config/valkey}"
VALKEY_CONFIG_DIR="$(repo_path "$VALKEY_CONFIG_DIR")"
VALKEY_CONFIG_FILE="${VALKEY_CONFIG_FILE:-$VALKEY_CONFIG_DIR/valkey.conf}"
VALKEY_CONFIG_FILE="$(repo_path "$VALKEY_CONFIG_FILE")"

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
GEN_WSS_SCRIPT="$ROOT_DIR/generate_wss_certs.sh"

# Serialize cert generation so parallel node startups do not mismatch key/cert.
acquire_cert_lock() {
	local lock_dir="$1"
	local pid_file="$2"
	local timeout="${3:-30}"
	local start=$SECONDS

	while ! mkdir "$lock_dir" 2>/dev/null; do
		if [[ -f "$pid_file" ]]; then
			local owner
			owner="$(cat "$pid_file" 2>/dev/null || true)"
			if [[ -n "$owner" ]] && ! kill -0 "$owner" 2>/dev/null; then
				rm -rf "$lock_dir"
				continue
			fi
		fi
		if ((SECONDS - start >= timeout)); then
			echo "Error: timed out waiting for WSS cert lock at $lock_dir"
			exit 1
		fi
		sleep 0.1
	done

	printf '%s\n' "$$" >"$pid_file"
}

cert_key_match() {
	local key_path="$1"
	local cert_path="$2"

	if ! command -v openssl >/dev/null 2>&1; then
		echo "Error: openssl is required but not found in PATH" >&2
		return 1
	fi

	local key_pub
	local cert_pub
	key_pub="$(openssl pkey -inform DER -in "$key_path" -pubout 2>/dev/null | openssl dgst -sha256 2>/dev/null)" || return 1
	cert_pub="$(openssl x509 -inform DER -in "$cert_path" -pubkey -noout 2>/dev/null | openssl dgst -sha256 2>/dev/null)" || return 1

	[[ "$key_pub" == "$cert_pub" ]]
}

ensure_wss_certs() {
	local lock_dir="$WSS_CERT_DIR/.wss-cert.lock"
	local pid_file="$lock_dir/pid"
	local timeout="${WSS_LOCK_TIMEOUT:-30}"

	mkdir -p "$WSS_CERT_DIR"

	(
		acquire_cert_lock "$lock_dir" "$pid_file" "$timeout"
		trap 'rm -rf "$lock_dir"' EXIT

		if [[ -s "$WS_TLS_KEY" && -s "$WS_TLS_CERT" && -f "$CERT_META" ]]; then
			if [[ "$(cat "$CERT_META")" == "$CERT_PARAMS" && "${WSS_FORCE_REGEN:-0}" != "1" ]]; then
				if cert_key_match "$WS_TLS_KEY" "$WS_TLS_CERT"; then
					exit 0
				fi
				echo "WSS cert/key mismatch detected; regenerating."
			fi
		fi
		if [[ ! -f "$GEN_WSS_SCRIPT" ]]; then
			echo "Error: missing $GEN_WSS_SCRIPT"
			exit 1
		fi
		bash "$GEN_WSS_SCRIPT" "$WSS_CERT_DIR" "$WSS_DNS_NAMES" "$WSS_IP_ADDRS"
		printf '%s\n' "$CERT_PARAMS" >"$CERT_META"
	)
}

ensure_wss_certs
export WS_TLS_KEY WS_TLS_CERT WSS_CERT_DIR WSS_DNS_NAME WSS_IP_ADDR WSS_DNS_NAMES WSS_IP_ADDRS

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

	local config_args=()
	if [[ -f "$VALKEY_CONFIG_FILE" ]]; then
		config_args+=("$VALKEY_CONFIG_FILE")
	fi

	mkdir -p "$REDIS_DIR" "$(dirname "$REDIS_LOG")"
	echo "Starting demo Redis-compatible server at $REDIS_URL"
	if (( ${#config_args[@]} > 0 )); then
		"$server_bin" "${config_args[@]}" \
			--bind 127.0.0.1 \
			--port "$REDIS_PORT" \
			--dir "$REDIS_DIR" \
			--save "" \
			--appendonly no \
			--daemonize no \
			>"$REDIS_LOG" 2>&1 &
	else
		"$server_bin" \
			--bind 127.0.0.1 \
			--port "$REDIS_PORT" \
			--dir "$REDIS_DIR" \
			--save "" \
			--appendonly no \
			--daemonize no \
			>"$REDIS_LOG" 2>&1 &
	fi

	if ! wait_for_tcp_port 127.0.0.1 "$REDIS_PORT" "${REDIS_WAIT_SECS:-10}"; then
		echo "Demo Redis did not start; disabling sqlite cache for this demo run. See $REDIS_LOG"
		DISABLE_SQLITE_CACHE=1
		export DISABLE_SQLITE_CACHE
	fi
}

cleanup() {
	echo "Stopping..."
	local pids
	pids="$(jobs -p)"
	if [[ -n "$pids" ]]; then
		kill $pids 2>/dev/null || true
	fi
}
trap cleanup INT TERM EXIT

start_demo_redis
export REDIS_URL DISABLE_SQLITE_CACHE

echo "Building..."
cd "$WS_DIR"
cargo build -p openraft_libp2p_cluster >/dev/null

export SKIP_BUILD=1

echo "Starting 3 nodes (Ctrl-C to stop)..."
echo "Tokio console:"
echo "  node1: ${NODE1_TOKIO_CONSOLE_BIND:-127.0.0.1:6669}"
echo "  node2: ${NODE2_TOKIO_CONSOLE_BIND:-127.0.0.1:6670}"
echo "  node3: ${NODE3_TOKIO_CONSOLE_BIND:-127.0.0.1:6671}"
echo "Connect with:"
echo "  tokio-console http://127.0.0.1:6669"
echo "  tokio-console http://127.0.0.1:6670"
echo "  tokio-console http://127.0.0.1:6671"
echo "Cluster graph:"
echo "  http://${NODE1_HTTP:-127.0.0.1:3001}/graph"
echo "  http://${NODE2_HTTP:-127.0.0.1:3002}/graph"
echo "  http://${NODE3_HTTP:-127.0.0.1:3003}/graph"
echo "External workers:"
echo "  DB_ROOT=$DB_ROOT WORKER_INDEX=1 ./run-worker.sh"
echo "  DB_ROOT=$DB_ROOT WORKER_INDEX=2 ./run-worker.sh"
echo "  DB_ROOT=$DB_ROOT ./join-4workers.sh"

"$ROOT_DIR/run-node1.sh" &

# Give node1 a moment to start listening.
sleep 1

"$ROOT_DIR/run-node2.sh" &

# Give node2 a moment to start listening.
sleep 1

"$ROOT_DIR/run-node3.sh" &

wait

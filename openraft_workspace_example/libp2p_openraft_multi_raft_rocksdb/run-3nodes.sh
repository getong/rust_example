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

DB_BASE="${DB_BASE:-/tmp/libp2p_openraft_multi_raft_rocksdb_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"

export DB_ROOT

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

echo "Building..."
cd "$WS_DIR"
cargo build -p libp2p_openraft_multi_raft_rocksdb >/dev/null

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

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
if [[ "${USE_ENV_DB_ROOT:-0}" == "1" ]]; then
	:
elif [[ "${IGNORE_ENV_DB_ROOT:-0}" == "1" && -z "$DB_ROOT_PRE" ]]; then
	echo "Ignoring DB_ROOT from env because IGNORE_ENV_DB_ROOT=1."
	unset DB_ROOT
fi

DB_BASE="${DB_BASE:-/tmp/libp2p_openraft_multi_raft_rocksdb_demo}"
DB_ROOT="${DB_ROOT:-$DB_BASE/$(date +%Y%m%d-%H%M%S)}"
NODE1_NAME="${NODE1_NAME:-node1}"
NODE2_NAME="${NODE2_NAME:-node2}"
NODE3_NAME="${NODE3_NAME:-node3}"
NODE1_DB="$DB_ROOT/${NODE1_NAME}-1"
NODE2_DB="$DB_ROOT/${NODE2_NAME}-2"
NODE3_DB="$DB_ROOT/${NODE3_NAME}-3"

NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001/wss}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002/wss}"
NODE3_LISTEN="${NODE3_LISTEN:-/ip4/127.0.0.1/tcp/4003/wss}"

NODE1_HTTP="${NODE1_HTTP:-127.0.0.1:3001}"
NODE2_HTTP="${NODE2_HTTP:-127.0.0.1:3002}"
NODE3_HTTP="${NODE3_HTTP:-127.0.0.1:3003}"

LOG_DIR="$DB_ROOT/logs"
NODE1_LOG="$LOG_DIR/node1.log"
NODE1_PEER_ID_FILE="$NODE1_DB/peer.id"
NODE2_PEER_ID_FILE="$NODE2_DB/peer.id"
NODE3_PEER_ID_FILE="$NODE3_DB/peer.id"

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
INIT_CLUSTER="${INIT_CLUSTER:-auto}"

mkdir -p "$NODE1_DB" "$NODE2_DB" "$NODE3_DB" "$LOG_DIR" "$WSS_CERT_DIR"

if [[ "${RESET:-0}" == "1" ]]; then
	# Only makes sense when user explicitly sets DB_ROOT. With default unique DB_ROOT it is harmless.
	rm -rf "$DB_ROOT"
	mkdir -p "$NODE1_DB" "$NODE2_DB" "$NODE3_DB" "$LOG_DIR" "$WSS_CERT_DIR"
fi

echo "Workspace: $WS_DIR"
echo "export DB_ROOT=$DB_ROOT"
echo "Node1 name: $NODE1_NAME"
echo "Node2 name: $NODE2_NAME"
echo "Node3 name: $NODE3_NAME"

PEER_ID_WAIT_SECS="${PEER_ID_WAIT_SECS:-120}"
GEN_SCRIPT="$ROOT_DIR/generate_libp2p_id.sh"
WSS_SCRIPT="$ROOT_DIR/generate_wss_certs.sh"

if [[ ! -x "$GEN_SCRIPT" ]]; then
	echo "Error: missing executable $GEN_SCRIPT"
	exit 1
fi
if [[ ! -f "$WSS_SCRIPT" ]]; then
	echo "Error: missing $WSS_SCRIPT"
	exit 1
fi

cd "$WS_DIR"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
	echo "Building..."
	cargo build -p libp2p_openraft_multi_raft_rocksdb >/dev/null
fi

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
		bash "$WSS_SCRIPT" "$WSS_CERT_DIR" "$WSS_DNS_NAMES" "$WSS_IP_ADDRS"
		printf '%s\n' "$CERT_PARAMS" >"$CERT_META"
	)
}

generate_peer_id() {
	local key_path="$1"
	local out_path="$2"
	"$GEN_SCRIPT" --key "$key_path" --out "$out_path"
}

db_has_rocksdb() {
	local dir="$1"
	[[ -f "$dir/CURRENT" ]]
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

P1="$(generate_peer_id "$NODE1_DB/node.key" "$NODE1_PEER_ID_FILE")"
P2="$(wait_for_peer_id "node2" "$NODE2_PEER_ID_FILE")"
P3="$(wait_for_peer_id "node3" "$NODE3_PEER_ID_FILE")"

ensure_wss_certs

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

if [[ "$NODE1_LISTEN" =~ /tcp/([0-9]+) ]]; then
	if port_in_use "${BASH_REMATCH[1]}"; then
		echo "Error: port ${BASH_REMATCH[1]} is already in use (NODE1_LISTEN=$NODE1_LISTEN)."
		echo "Hint: stop the previous nodes, or set NODE1_LISTEN/NODE2_LISTEN to other ports."
		exit 1
	fi
fi

export RUST_LOG="${RUST_LOG:-info}"
export LIBP2P_SELF_NAME="$NODE1_NAME"

echo "Logs:"
echo "  $NODE1_LOG"

echo "Starting node1 (Ctrl-C to stop)..."

should_init=0
case "$INIT_CLUSTER" in
1 | true | yes | always)
	should_init=1
	;;
0 | false | no | never)
	should_init=0
	;;
*)
	if ! db_has_rocksdb "$NODE1_DB"; then
		should_init=1
	fi
	;;
esac

if ((should_init == 0)) && db_has_rocksdb "$NODE1_DB"; then
	echo "Node1 DB already initialized; skipping --init (set INIT_CLUSTER=1 or RESET=1 to re-init)."
fi

cmd=(
	cargo run -p libp2p_openraft_multi_raft_rocksdb --bin libp2p_openraft_multi_raft_rocksdb --
	--id 1
	--listen "$NODE1_LISTEN"
	--http "$NODE1_HTTP"
	--db "$NODE1_DB"
	--ws-tls-key "$WS_TLS_KEY"
	--ws-tls-cert "$WS_TLS_CERT"
)

if [[ "${KAMEO_REMOTE:-0}" == "1" ]]; then
	cmd+=(--kameo-remote)
fi

if ((should_init == 1)); then
	cmd+=(--init)
fi

cmd+=(
	--node 1="$ADDR1"
	--node 2="$ADDR2"
	--node 3="$ADDR3"
)

"${cmd[@]}" 2>&1 | tee "$NODE1_LOG"

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

DB_ROOT="${DB_ROOT:-}"
if [[ -z "$DB_ROOT" ]]; then
	echo "Error: DB_ROOT is not set."
	echo "Hint: start the control plane with run-3nodes.sh first, then export the printed DB_ROOT."
	exit 1
fi

WORKER_INDEX="${WORKER_INDEX:-1}"
WORKER_NAME="${WORKER_NAME:-worker${WORKER_INDEX}}"
WORKER_DB="${WORKER_DB:-$DB_ROOT/${WORKER_NAME}}"
WORKER_LISTEN="${WORKER_LISTEN:-/ip4/0.0.0.0/tcp/$((4100 + WORKER_INDEX))/wss}"
WORKER_ADVERTISE_LISTEN="${WORKER_ADVERTISE_LISTEN:-/ip4/127.0.0.1/tcp/$((4100 + WORKER_INDEX))/wss}"
WORKER_HTTP="${WORKER_HTTP:-127.0.0.1:$((3100 + WORKER_INDEX))}"
WORKER_TOKIO_CONSOLE_BIND="${WORKER_TOKIO_CONSOLE_BIND:-127.0.0.1:$((6700 + WORKER_INDEX))}"

NODE1_NAME="${NODE1_NAME:-node1}"
NODE2_NAME="${NODE2_NAME:-node2}"
NODE3_NAME="${NODE3_NAME:-node3}"
NODE1_DB="$DB_ROOT/${NODE1_NAME}-1"
NODE2_DB="$DB_ROOT/${NODE2_NAME}-2"
NODE3_DB="$DB_ROOT/${NODE3_NAME}-3"

NODE1_LISTEN="${NODE1_LISTEN:-/ip4/127.0.0.1/tcp/4001/wss}"
NODE2_LISTEN="${NODE2_LISTEN:-/ip4/127.0.0.1/tcp/4002/wss}"
NODE3_LISTEN="${NODE3_LISTEN:-/ip4/127.0.0.1/tcp/4003/wss}"

LOG_DIR="$DB_ROOT/logs"
WORKER_LOG="$LOG_DIR/${WORKER_NAME}.log"
NODE1_PEER_ID_FILE="$NODE1_DB/peer.id"
NODE2_PEER_ID_FILE="$NODE2_DB/peer.id"
NODE3_PEER_ID_FILE="$NODE3_DB/peer.id"
WORKER_PEER_ID_FILE="$WORKER_DB/peer.id"

WSS_CERT_DIR="${WSS_CERT_DIR:-$DB_ROOT/certs}"
WSS_DNS_NAME="${WSS_DNS_NAME:-localhost}"
WSS_IP_ADDR="${WSS_IP_ADDR:-127.0.0.1}"
WS_TLS_KEY="${WS_TLS_KEY:-$WSS_CERT_DIR/private.der}"
WS_TLS_CERT="${WS_TLS_CERT:-$WSS_CERT_DIR/fullchain.der}"
WSS_DNS_NAMES="${WSS_DNS_NAMES:-$WSS_DNS_NAME}"
WSS_IP_ADDRS="${WSS_IP_ADDRS:-$WSS_IP_ADDR}"

mkdir -p "$WORKER_DB" "$LOG_DIR" "$WSS_CERT_DIR"

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

wait_for_peer_id() {
	local label="$1"
	local path="$2"
	local wait_secs="${PEER_ID_WAIT_SECS:-120}"
	local deadline=$((SECONDS + wait_secs))
	while [[ ! -s "$path" ]]; do
		if ((wait_secs == 0 || SECONDS >= deadline)); then
			echo "Error: timed out waiting for peer id for ${label} at ${path}"
			exit 1
		fi
		sleep 0.2
	done
	cat "$path"
}

generate_peer_id() {
	local key_path="$1"
	local out_path="$2"
	"$GEN_SCRIPT" --key "$key_path" --out "$out_path"
}

port_in_use() {
	local port="$1"
	if command -v lsof >/dev/null 2>&1; then
		lsof -ti "tcp:${port}" >/dev/null 2>&1
	else
		return 1
	fi
}

if [[ "$WORKER_LISTEN" =~ /tcp/([0-9]+) ]] && port_in_use "${BASH_REMATCH[1]}"; then
	echo "Error: port ${BASH_REMATCH[1]} is already in use (WORKER_LISTEN=$WORKER_LISTEN)."
	exit 1
fi

if [[ "$WORKER_TOKIO_CONSOLE_BIND" =~ :([0-9]+)$ ]] && port_in_use "${BASH_REMATCH[1]}"; then
	echo "Error: port ${BASH_REMATCH[1]} is already in use (WORKER_TOKIO_CONSOLE_BIND=$WORKER_TOKIO_CONSOLE_BIND)."
	exit 1
fi

cd "$WS_DIR"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
	echo "Building..."
	if [[ "${RUSTFLAGS:-}" != *"tokio_unstable"* ]]; then
		export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg tokio_unstable"
	fi
	cargo build -p openraft_libp2p_cluster >/dev/null
fi

if [[ ! -s "$WS_TLS_KEY" || ! -s "$WS_TLS_CERT" ]]; then
	bash "$WSS_SCRIPT" "$WSS_CERT_DIR" "$WSS_DNS_NAMES" "$WSS_IP_ADDRS"
fi

if [[ -n "${BOOTSTRAP_NODE:-}" ]]; then
	BOOTSTRAP_ARG="$BOOTSTRAP_NODE"
else
	P1="$(wait_for_peer_id "node1" "$NODE1_PEER_ID_FILE")"
	ADDR1="$NODE1_LISTEN/p2p/$P1"
	BOOTSTRAP_ARG="$P1=$ADDR1"
fi
PW="$(generate_peer_id "$WORKER_DB/node.key" "$WORKER_PEER_ID_FILE")"
WORKER_ADDR="${WORKER_ADVERTISE:-$WORKER_ADVERTISE_LISTEN/p2p/$PW}"

export RUST_LOG="${RUST_LOG:-info}"
export LIBP2P_SELF_NAME="$WORKER_NAME"
export TOKIO_CONSOLE_BIND="$WORKER_TOKIO_CONSOLE_BIND"

echo "Starting libp2p worker (Ctrl-C to stop)..."
echo "Worker peer id: $PW"
echo "Worker listen:  $WORKER_LISTEN"
echo "Worker addr:    $WORKER_ADDR"
echo "Worker HTTP:    $WORKER_HTTP"
echo "Bootstrap node:"
echo "  $BOOTSTRAP_ARG"
echo "Logs:"
echo "  $WORKER_LOG"

cmd=(
	cargo run -p openraft_libp2p_cluster --bin openraft_libp2p_cluster --
	--id "$PW"
	--listen "$WORKER_LISTEN"
	--http "$WORKER_HTTP"
	--db "$WORKER_DB"
	--ws-tls-key "$WS_TLS_KEY"
	--ws-tls-cert "$WS_TLS_CERT"
	--disable-sqlite-cache
	--bootstrap-node "$BOOTSTRAP_ARG"
	--advertise "$WORKER_ADDR"
)

"${cmd[@]}" 2>&1 | tee "$WORKER_LOG"

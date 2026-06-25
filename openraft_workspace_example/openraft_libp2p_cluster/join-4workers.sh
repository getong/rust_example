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
	unset DB_ROOT
fi

DB_ROOT="${DB_ROOT:-}"
if [[ -z "$DB_ROOT" ]]; then
	echo "Error: DB_ROOT is not set." >&2
	echo "Hint: use the DB_ROOT printed by ./run-3nodes.sh, for example:" >&2
	echo "  DB_ROOT=/tmp/openraft_libp2p_cluster_demo/<run-id> ./join-4workers.sh" >&2
	exit 1
fi

CLUSTER_HTTP="${CLUSTER_HTTP:-http://127.0.0.1:3001/cluster}"
WORKER_COUNT="${WORKER_COUNT:-4}"
WORKER_START_INDEX="${WORKER_START_INDEX:-1}"
WORKER_PORT_BASE="${WORKER_PORT_BASE:-4100}"
WORKER_HTTP_BASE="${WORKER_HTTP_BASE:-3100}"
WORKER_CONSOLE_BASE="${WORKER_CONSOLE_BASE:-6700}"
CLUSTER_WAIT_SECS="${CLUSTER_WAIT_SECS:-30}"
DRY_RUN="${DRY_RUN:-0}"

if ((WORKER_COUNT < 1)); then
	echo "Error: WORKER_COUNT must be >= 1." >&2
	exit 1
fi

fetch_cluster_info() {
	local deadline=$((SECONDS + CLUSTER_WAIT_SECS))
	while true; do
		if cluster_json="$(curl -fsS "$CLUSTER_HTTP" 2>/dev/null)"; then
			printf '%s' "$cluster_json"
			return 0
		fi
		if ((SECONDS >= deadline)); then
			echo "Error: failed to fetch cluster info from $CLUSTER_HTTP." >&2
			exit 1
		fi
		sleep 0.5
	done
}

parse_control_nodes() {
	if command -v jq >/dev/null 2>&1; then
		jq -r '
		  def from_object_nodes:
		    to_entries[] | "\(.key)=\(.value.addr // .value)";
		  def membership_nodes:
		    (.raft_metrics.membership_config.membership.nodes? // empty) |
		    if type == "object" then from_object_nodes else empty end;
		  [membership_nodes] as $members |
		  if ($members | length) == 3 then
		    $members[]
		  else
		    .known_nodes[] | "\(.node_id)=\(.addr)"
		  end
		'
		return 0
	fi

	if command -v python3 >/dev/null 2>&1; then
		python3 -c 'import json,sys
data=json.load(sys.stdin)
nodes=((data.get("raft_metrics") or {}).get("membership_config") or {}).get("membership", {}).get("nodes")
out=[]
if isinstance(nodes, dict):
    for node_id, node in nodes.items():
        addr = node.get("addr") if isinstance(node, dict) else node
        if node_id and addr:
            out.append(f"{node_id}={addr}")
if len(out) != 3:
    out=[]
    for node in data.get("known_nodes", []):
        node_id=node.get("node_id", "")
        addr=node.get("addr", "")
        if node_id and addr:
            out.append(f"{node_id}={addr}")
print("\n".join(out))'
		return 0
	fi

	echo "Error: jq or python3 is required to parse $CLUSTER_HTTP." >&2
	return 1
}

cluster_info="$(fetch_cluster_info)"
control_nodes=()
while IFS= read -r node; do
	[[ -z "$node" ]] && continue
	control_nodes+=("$node")
done < <(printf '%s' "$cluster_info" | parse_control_nodes)

if ((${#control_nodes[@]} != 3)); then
	echo "Error: expected exactly 3 control nodes from $CLUSTER_HTTP, got ${#control_nodes[@]}." >&2
	printf 'Parsed nodes:\n' >&2
	printf '  %s\n' "${control_nodes[@]}" >&2
	exit 1
fi

control_nodes_env="${control_nodes[*]}"

echo "Control plane from $CLUSTER_HTTP:"
printf '  %s\n' "${control_nodes[@]}"
echo "Starting $WORKER_COUNT external libp2p workers..."

if [[ "${SKIP_BUILD:-0}" != "1" && "$DRY_RUN" != "1" ]]; then
	echo "Building..."
	cd "$WS_DIR"
	if [[ "${RUSTFLAGS:-}" != *"tokio_unstable"* ]]; then
		export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg tokio_unstable"
	fi
	cargo build -p openraft_libp2p_cluster >/dev/null
	export SKIP_BUILD=1
fi

cleanup() {
	echo "Stopping workers..."
	local pids
	pids="$(jobs -p)"
	if [[ -n "$pids" ]]; then
		kill $pids 2>/dev/null || true
	fi
}
trap cleanup INT TERM EXIT

export SKIP_BUILD="${SKIP_BUILD:-0}"

for ((offset = 0; offset < WORKER_COUNT; offset++)); do
	index=$((WORKER_START_INDEX + offset))
	listen_port=$((WORKER_PORT_BASE + index))
	http_port=$((WORKER_HTTP_BASE + index))
	console_port=$((WORKER_CONSOLE_BASE + index))
	(
		export DB_ROOT
		export CONTROL_NODES="$control_nodes_env"
		export WORKER_INDEX="$index"
		export WORKER_NAME="${WORKER_NAME_PREFIX:-worker}${index}"
		export WORKER_LISTEN="${WORKER_LISTEN_PREFIX:-/ip4/0.0.0.0/tcp}/${listen_port}/wss"
		export WORKER_HTTP="${WORKER_HTTP_HOST:-127.0.0.1}:${http_port}"
		export WORKER_TOKIO_CONSOLE_BIND="${WORKER_CONSOLE_HOST:-127.0.0.1}:${console_port}"
		export SKIP_BUILD
		if [[ "$DRY_RUN" == "1" ]]; then
			printf 'DRY_RUN worker %s: WORKER_LISTEN=%s WORKER_HTTP=%s CONTROL_NODES=%q %s\n' \
				"$index" "$WORKER_LISTEN" "$WORKER_HTTP" "$CONTROL_NODES" "$ROOT_DIR/run-worker.sh"
			exit 0
		fi
		"$ROOT_DIR/run-worker.sh"
	) &
	sleep 0.5
done

echo "Worker HTTP endpoints:"
for ((offset = 0; offset < WORKER_COUNT; offset++)); do
	index=$((WORKER_START_INDEX + offset))
	http_port=$((WORKER_HTTP_BASE + index))
	echo "  http://${WORKER_HTTP_HOST:-127.0.0.1}:${http_port}"
done

wait

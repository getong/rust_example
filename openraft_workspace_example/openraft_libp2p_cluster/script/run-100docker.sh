#!/usr/bin/env bash
# Run a docker cluster (default 100 nodes; pass a count as the first argument,
# e.g. `./run-100docker.sh 20`).
#
#   - one user-defined bridge network (olpc-net) gives every container a DNS
#     name, so nodes advertise /dns4/olpc-node-<i>/tcp/4001/ws/p2p/<peerid>
#     (plain websocket: no wss certs needed inside the container network);
#   - olpc-node-1 gets a pre-generated libp2p key so its peer id is known up
#     front and can be handed to every node as the bootstrap address. Exactly
#     like run-20nodes.sh, no other node is pre-designated: nodes race to join
#     and the first CONTROL_NODES joiners become the OpenRaft voters;
#   - one valkey/redis container backs the sqlite cache of all nodes
#     (DISABLE_SQLITE_CACHE=1 skips it);
#   - node1's HTTP API is published on localhost:${NODE1_HTTP_PORT} for
#     inspection.
#
# Tunables (env): TOTAL_NODES, CONTROL_NODES, IMAGE, TAG, NET, PREFIX,
#   NODE1_HTTP_PORT, P2P_PORT, HTTP_PORT, MAX_CONTROL_NODES, GROUP_IDS,
#   DISABLE_SQLITE_CACHE, REDIS_IMAGE, NODE_CPUS, NODE_MEMORY, SKIP_BUILD,
#   STATE_DIR, RUST_LOG, VOTER_REPLACE_TIMEOUT_SECS.
#
# Stop everything with ./script/stop-docker-nodes.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ $# -ge 1 ]]; then
	if [[ ! "$1" =~ ^[0-9]+$ ]] || (($1 < 1)); then
		echo "Error: node count must be a positive integer (got '$1')." >&2
		echo "Usage: $0 [total_nodes]" >&2
		exit 1
	fi
	TOTAL_NODES="$1"
fi
TOTAL_NODES="${TOTAL_NODES:-100}"
CONTROL_NODES="${CONTROL_NODES:-5}"
if ((CONTROL_NODES < 1 || CONTROL_NODES > TOTAL_NODES)); then
	echo "Error: CONTROL_NODES must be within 1..TOTAL_NODES (got $CONTROL_NODES/$TOTAL_NODES)." >&2
	exit 1
fi

IMAGE="${IMAGE:-openraft-libp2p-cluster}"
TAG="${TAG:-0.1}"
NET="${NET:-olpc-net}"
PREFIX="${PREFIX:-olpc-node}"
LABEL="olpc-cluster=demo"
P2P_PORT="${P2P_PORT:-4001}"
HTTP_PORT="${HTTP_PORT:-3000}"
NODE1_HTTP_PORT="${NODE1_HTTP_PORT:-3001}"
MAX_CONTROL_NODES="${MAX_CONTROL_NODES:-$CONTROL_NODES}"
GROUP_IDS="${GROUP_IDS:-users orders products tasks}"
VOTER_REPLACE_TIMEOUT_SECS="${VOTER_REPLACE_TIMEOUT_SECS:-60}"
DISABLE_SQLITE_CACHE="${DISABLE_SQLITE_CACHE:-0}"
REDIS_IMAGE="${REDIS_IMAGE:-valkey/valkey:8-alpine}"
REDIS_NAME="${REDIS_NAME:-olpc-redis}"
STATE_DIR="${STATE_DIR:-/tmp/olpc-docker-cluster}"
# Same large-cluster caps as run-20nodes.sh: 2 tokio + 2 rayon threads per
# node and wide raft timings, or 100 nodes on one docker host oversubscribe
# the machine into election storms.
RUST_LOG="${RUST_LOG:-warn,openraft_libp2p_cluster::wasm_sync=info}"
TOKIO_WORKER_THREADS="${TOKIO_WORKER_THREADS:-2}"
RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-2}"
RAFT_KEEPALIVE_MS="${RAFT_KEEPALIVE_MS:-2000}"
RAFT_ELECTION_TIMEOUT_MIN_MS="${RAFT_ELECTION_TIMEOUT_MIN_MS:-6000}"
RAFT_ELECTION_TIMEOUT_MAX_MS="${RAFT_ELECTION_TIMEOUT_MAX_MS:-12000}"
# Optional per-container resource caps, e.g. NODE_CPUS=0.5 NODE_MEMORY=512m
NODE_CPUS="${NODE_CPUS:-}"
NODE_MEMORY="${NODE_MEMORY:-}"

CONTROL_UP_TIMEOUT_SECS="${CONTROL_UP_TIMEOUT_SECS:-$((180 + 2 * TOTAL_NODES))}"

# Localhost probes must never go through an http proxy.
no_proxy="$(printf '127.0.0.1,localhost,::1%s' "${no_proxy:+,$no_proxy}")"
NO_PROXY="$no_proxy"
export no_proxy NO_PROXY

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
	"$SCRIPT_DIR/build-docker.sh"
fi

echo "Cleaning up any previous olpc docker cluster..."
"$SCRIPT_DIR/stop-docker-nodes.sh" >/dev/null 2>&1 || true

docker network inspect "$NET" >/dev/null 2>&1 || docker network create "$NET" >/dev/null

if [[ "$DISABLE_SQLITE_CACHE" != "1" ]]; then
	echo "Starting $REDIS_NAME ($REDIS_IMAGE)..."
	docker run -d --name "$REDIS_NAME" --hostname "$REDIS_NAME" \
		--network "$NET" --label "$LABEL" \
		"$REDIS_IMAGE" --save "" --appendonly no >/dev/null
	REDIS_URL="redis://${REDIS_NAME}:6379/"
else
	REDIS_URL=""
fi

# Pre-generate the bootstrap (node1) libp2p key so its peer id is known before
# any container starts; every node gets it as the bootstrap address.
KEY_DIR="$STATE_DIR/bootstrap-key"
rm -rf "$KEY_DIR"
mkdir -p "$KEY_DIR"
echo "Generating bootstrap peer id..."
BOOT_PEER_ID="$(docker run --rm -v "$KEY_DIR":/keys --entrypoint olpc-peer-id \
	"${IMAGE}:${TAG}" --key /keys/node.key --create | tail -1)"
if [[ -z "$BOOT_PEER_ID" ]]; then
	echo "Error: failed to generate bootstrap peer id." >&2
	exit 1
fi
BOOTSTRAP="${BOOT_PEER_ID}=/dns4/${PREFIX}-1/tcp/${P2P_PORT}/ws/p2p/${BOOT_PEER_ID}"
echo "Bootstrap: $BOOTSTRAP"

common_args=(
	--network "$NET"
	--label "$LABEL"
	-e P2P_PORT="$P2P_PORT"
	-e HTTP_PORT="$HTTP_PORT"
	-e MAX_CONTROL_NODES="$MAX_CONTROL_NODES"
	-e BOOTSTRAP="$BOOTSTRAP"
	-e RUST_LOG="$RUST_LOG"
	-e TOKIO_WORKER_THREADS="$TOKIO_WORKER_THREADS"
	-e RAYON_NUM_THREADS="$RAYON_NUM_THREADS"
	-e RAFT_KEEPALIVE_MS="$RAFT_KEEPALIVE_MS"
	-e RAFT_ELECTION_TIMEOUT_MIN_MS="$RAFT_ELECTION_TIMEOUT_MIN_MS"
	-e RAFT_ELECTION_TIMEOUT_MAX_MS="$RAFT_ELECTION_TIMEOUT_MAX_MS"
	-e VOTER_REPLACE_TIMEOUT_SECS="$VOTER_REPLACE_TIMEOUT_SECS"
)
if [[ -n "$REDIS_URL" ]]; then
	common_args+=(-e REDIS_URL="$REDIS_URL")
else
	common_args+=(-e DISABLE_SQLITE_CACHE=1)
fi
if [[ -n "$NODE_CPUS" ]]; then
	common_args+=(--cpus "$NODE_CPUS")
fi
if [[ -n "$NODE_MEMORY" ]]; then
	common_args+=(--memory "$NODE_MEMORY")
fi

start_node() {
	local index="$1"
	local name="${PREFIX}-${index}"
	local extra=()
	if ((index == 1)); then
		extra+=(-v "$KEY_DIR":/keys:ro -e BOOTSTRAP_KEY_FILE=/keys/node.key
			-p "${NODE1_HTTP_PORT}:${HTTP_PORT}")
	fi
	# ${extra[@]+...}: empty-array expansion is an unbound-variable error
	# under `set -u` on bash 3.2 (stock macOS).
	docker run -d --name "$name" --hostname "$name" \
		"${common_args[@]}" ${extra[@]+"${extra[@]}"} \
		-e ADVERTISE_HOST="$name" \
		"${IMAGE}:${TAG}" >/dev/null
	echo "$name started"
}

node1_http="http://127.0.0.1:${NODE1_HTTP_PORT}"

wait_for_node1_http() {
	local timeout="$1"
	local start=$SECONDS
	while ! curl -fsS --max-time 5 "$node1_http/cluster" >/dev/null 2>&1; do
		if ((SECONDS - start >= timeout)); then
			echo "Error: node1 HTTP did not come up at $node1_http within ${timeout}s." >&2
			echo "       docker logs ${PREFIX}-1" >&2
			exit 1
		fi
		sleep 0.5
	done
}

voter_count() {
	local group="$1"
	curl -fsS --max-time 5 "$node1_http/openraft/nodes?group_id=${group}" 2>/dev/null |
		grep -o '"voters":[0-9]*' | head -1 | cut -d: -f2
}

wait_for_voters() {
	local expected="$1"
	local timeout="$2"
	local start=$SECONDS
	local group voters pending
	while true; do
		pending=""
		for group in $GROUP_IDS; do
			voters="$(voter_count "$group" || true)"
			if [[ -z "$voters" ]] || ((voters < expected)); then
				pending="$pending $group=${voters:-?}"
			fi
		done
		if [[ -z "$pending" ]]; then
			echo "All raft groups have $expected voters."
			return 0
		fi
		if ((SECONDS - start >= timeout)); then
			echo "Warning: expected $expected voters in every group within ${timeout}s (pending:$pending)." >&2
			echo "         The cluster keeps converging in the background; check $node1_http/openraft/nodes" >&2
			return 0
		fi
		sleep 2
	done
}

echo "Starting node 1 (libp2p + openraft bootstrap)..."
start_node 1
wait_for_node1_http "$CONTROL_UP_TIMEOUT_SECS"

# Start the rest in random order: they race to join the control membership on
# their own; the first joiners become the voters.
SHUFFLED=()
for ((i = 2; i <= TOTAL_NODES; i++)); do
	SHUFFLED+=("$i")
done
for ((i = ${#SHUFFLED[@]} - 1; i > 0; i--)); do
	j=$((RANDOM % (i + 1)))
	tmp="${SHUFFLED[$i]}"
	SHUFFLED[i]="${SHUFFLED[$j]}"
	SHUFFLED[j]="$tmp"
done

echo "Starting nodes 2..$TOTAL_NODES..."
for i in ${SHUFFLED[@]+"${SHUFFLED[@]}"}; do
	start_node "$i"
	sleep "0.$((RANDOM % 3 + 1))"
done

echo "Waiting for $CONTROL_NODES OpenRaft voters in every raft group..."
wait_for_voters "$CONTROL_NODES" "$CONTROL_UP_TIMEOUT_SECS"

echo
echo "Cluster is up: $TOTAL_NODES nodes on docker network '$NET'."
echo "Cluster graph: $node1_http/graph"
echo "Node status:   $node1_http/openraft/nodes"
echo "Libp2p info:   $node1_http/libp2p/info"
echo "Node logs:     docker logs -f ${PREFIX}-<i>"
echo "List nodes:    docker ps --filter label=$LABEL"
echo "Stop cluster:  $SCRIPT_DIR/stop-docker-nodes.sh"

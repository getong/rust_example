#!/usr/bin/env bash
# Deploy the cluster to kubernetes (default 100 nodes = 1 bootstrap + 99
# workers; pass a count as the first argument, e.g. `./k8s-run-100nodes.sh 20`).
#
# Steps:
#   1. build the docker image (SKIP_BUILD=1 to skip) and optionally load it
#      into the cluster runtime (LOAD_MODE=kind|minikube|none, or PUSH=1 with
#      a registry-qualified IMAGE);
#   2. generate the bootstrap node's libp2p key locally (docker run) so its
#      peer id is known up front;
#   3. create Secret olpc-bootstrap-key + ConfigMap olpc-cluster-config, apply
#      k8s/olpc-cluster.yaml, scale workers to <total_nodes> - 1;
#   4. wait for the bootstrap pod and verify the OpenRaft voter count from
#      inside it.
#
# Requirements: docker, kubectl (current context), and enough cluster capacity
# for <total_nodes> pods (defaults request 100m CPU / 128Mi each => 100 nodes
# need ~10 CPUs / ~13Gi of schedulable requests).
#
# Tunables (env): IMAGE, TAG, LOAD_MODE, KIND_CLUSTER, PUSH, SKIP_BUILD,
#   CONTROL_NODES, MAX_CONTROL_NODES, GROUP_IDS, DISABLE_SQLITE_CACHE,
#   RUST_LOG, RAFT_*, VOTER_REPLACE_TIMEOUT_SECS, STATE_DIR.
#
# Tear down with ./script/k8s-delete.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ $# -ge 1 ]]; then
	if [[ ! "$1" =~ ^[0-9]+$ ]] || (($1 < 2)); then
		echo "Error: node count must be an integer >= 2 (got '$1')." >&2
		echo "Usage: $0 [total_nodes]" >&2
		exit 1
	fi
	TOTAL_NODES="$1"
fi
TOTAL_NODES="${TOTAL_NODES:-100}"
WORKER_REPLICAS=$((TOTAL_NODES - 1))
CONTROL_NODES="${CONTROL_NODES:-5}"
MAX_CONTROL_NODES="${MAX_CONTROL_NODES:-$CONTROL_NODES}"
GROUP_IDS="${GROUP_IDS:-users orders products tasks}"

NS=olpc
IMAGE="${IMAGE:-openraft-libp2p-cluster}"
TAG="${TAG:-0.1}"
# How the locally built image reaches the cluster's container runtime:
#   auto           docker-desktop context -> docker-desktop, else none
#   docker-desktop import into each containerd k8s node via
#                  `docker save | docker exec <node> ctr images import`
#                  (Docker Desktop's multi-node kubernetes runs kind-style
#                  containerd nodes that do NOT see `docker build` images)
#   kind/minikube  load through the respective CLI
#   none           image already reachable (pushed registry image)
LOAD_MODE="${LOAD_MODE:-auto}"
STATE_DIR="${STATE_DIR:-/tmp/olpc-k8s-cluster}"
VOTER_REPLACE_TIMEOUT_SECS="${VOTER_REPLACE_TIMEOUT_SECS:-60}"
RUST_LOG="${RUST_LOG:-warn,openraft_libp2p_cluster::wasm_sync=info}"
RAFT_KEEPALIVE_MS="${RAFT_KEEPALIVE_MS:-2000}"
RAFT_ELECTION_TIMEOUT_MIN_MS="${RAFT_ELECTION_TIMEOUT_MIN_MS:-6000}"
RAFT_ELECTION_TIMEOUT_MAX_MS="${RAFT_ELECTION_TIMEOUT_MAX_MS:-12000}"
DISABLE_SQLITE_CACHE="${DISABLE_SQLITE_CACHE:-0}"

CONTROL_UP_TIMEOUT_SECS="${CONTROL_UP_TIMEOUT_SECS:-$((300 + 2 * TOTAL_NODES))}"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
	IMAGE="$IMAGE" TAG="$TAG" PUSH="${PUSH:-0}" "$SCRIPT_DIR/build-docker.sh"
fi

# Import the image into every containerd k8s node container (Docker Desktop
# multi-node kubernetes / any kind-style cluster whose node names are
# exec-able docker containers).
load_via_ctr() {
	local tar
	tar="$(mktemp -t olpc-image.XXXXXX.tar)"
	echo "Exporting ${IMAGE}:${TAG} for the k8s nodes..."
	docker save -o "$tar" "${IMAGE}:${TAG}"
	local node
	for node in $(kubectl get nodes -o jsonpath='{.items[*].metadata.name}'); do
		echo "Importing image into node $node..."
		if ! docker exec -i "$node" ctr -n k8s.io images import - <"$tar" >/dev/null; then
			echo "Error: failed to import image into node $node." >&2
			rm -f "$tar"
			exit 1
		fi
	done
	rm -f "$tar"
}

if [[ "$LOAD_MODE" == "auto" ]]; then
	if [[ "$(kubectl config current-context 2>/dev/null)" == "docker-desktop" ]]; then
		LOAD_MODE="docker-desktop"
	else
		LOAD_MODE="none"
	fi
	echo "LOAD_MODE=auto -> $LOAD_MODE"
fi

case "$LOAD_MODE" in
none) ;;
docker-desktop)
	load_via_ctr
	;;
kind)
	kind load docker-image "${IMAGE}:${TAG}" --name "${KIND_CLUSTER:-kind}"
	;;
minikube)
	minikube image load "${IMAGE}:${TAG}"
	;;
*)
	echo "Error: LOAD_MODE must be auto, none, docker-desktop, kind or minikube (got '$LOAD_MODE')." >&2
	exit 1
	;;
esac

# Bootstrap identity: generated locally so the peer id can be published to
# every pod via the ConfigMap before anything starts.
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
echo "Bootstrap peer id: $BOOT_PEER_ID"

kubectl create namespace "$NS" --dry-run=client -o yaml | kubectl apply -f -

kubectl -n "$NS" create secret generic olpc-bootstrap-key \
	--from-file=node.key="$KEY_DIR/node.key" \
	--dry-run=client -o yaml | kubectl apply -f -

# Shared node env; entrypoint.sh turns these into CLI flags. Pods add their
# own ADVERTISE_HOST on top.
config_args=(
	--from-literal=BOOTSTRAP_PEER_ID="$BOOT_PEER_ID"
	--from-literal=BOOTSTRAP_HOST="olpc-bootstrap-0.olpc-bootstrap.${NS}.svc.cluster.local"
	--from-literal=MAX_CONTROL_NODES="$MAX_CONTROL_NODES"
	--from-literal=VOTER_REPLACE_TIMEOUT_SECS="$VOTER_REPLACE_TIMEOUT_SECS"
	--from-literal=RUST_LOG="$RUST_LOG"
	--from-literal=TOKIO_WORKER_THREADS="2"
	--from-literal=RAYON_NUM_THREADS="2"
	--from-literal=RAFT_KEEPALIVE_MS="$RAFT_KEEPALIVE_MS"
	--from-literal=RAFT_ELECTION_TIMEOUT_MIN_MS="$RAFT_ELECTION_TIMEOUT_MIN_MS"
	--from-literal=RAFT_ELECTION_TIMEOUT_MAX_MS="$RAFT_ELECTION_TIMEOUT_MAX_MS"
)
if [[ "$DISABLE_SQLITE_CACHE" == "1" ]]; then
	config_args+=(--from-literal=DISABLE_SQLITE_CACHE=1)
else
	config_args+=(--from-literal=REDIS_URL="redis://olpc-redis.${NS}.svc.cluster.local:6379/")
fi
kubectl -n "$NS" create configmap olpc-cluster-config "${config_args[@]}" \
	--dry-run=client -o yaml | kubectl apply -f -

kubectl apply -f "$ROOT_DIR/k8s/olpc-cluster.yaml"
# The yaml carries a default image tag; pin both statefulsets to the tag that
# was actually built/loaded so IMAGE/TAG overrides always win.
kubectl -n "$NS" set image statefulset/olpc-bootstrap node="${IMAGE}:${TAG}"
kubectl -n "$NS" set image statefulset/olpc-worker node="${IMAGE}:${TAG}"
kubectl -n "$NS" scale statefulset olpc-worker --replicas="$WORKER_REPLICAS"

echo "Waiting for the bootstrap pod..."
kubectl -n "$NS" rollout status statefulset/olpc-bootstrap --timeout="${CONTROL_UP_TIMEOUT_SECS}s"

echo "Waiting for $WORKER_REPLICAS worker pods (this can take a while)..."
if ! kubectl -n "$NS" rollout status statefulset/olpc-worker --timeout="${CONTROL_UP_TIMEOUT_SECS}s"; then
	echo "Warning: not all workers became ready in time; the cluster keeps converging." >&2
fi

# Query the bootstrap pod's HTTP API through the apiserver pod proxy: the
# node image ships no curl, and this needs no in-pod tooling at all.
voter_count() {
	local group="$1"
	kubectl get --raw \
		"/api/v1/namespaces/${NS}/pods/olpc-bootstrap-0:3000/proxy/openraft/nodes?group_id=${group}" 2>/dev/null |
		grep -o '"voters":[0-9]*' | head -1 | cut -d: -f2
}

echo "Waiting for $CONTROL_NODES OpenRaft voters in every raft group..."
start=$SECONDS
while true; do
	pending=""
	for group in $GROUP_IDS; do
		voters="$(voter_count "$group" || true)"
		if [[ -z "$voters" ]] || ((voters < CONTROL_NODES)); then
			pending="$pending $group=${voters:-?}"
		fi
	done
	if [[ -z "$pending" ]]; then
		echo "All raft groups have $CONTROL_NODES voters."
		break
	fi
	if ((SECONDS - start >= CONTROL_UP_TIMEOUT_SECS)); then
		echo "Warning: expected $CONTROL_NODES voters per group within ${CONTROL_UP_TIMEOUT_SECS}s (pending:$pending)." >&2
		echo "         The cluster keeps converging; re-check with the commands below." >&2
		break
	fi
	sleep 3
done

echo
echo "Cluster deployed: $TOTAL_NODES nodes (1 bootstrap + $WORKER_REPLICAS workers) in namespace $NS."
echo "Pods:          kubectl -n $NS get pods -o wide"
echo "Node status:   kubectl get --raw \"/api/v1/namespaces/$NS/pods/olpc-bootstrap-0:3000/proxy/openraft/nodes\""
echo "Local access:  kubectl -n $NS port-forward svc/olpc-bootstrap 3000:3000   # then http://127.0.0.1:3000/graph"
echo "Logs:          kubectl -n $NS logs -f olpc-worker-0"
echo "Tear down:     $SCRIPT_DIR/k8s-delete.sh"

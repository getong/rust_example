#!/usr/bin/env bash
set -euo pipefail

# Build and start a 6-node Redis Cluster (3 masters, 3 replicas).
# Defaults align with the ClickHouse helpers in this repo.
#
# Environment overrides:
#   REDIS_IMAGE_NAME   - docker image tag (default: redis-cluster-local)
#   REDIS_NETWORK_NAME - docker network (default: redis-cluster-net)
#   REDIS_FIRST_PORT   - first host port to map (default: 7000)
#   SKIP_BUILD         - set to 1 to reuse an existing image
#   NO_CACHE           - set to 0 to allow Docker layer cache

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IMAGE_NAME="${REDIS_IMAGE_NAME:-redis-cluster-local}"
NETWORK_NAME="${REDIS_NETWORK_NAME:-redis-cluster-net}"
SKIP_BUILD="${SKIP_BUILD:-0}"
NO_CACHE="${NO_CACHE:-0}"
FIRST_PORT="${REDIS_FIRST_PORT:-7000}"
PORT_SCAN_LIMIT="${REDIS_PORT_SCAN_LIMIT:-50}"

if [ "${SKIP_BUILD}" != "1" ]; then
  echo "Building ${IMAGE_NAME} from Dockerfile.redis.cluster..."
  if [ "${NO_CACHE}" = "1" ]; then
    docker build --no-cache -f "${ROOT_DIR}/Dockerfile.redis.cluster" -t "${IMAGE_NAME}" "${ROOT_DIR}"
  else
    docker build -f "${ROOT_DIR}/Dockerfile.redis.cluster" -t "${IMAGE_NAME}" "${ROOT_DIR}"
  fi
else
  echo "Skipping build (SKIP_BUILD=1). Using existing image: ${IMAGE_NAME}"
fi

if ! docker network ls --format '{{.Name}}' | grep -qx "${NETWORK_NAME}"; then
  echo "Creating docker network ${NETWORK_NAME}..."
  docker network create "${NETWORK_NAME}"
fi

find_free_port() {
  local start_port="$1"
  local attempts="$2"
  local port="${start_port}"
  local py_bin
  py_bin="$(command -v python3 || command -v python || true)"
  if [ -z "${py_bin}" ]; then
    echo "python3/python is required to probe free ports" >&2
    return 1
  fi

  for _ in $(seq 1 "${attempts}"); do
    if "${py_bin}" - "${port}" <<'PY'
import socket
import sys

port = int(sys.argv[1])
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        s.bind(("0.0.0.0", port))
    except OSError:
        sys.exit(1)
sys.exit(0)
PY
    then
      echo "${port}"
      return 0
    fi
    port=$((port + 1))
  done

  echo "Could not find a free port starting at ${start_port} after ${attempts} attempts" >&2
  return 1
}

start_node() {
  local name="$1"
  local host_port="$2"

  if docker ps -a --format '{{.Names}}' | grep -Eq "^${name}\$"; then
    echo "Container ${name} exists; removing..."
    docker rm -f "${name}" >/dev/null
  fi

  echo "Starting ${name} (host port ${host_port})..."
  docker run -d \
    --name "${name}" \
    --hostname "${name}" \
    --network "${NETWORK_NAME}" \
    -p "${host_port}:6379" \
    -e REDIS_ANNOUNCE_HOST="${name}" \
    "${IMAGE_NAME}"
}

nodes=()
host_ports=()
for i in $(seq 0 5); do
  desired_port=$((FIRST_PORT + i))
  port="$(find_free_port "${desired_port}" "${PORT_SCAN_LIMIT}")"
  if [ -z "${port}" ]; then
    echo "No available port found for node $((i + 1)); aborting" >&2
    exit 1
  fi
  if [ "${port}" != "${desired_port}" ]; then
    echo "Port ${desired_port} in use, shifting redis$((i + 1)) to ${port}"
  fi
  name="redis$((i + 1))"
  start_node "${name}" "${port}"
  nodes+=("${name}")
  host_ports+=("${port}")
done

echo "Creating Redis Cluster (3 masters, 3 replicas)..."
addresses=()
for name in "${nodes[@]}"; do
  addresses+=("${name}:6379")
done

docker exec -i "${nodes[0]}" redis-cli --cluster create "${addresses[@]}" --cluster-replicas 1 --cluster-yes

echo ""
echo "========================================="
echo "Redis Cluster is starting"
echo "========================================="
for i in "${!nodes[@]}"; do
  host_port="${host_ports[$i]}"
  echo "  - ${nodes[$i]} => redis://localhost:${host_port}"
done
echo ""
echo "Verify cluster state (wait a few seconds first):"
echo "  docker exec -it ${nodes[0]} redis-cli cluster info"
echo "  docker exec -it ${nodes[0]} redis-cli cluster nodes"

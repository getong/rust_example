#!/usr/bin/env bash
set -euo pipefail

# Build and start a 2-node ClickHouse cluster (two shards, one replica each).
# Defaults align with local samples; override with env vars if needed.
#
# Environment overrides:
#   IMAGE_NAME            - docker image tag (default: clickhouse-cluster)
#   NETWORK_NAME          - docker network (default: clickhouse-cluster-net)
#   CLICKHOUSE_PASSWORD   - ClickHouse password (default: changeme)
#   CLICKHOUSE_USER       - ClickHouse user (default: default)
#   CLICKHOUSE_DB         - default DB (default: test)
#   NODE1_HTTP_PORT       - host HTTP port for ch1 (default: 8123)
#   NODE1_TCP_PORT        - host TCP port for ch1 (default: 9000)
#   NODE1_INTERSERVER     - host interserver port for ch1 (default: 9009)
#   NODE2_HTTP_PORT       - host HTTP port for ch2 (default: 8124)
#   NODE2_TCP_PORT        - host TCP port for ch2 (default: 9001)
#   NODE2_INTERSERVER     - host interserver port for ch2 (default: 9010)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IMAGE_NAME="${IMAGE_NAME:-clickhouse-cluster}"
NETWORK_NAME="${NETWORK_NAME:-clickhouse-cluster-net}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-changeme}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_DB="${CLICKHOUSE_DB:-test}"

NODE1_HTTP_PORT="${NODE1_HTTP_PORT:-8123}"
NODE1_TCP_PORT="${NODE1_TCP_PORT:-9000}"
NODE1_INTERSERVER="${NODE1_INTERSERVER:-9009}"

NODE2_HTTP_PORT="${NODE2_HTTP_PORT:-8124}"
NODE2_TCP_PORT="${NODE2_TCP_PORT:-9001}"
NODE2_INTERSERVER="${NODE2_INTERSERVER:-9010}"

echo "Building ${IMAGE_NAME} from Dockerfile.clickhouse.cluster..."
docker build --pull -f "${ROOT_DIR}/Dockerfile.clickhouse.cluster" -t "${IMAGE_NAME}" "${ROOT_DIR}"

if ! docker network ls --format '{{.Name}}' | grep -qx "${NETWORK_NAME}"; then
  echo "Creating docker network ${NETWORK_NAME}..."
  docker network create "${NETWORK_NAME}"
fi

start_node() {
  local name="$1"
  local shard="$2"
  local replica="$3"
  local http_port="$4"
  local tcp_port="$5"
  local inter_port="$6"

  if docker ps -a --format '{{.Names}}' | grep -Eq "^${name}\$"; then
    echo "Container ${name} already exists. Removing it to apply fresh config..."
    docker rm -f "${name}" >/dev/null
  fi

  echo "Starting ${name} (shard=${shard}, replica=${replica})..."
  docker run -d \
    --name "${name}" \
    --hostname "${name}" \
    --network "${NETWORK_NAME}" \
    -p "${http_port}:8123" \
    -p "${tcp_port}:9000" \
    -p "${inter_port}:9009" \
    -e CLICKHOUSE_USER="${CLICKHOUSE_USER}" \
    -e CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD}" \
    -e CLICKHOUSE_DB="${CLICKHOUSE_DB}" \
    -e CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1 \
    -e CLICKHOUSE_SHARD="${shard}" \
    -e CLICKHOUSE_REPLICA="${replica}" \
    "${IMAGE_NAME}"
}

start_node ch1 1 1 "${NODE1_HTTP_PORT}" "${NODE1_TCP_PORT}" "${NODE1_INTERSERVER}"
start_node ch2 2 1 "${NODE2_HTTP_PORT}" "${NODE2_TCP_PORT}" "${NODE2_INTERSERVER}"

echo "Cluster is starting."
echo "Node ch1 HTTP: ${NODE1_HTTP_PORT}, TCP: ${NODE1_TCP_PORT}"
echo "Node ch2 HTTP: ${NODE2_HTTP_PORT}, TCP: ${NODE2_TCP_PORT}"
echo "Check logs: docker logs -f ch1  | docker logs -f ch2"

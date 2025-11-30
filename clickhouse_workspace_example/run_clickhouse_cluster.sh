#!/usr/bin/env bash
set -euo pipefail

# Build and start a 4-node ClickHouse cluster (two shards, two replicas each).
# Includes built-in ClickHouse Keeper for replication coordination.
# Defaults align with local samples; override with env vars if needed.
#
# Cluster topology:
#   - Shard 1: ch1 (replica 1), ch2 (replica 2)
#   - Shard 2: ch3 (replica 1), ch4 (replica 2)
#
# Environment overrides:
#   IMAGE_NAME            - docker image tag (default: clickhouse-cluster)
#   NETWORK_NAME          - docker network (default: clickhouse-cluster-net)
#   CLICKHOUSE_PASSWORD   - ClickHouse password (default: changeme)
#   CLICKHOUSE_USER       - ClickHouse user (default: default)
#   CLICKHOUSE_DB         - default DB (default: test)
#   NODE1_HTTP_PORT       - host HTTP port for ch1 (default: 8123)
#   NODE2_HTTP_PORT       - host HTTP port for ch2 (default: 8124)
#   NODE3_HTTP_PORT       - host HTTP port for ch3 (default: 8125)
#   NODE4_HTTP_PORT       - host HTTP port for ch4 (default: 8126)
#   NODE1_HTTPS_PORT      - host HTTPS port for ch1 (default: 8443)
#   NODE2_HTTPS_PORT      - host HTTPS port for ch2 (default: 8444)
#   NODE3_HTTPS_PORT      - host HTTPS port for ch3 (default: 8445)
#   NODE4_HTTPS_PORT      - host HTTPS port for ch4 (default: 8446)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IMAGE_NAME="${IMAGE_NAME:-clickhouse-cluster}"
NETWORK_NAME="${NETWORK_NAME:-clickhouse-cluster-net}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-changeme}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_DB="${CLICKHOUSE_DB:-test}"
SKIP_BUILD="${SKIP_BUILD:-0}"
NO_CACHE="${NO_CACHE:-1}"

# HTTP ports for each node (exposed to host)
NODE1_HTTP_PORT="${NODE1_HTTP_PORT:-8123}"
NODE2_HTTP_PORT="${NODE2_HTTP_PORT:-8124}"
NODE3_HTTP_PORT="${NODE3_HTTP_PORT:-8125}"
NODE4_HTTP_PORT="${NODE4_HTTP_PORT:-8126}"
# HTTPS ports for each node (exposed to host)
NODE1_HTTPS_PORT="${NODE1_HTTPS_PORT:-8443}"
NODE2_HTTPS_PORT="${NODE2_HTTPS_PORT:-8444}"
NODE3_HTTPS_PORT="${NODE3_HTTPS_PORT:-8445}"
NODE4_HTTPS_PORT="${NODE4_HTTPS_PORT:-8446}"

if [ "${SKIP_BUILD:-0}" = "1" ]; then
  echo "Skipping image build (SKIP_BUILD=1). Using existing image: ${IMAGE_NAME}"
else
  echo "Building ${IMAGE_NAME} from Dockerfile.clickhouse.cluster..."
  build_flags=()
  [ "${NO_PULL:-0}" != "1" ] && build_flags+=(--pull)
  [ "${NO_CACHE}" = "1" ] && build_flags+=(--no-cache)
  docker build "${build_flags[@]}" -f "${ROOT_DIR}/Dockerfile.clickhouse.cluster" -t "${IMAGE_NAME}" "${ROOT_DIR}"
fi

if ! docker network ls --format '{{.Name}}' | grep -qx "${NETWORK_NAME}"; then
  echo "Creating docker network ${NETWORK_NAME}..."
  docker network create "${NETWORK_NAME}"
fi

start_node() {
  local name="$1"
  local shard="$2"
  local replica="$3"
  local http_port="$4"
  local https_port="$5"
  local enable_keeper="$6"
  local keeper_id="$7"

  if docker ps -a --format '{{.Names}}' | grep -Eq "^${name}\$"; then
    echo "Container ${name} already exists. Removing it to apply fresh config..."
    docker rm -f "${name}" >/dev/null
  fi

  local keeper_status="disabled"
  if [ "$enable_keeper" = "true" ]; then
    keeper_status="enabled (id=${keeper_id})"
  fi
  
  echo "Starting ${name} (shard=${shard}, replica=${replica}, keeper=${keeper_status})..."
  docker run -d \
    --name "${name}" \
    --hostname "${name}" \
    --network "${NETWORK_NAME}" \
    -p "${http_port}:8123" \
    -p "${https_port}:8443" \
    -e CLICKHOUSE_USER="${CLICKHOUSE_USER}" \
    -e CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD}" \
    -e CLICKHOUSE_DB="${CLICKHOUSE_DB}" \
    -e CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1 \
    -e CLICKHOUSE_SHARD="${shard}" \
    -e CLICKHOUSE_REPLICA="${replica}" \
    -e ENABLE_KEEPER="${enable_keeper}" \
    -e KEEPER_SERVER_ID="${keeper_id}" \
    "${IMAGE_NAME}"
}

# Start all 4 nodes
# Shard 1: ch1 (replica 1), ch2 (replica 2)
# Shard 2: ch3 (replica 1), ch4 (replica 2)
# Only first 3 nodes run Keeper (quorum requires odd number of nodes)
start_node ch1 1 1 "${NODE1_HTTP_PORT}" "${NODE1_HTTPS_PORT}" true 1
start_node ch2 1 2 "${NODE2_HTTP_PORT}" "${NODE2_HTTPS_PORT}" true 2
start_node ch3 2 1 "${NODE3_HTTP_PORT}" "${NODE3_HTTPS_PORT}" true 3
start_node ch4 2 2 "${NODE4_HTTP_PORT}" "${NODE4_HTTPS_PORT}" false 0

echo ""
echo "========================================="
echo "Cluster is starting with replication enabled"
echo "========================================="
echo "Shard 1 (replicas):"
echo "  - ch1 HTTP: http://localhost:${NODE1_HTTP_PORT} [Keeper enabled]"
echo "  - ch1 HTTPS: https://localhost:${NODE1_HTTPS_PORT} [Keeper enabled]"
echo "  - ch2 HTTP: http://localhost:${NODE2_HTTP_PORT} [Keeper enabled]"
echo "  - ch2 HTTPS: https://localhost:${NODE2_HTTPS_PORT} [Keeper enabled]"
echo ""
echo "Shard 2 (replicas):"
echo "  - ch3 HTTP: http://localhost:${NODE3_HTTP_PORT} [Keeper enabled]"
echo "  - ch3 HTTPS: https://localhost:${NODE3_HTTPS_PORT} [Keeper enabled]"
echo "  - ch4 HTTP: http://localhost:${NODE4_HTTP_PORT}"
echo "  - ch4 HTTPS: https://localhost:${NODE4_HTTPS_PORT}"
echo ""
echo "Note: Only ch1, ch2, ch3 run ClickHouse Keeper (requires 3-node quorum)"
echo ""
echo "Wait 15-30 seconds for Keeper to establish quorum, then check status:"
echo "  docker exec -it ch1 clickhouse-client --user=${CLICKHOUSE_USER} --password=${CLICKHOUSE_PASSWORD} --query=\"SELECT * FROM system.zookeeper WHERE path='/'\""
echo ""
echo "To connect with clickhouse-client:"
echo "  docker exec -it ch1 clickhouse-client --user=${CLICKHOUSE_USER} --password=${CLICKHOUSE_PASSWORD}"
echo ""
echo "To check logs:"
echo "  docker logs -f ch1"
echo "  docker logs -f ch2"
echo "  docker logs -f ch3"
echo "  docker logs -f ch4"
echo ""
echo "Run the Rust example:"
echo "  export CH_NODES=\"https://localhost:${NODE1_HTTPS_PORT},https://localhost:${NODE2_HTTPS_PORT},https://localhost:${NODE3_HTTPS_PORT},https://localhost:${NODE4_HTTPS_PORT}\""
echo "  export CH_CA_CERT=\"${ROOT_DIR}/tls/ca.crt\""
echo "  cargo run --bin clickhouse_cluster_example"

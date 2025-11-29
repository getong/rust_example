#!/usr/bin/env bash
set -euo pipefail

# Build and run the ClickHouse image defined in Dockerfile.clickhouse.
# Environment overrides:
#   IMAGE_NAME            - docker image tag (default: clickhouse-local)
#   CONTAINER_NAME        - container name (default: clickhouse-local)
#   HTTP_PORT             - host port for HTTP interface (default: 8123)
#   TCP_PORT              - host port for native TCP interface (default: 9000)
#   INTERSERVER_PORT      - host port for interserver/replication (default: 9009)
#   CLICKHOUSE_USER       - ClickHouse user (default: default)
#   CLICKHOUSE_PASSWORD   - ClickHouse password (default: changeme)
#   CLICKHOUSE_DB         - default database (default: test)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IMAGE_NAME="${IMAGE_NAME:-clickhouse-local}"
CONTAINER_NAME="${CONTAINER_NAME:-clickhouse-local}"
HTTP_PORT="${HTTP_PORT:-8123}"
TCP_PORT="${TCP_PORT:-9000}"
INTERSERVER_PORT="${INTERSERVER_PORT:-9009}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-changeme}"
CLICKHOUSE_DB="${CLICKHOUSE_DB:-test}"
SKIP_BUILD="${SKIP_BUILD:-1}"

echo "Building ${IMAGE_NAME} from Dockerfile.clickhouse..."
docker build -f "${ROOT_DIR}/Dockerfile.clickhouse" -t "${IMAGE_NAME}" "${ROOT_DIR}"

if docker ps -a --format '{{.Names}}' | grep -Eq "^${CONTAINER_NAME}\$"; then
  echo "Container ${CONTAINER_NAME} already exists. Remove it or set CONTAINER_NAME to a new value."
  exit 1
fi

echo "Starting container ${CONTAINER_NAME} (HTTP:${HTTP_PORT}, TCP:${TCP_PORT}, Interserver:${INTERSERVER_PORT})..."
docker run -d \
  --name "${CONTAINER_NAME}" \
  -p "${HTTP_PORT}:8123" \
  -p "${TCP_PORT}:9000" \
  -p "${INTERSERVER_PORT}:9009" \
  -e CLICKHOUSE_USER="${CLICKHOUSE_USER}" \
  -e CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD}" \
  -e CLICKHOUSE_DB="${CLICKHOUSE_DB}" \
  -e CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1 \
  "${IMAGE_NAME}"

echo "ClickHouse is starting. Check logs with: docker logs -f ${CONTAINER_NAME}"

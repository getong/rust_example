#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IMAGE_NAME="${IMAGE_NAME:-tidb-playground:local}"
CONTAINER_NAME="${CONTAINER_NAME:-tidb-test}"
PORT_SQL="${PORT_SQL:-4000}"
PORT_PD="${PORT_PD:-2379}"
PORT_MONITOR="${PORT_MONITOR:-3000}"

echo "[tidb] building image '${IMAGE_NAME}' ..."
docker build -t "${IMAGE_NAME}" "${SCRIPT_DIR}"

if docker ps -a --format '{{.Names}}' | grep -w "${CONTAINER_NAME}" >/dev/null 2>&1; then
  echo "[tidb] removing existing container '${CONTAINER_NAME}' ..."
  docker rm -f "${CONTAINER_NAME}" >/dev/null
fi

echo "[tidb] starting container '${CONTAINER_NAME}' ..."
docker run -d \
  --name "${CONTAINER_NAME}" \
  -p "${PORT_SQL}:4000" \
  -p "${PORT_PD}:2379" \
  -p "${PORT_MONITOR}:3000" \
  -e TIDB_VERSION="${TIDB_VERSION:-v7.1.1}" \
  -e TIDB_HOST="${TIDB_HOST:-0.0.0.0}" \
  -e TIDB_TAG="${TIDB_TAG:-test-cluster}" \
  -e DB_HOST="${DB_HOST:-127.0.0.1}" \
  -e DB_PORT="${DB_PORT:-4000}" \
  -e DB_USER="${DB_USER:-root}" \
  -e DB_NAME="${DB_NAME:-test}" \
  "${IMAGE_NAME}"

echo "[tidb] container '${CONTAINER_NAME}' is running"
echo "SQL:    localhost:${PORT_SQL}"
echo "PD UI:  http://localhost:${PORT_PD}/dashboard"

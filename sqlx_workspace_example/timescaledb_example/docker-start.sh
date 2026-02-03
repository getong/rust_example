#!/usr/bin/env bash
set -euo pipefail

IMAGE="timescale/timescaledb:2.25.0-pg18"
CONTAINER_NAME="ts-container"
POSTGRES_USER="${POSTGRES_USER:-postgres}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-postgres}"
POSTGRES_DB="${POSTGRES_DB:-tsdb}"

docker run --rm --name "${CONTAINER_NAME}" \
  -e POSTGRES_USER="${POSTGRES_USER}" \
  -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD}" \
  -e POSTGRES_DB="${POSTGRES_DB}" \
  -p 5432:5432 \
  --memory=512M \
  -d \
  --cpus="0.125" \
  "${IMAGE}"

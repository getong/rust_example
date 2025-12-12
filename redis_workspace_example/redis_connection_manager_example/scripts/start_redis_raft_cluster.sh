#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-redis:8.4-alpine}"
NETWORK_NAME="${NETWORK_NAME:-redis-raft-net}"
REDIS_PASSWORD="${REDIS_PASSWORD:-abc123}"

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required to run this script." >&2
    exit 1
  fi
}

require_docker

if ! docker network inspect "${NETWORK_NAME}" >/dev/null 2>&1; then
  docker network create "${NETWORK_NAME}"
fi

run_node() {
  local id="$1" port="$2" raft_port="$3"
  local name="redis-raft-${id}"

  docker rm -f "${name}" >/dev/null 2>&1 || true

  docker run -d \
    --name "${name}" \
    --network "${NETWORK_NAME}" \
    -p "${port}:${port}" \
    -p "${raft_port}:${raft_port}" \
    -e NODE_ID="${id}" \
    -e REDIS_PORT="${port}" \
    -e RAFT_PORT="${raft_port}" \
    -e REDIS_PASSWORD="${REDIS_PASSWORD}" \
    "${IMAGE_NAME}"
}

wait_for_node() {
  local name="$1" port="$2"
  local attempt
  for attempt in $(seq 1 30); do
    if docker exec "${name}" redis-cli -a "${REDIS_PASSWORD}" -p "${port}" PING >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "Node ${name} on port ${port} did not become ready." >&2
  exit 1
}

run_node 1 6379 5001
run_node 2 6380 5002
run_node 3 6381 5003

wait_for_node "redis-raft-1" 6379
wait_for_node "redis-raft-2" 6380
wait_for_node "redis-raft-3" 6381

docker exec redis-raft-1 redis-cli -a "${REDIS_PASSWORD}" -p 6379 RAFT.CLUSTER CREATE
docker exec redis-raft-2 redis-cli -a "${REDIS_PASSWORD}" -p 6380 RAFT.CLUSTER JOIN redis-raft-1:6379
docker exec redis-raft-3 redis-cli -a "${REDIS_PASSWORD}" -p 6381 RAFT.CLUSTER JOIN redis-raft-1:6379

echo "Redis Raft cluster is up."
echo "Use: redis-cli -a ${REDIS_PASSWORD} -p 6379 INFO"

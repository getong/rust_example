#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-redis:8.4-alpine}"
NETWORK_NAME="${NETWORK_NAME:-redis-raft-net}"
REDIS_PASSWORD="${REDIS_PASSWORD:-abc123}"
ENABLE_REDISRAFT_MODULE="${ENABLE_REDISRAFT_MODULE:-0}" # 1 to load redisraft.so manually; 0 assumes RAFT is built-in to the image
REDISRAFT_SO="${REDISRAFT_SO:-./redisraft.so}"
CONTAINER_MODULE_PATH="${CONTAINER_MODULE_PATH:-/opt/redisraft/redisraft.so}"
RAFT_PORT_BASE="${RAFT_PORT_BASE:-5001}"
PUBLISH_RAFT_PORTS="${PUBLISH_RAFT_PORTS:-0}" # Set to 1 to publish Raft ports to host
USE_IMAGE_REDISRAFT="${USE_IMAGE_REDISRAFT:-0}" # Set to 1 if the image already bundles redisraft.so
MODULE_MOUNT_ARGS=()

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required to run this script." >&2
    exit 1
  fi
}

require_docker

echo "Using image: ${IMAGE_NAME}"
if [ "${IMAGE_NAME}" != "redis:8.4-alpine" ]; then
  echo "Note: IMAGE_NAME is overridden from the default. If RAFT commands are missing, try IMAGE_NAME=redis:8.4-alpine or ENABLE_REDISRAFT_MODULE=1." >&2
fi

# Ensure redisraft.so is available (only when we explicitly want to load the module).
if [ "${ENABLE_REDISRAFT_MODULE}" = "1" ]; then
  if [ "${USE_IMAGE_REDISRAFT}" != "1" ]; then
    if [ ! -f "${REDISRAFT_SO}" ]; then
      echo "redisraft.so not found at ${REDISRAFT_SO}." >&2
      echo "Provide a compiled redisraft.so (or set USE_IMAGE_REDISRAFT=1 with an image that bundles the module)." >&2
      exit 1
    fi

    HOST_REDISRAFT_SO="$(cd "$(dirname "${REDISRAFT_SO}")" && pwd)/$(basename "${REDISRAFT_SO}")"
    MODULE_MOUNT_ARGS=(-v "${HOST_REDISRAFT_SO}:${CONTAINER_MODULE_PATH}:ro")
  else
    echo "Assuming ${IMAGE_NAME} already bundles redisraft.so at ${CONTAINER_MODULE_PATH} (USE_IMAGE_REDISRAFT=1)." >&2
  fi
fi

# Ensure the image is available locally. Network pulls may be blocked
# in this environment, so bail out with a clear instruction if it's missing.
if ! docker image inspect "${IMAGE_NAME}" >/dev/null 2>&1; then
  echo "Image ${IMAGE_NAME} not available locally. Pull it (or load from a tar) or set IMAGE_NAME=redis:8.4-alpine to use the built-in RAFT image you mentioned." >&2
  exit 1
fi

if ! docker network inspect "${NETWORK_NAME}" >/dev/null 2>&1; then
  docker network create "${NETWORK_NAME}"
fi

run_node() {
  local id="$1" host_port="$2"
  local raft_port=$((RAFT_PORT_BASE + id - 1))
  local name="redis-raft-${id}"
  local raft_opts=""

  if [ "${ENABLE_REDISRAFT_MODULE}" = "1" ]; then
    raft_opts=" --loadmodule ${CONTAINER_MODULE_PATH} raft-log-fsync no raft-addr ${name}:${raft_port}"
  fi

  local args=(
    docker run -d
    --name "${name}"
    --network "${NETWORK_NAME}"
    -p "${host_port}:6379"
  )

  if [ "${ENABLE_REDISRAFT_MODULE}" = "1" ] && [ "${PUBLISH_RAFT_PORTS}" = "1" ]; then
    args+=(-p "${raft_port}:${raft_port}")
  fi

  if [ "${#MODULE_MOUNT_ARGS[@]}" -gt 0 ]; then
    args+=("${MODULE_MOUNT_ARGS[@]}")
  fi

  docker rm -f "${name}" >/dev/null 2>&1 || true

  args+=(
    "${IMAGE_NAME}"
    sh -c
    "mkdir -p /data/node${id} && redis-server \
      --port 6379 \
      --bind 0.0.0.0 \
      --protected-mode no \
      --requirepass ${REDIS_PASSWORD} \
      --dir /data/node${id} \
      --pidfile /data/node${id}/redis.pid \
      --logfile /data/node${id}/redis.log${raft_opts}"
  )

  "${args[@]}"
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

ensure_raft_available() {
  local name="$1" port="$2"
  local output

  if ! output=$(docker exec "${name}" redis-cli -a "${REDIS_PASSWORD}" -p "${port}" COMMAND INFO RAFT.CLUSTER 2>&1); then
    echo "Failed to query RAFT command info on ${name} (image ${IMAGE_NAME}). Output: ${output}" >&2
    exit 1
  fi

  if printf "%s" "${output}" | grep -qiE "unknown command|not allowed"; then
    echo "RAFT commands not available on ${name} (image ${IMAGE_NAME})." >&2
    echo "Official redis images (including redis:8.4-alpine) do NOT bundle RedisRaft; use an image that has it (e.g. redislabs/redisraft) or run with ENABLE_REDISRAFT_MODULE=1 and a redisraft.so." >&2
    exit 1
  fi

  if ! printf "%s" "${output}" | grep -q "RAFT.CLUSTER"; then
    echo "RAFT command info did not include RAFT.CLUSTER on ${name}. Output: ${output}" >&2
    echo "This usually means the image (${IMAGE_NAME}) lacks RedisRaft; switch IMAGE_NAME to a RedisRaft-enabled image or set ENABLE_REDISRAFT_MODULE=1." >&2
    exit 1
  fi
}

run_node 1 6379
run_node 2 6380
run_node 3 6381

wait_for_node "redis-raft-1" 6379
wait_for_node "redis-raft-2" 6379
wait_for_node "redis-raft-3" 6379

ensure_raft_available "redis-raft-1" 6379

docker exec redis-raft-1 redis-cli -a "${REDIS_PASSWORD}" -p 6379 RAFT.CLUSTER CREATE
docker exec redis-raft-2 redis-cli -a "${REDIS_PASSWORD}" -p 6379 RAFT.CLUSTER JOIN redis-raft-1:6379
docker exec redis-raft-3 redis-cli -a "${REDIS_PASSWORD}" -p 6379 RAFT.CLUSTER JOIN redis-raft-1:6379

echo "Redis Raft cluster is up."
echo "Use: redis-cli -a ${REDIS_PASSWORD} -p 6379 INFO"

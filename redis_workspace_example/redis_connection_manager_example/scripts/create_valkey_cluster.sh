#!/usr/bin/env bash
set -euo pipefail

# Build/start a Valkey cluster locally with ports mapped to the host.
# Defaults:
#   - Image: valkey-cluster (built from docker/valkey-cluster.Dockerfile)
#   - Network: valkey-net
#   - 6 nodes (3 masters, 3 replicas), base host port 7000 -> 7001..7006 and bus 17001..17006
#   - Password: abc123
#
# Override with environment variables:
#   VALKEY_IMAGE, VALKEY_NETWORK, BASE_PORT, NODE_COUNT, CLUSTER_REPLICAS, VALKEY_PASSWORD, CLUSTER_HOST_IP

IMAGE="${VALKEY_IMAGE:-valkey-cluster}"
NETWORK="${VALKEY_NETWORK:-valkey-net}"
BASE_PORT="${BASE_PORT:-7000}"
NODE_COUNT="${NODE_COUNT:-6}"
REPLICAS="${CLUSTER_REPLICAS:-1}"
PASSWORD="${VALKEY_PASSWORD:-abc123}"
HOST_IP="${CLUSTER_HOST_IP:-}"

resolve_host_ip() {
  if [ -n "${HOST_IP}" ]; then
    echo "${HOST_IP}"
    return 0
  fi

  if command -v getent >/dev/null 2>&1 && getent hosts host.docker.internal >/dev/null 2>&1; then
    echo "host.docker.internal"
    return 0
  fi

  if command -v ip >/dev/null 2>&1; then
    local docker_ip
    docker_ip=$(ip -4 addr show docker0 2>/dev/null | awk '/inet /{print $2}' | cut -d/ -f1)
    if [ -n "${docker_ip}" ]; then
      echo "${docker_ip}"
      return 0
    fi
  fi

  echo "Set CLUSTER_HOST_IP to a host address reachable from containers (e.g. host.docker.internal or 172.17.0.1)" >&2
  exit 1
}

HOST_IP="$(resolve_host_ip)"

if ! docker network inspect "${NETWORK}" >/dev/null 2>&1; then
  docker network create "${NETWORK}"
fi

for i in $(seq 1 "${NODE_COUNT}"); do
  docker rm -f "valkey-${i}" >/dev/null 2>&1 || true
done

for i in $(seq 1 "${NODE_COUNT}"); do
  port=$((BASE_PORT + i))
  bus=$((port + 10000))
  docker run -d --name "valkey-${i}" --net "${NETWORK}" \
    -p "${port}:${port}" -p "${bus}:${bus}" \
    -e NODE_ID="${i}" \
    -e VALKEY_PORT="${port}" \
    -e VALKEY_PASSWORD="${PASSWORD}" \
    -e CLUSTER_ANNOUNCE_IP="${HOST_IP}" \
    -e CLUSTER_ANNOUNCE_PORT="${port}" \
    -e CLUSTER_ANNOUNCE_BUS_PORT="${bus}" \
    "${IMAGE}"
done

master_ports=()
node_uris=()
for i in $(seq 1 "${NODE_COUNT}"); do
  master_ports+=("${HOST_IP}:$((BASE_PORT + i))")
  node_uris+=("redis://:${PASSWORD}@${HOST_IP}:$((BASE_PORT + i))/")
done

echo "Waiting 3s for nodes to start..."
sleep 3

echo "Creating cluster with replicas=${REPLICAS} ..."
docker exec -i "valkey-1" valkey-cli -a "${PASSWORD}" --cluster create \
  "${master_ports[@]}" \
  --cluster-replicas "${REPLICAS}" --cluster-yes

echo "Cluster created. Sample client env:"
printf '  export VALKEY_NODES="%s"\n' "$(IFS=,; echo "${node_uris[*]}")"
echo "  cargo run"

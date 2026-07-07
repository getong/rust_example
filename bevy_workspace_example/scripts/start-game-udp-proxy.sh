#!/usr/bin/env bash
set -euo pipefail

name="${GAME_UDP_PROXY_NAME:-bevy-game-udp-proxy}"
network="${GAME_UDP_PROXY_NETWORK:-kind}"
image="${GAME_UDP_PROXY_IMAGE:-alpine/socat:latest}"
service="${GAME_UDP_PROXY_SERVICE:-bevy-game-server-local}"

node_port="${GAME_UDP_PROXY_NODE_PORT:-}"
if [[ -z "${node_port}" ]] && command -v kubectl >/dev/null 2>&1; then
  node_port="$(kubectl get "svc/${service}" -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || true)"
fi
node_port="${node_port:-30600}"

listen_port="${GAME_UDP_PROXY_PORT:-${node_port}}"
target="${GAME_UDP_PROXY_TARGET:-}"
if [[ -z "${target}" ]] && command -v kubectl >/dev/null 2>&1; then
  node_name="$(kubectl get nodes --no-headers 2>/dev/null | awk '$3 !~ /control-plane/ {print $1; exit}')"
  node_name="${node_name:-$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || true)}"
  if [[ -n "${node_name}" ]]; then
    node_ip="$(kubectl get "node/${node_name}" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null || true)"
    if [[ -n "${node_ip}" ]]; then
      target="${node_ip}:${node_port}"
    fi
  fi
fi
target="${target:-172.21.0.2:${node_port}}"

if docker ps -a --format '{{.Names}}' | grep -Fxq "${name}"; then
  docker rm -f "${name}" >/dev/null
fi

docker run -d \
  --name "${name}" \
  --network "${network}" \
  -p "127.0.0.1:${listen_port}:${listen_port}/udp" \
  "${image}" \
  -dd "UDP-LISTEN:${listen_port},fork,reuseaddr" "UDP:${target}"

echo "UDP proxy ${name}: 127.0.0.1:${listen_port} -> ${target} on docker network ${network}"

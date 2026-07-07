#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

addr="${GAME_SERVER_ADDR:-127.0.0.1:${GAME_UDP_PROXY_PORT:-30600}}"

if [[ "${START_GAME_UDP_PROXY:-true}" != "false" && "${addr}" == 127.0.0.1:* ]]; then
  if ! docker ps --format '{{.Names}}' | grep -Fxq "${GAME_UDP_PROXY_NAME:-bevy-game-udp-proxy}"; then
    "${repo_root}/scripts/start-game-udp-proxy.sh"
  fi
fi

if [[ "${GAME_CLIENT_PROBE:-true}" != "false" ]]; then
  GAME_SERVER_ADDR="${addr}" GAME_CLIENT_PROBE_TIMEOUT_SECONDS="${GAME_CLIENT_PROBE_TIMEOUT_SECONDS:-6}" cargo run -p game_client --bin net_probe
fi

exec env GAME_SERVER_ADDR="${addr}" cargo run -p game_client

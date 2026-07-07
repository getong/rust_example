#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

image="${GAME_SERVER_IMAGE:-game-server:dev-$(date +%Y%m%d%H%M%S)}"

GAME_SERVER_IMAGE="${image}" "${repo_root}/scripts/build-game-server-image.sh"
"${repo_root}/scripts/deploy-game-server-agones.sh"

if [[ "${START_GAME_UDP_PROXY:-true}" != "false" ]]; then
  "${repo_root}/scripts/start-game-udp-proxy.sh"
fi

echo "Probe:"
echo "  GAME_SERVER_ADDR=127.0.0.1:30600 cargo run -p game_client --bin net_probe"
echo "Client:"
echo "  GAME_SERVER_ADDR=127.0.0.1:30600 cargo run -p game_client"

if [[ "${RUN_GAME_CLIENT:-false}" == "true" ]]; then
  GAME_SERVER_ADDR="${GAME_SERVER_ADDR:-127.0.0.1:30600}" cargo run -p game_client
fi

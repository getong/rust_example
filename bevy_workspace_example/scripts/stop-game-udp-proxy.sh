#!/usr/bin/env bash
set -euo pipefail

name="${GAME_UDP_PROXY_NAME:-bevy-game-udp-proxy}"
docker rm -f "${name}" >/dev/null 2>&1 || true
echo "UDP proxy ${name} stopped"

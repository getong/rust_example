#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

image="${GAME_SERVER_IMAGE:-game-server:dev-$(date +%Y%m%d%H%M%S)}"
dockerfile="${GAME_SERVER_DOCKERFILE:-Dockerfile}"
manifest="${GAME_SERVER_FLEET_MANIFEST:-deploy/agones/game-server-fleet.yaml}"

command -v docker >/dev/null 2>&1 || {
  echo "docker is required" >&2
  exit 1
}

echo "Building ${image}"
docker build -t "${image}" -f "${dockerfile}" .

tmp="$(mktemp)"
awk -v image="${image}" '
  $1 == "image:" && changed == 0 {
    indent = substr($0, 1, index($0, "image:") - 1)
    print indent "image: " image
    changed = 1
    next
  }
  { print }
  END {
    if (changed == 0) {
      exit 42
    }
  }
' "${manifest}" > "${tmp}" || {
  rm -f "${tmp}"
  echo "failed to update image in ${manifest}" >&2
  exit 1
}
mv "${tmp}" "${manifest}"

echo "Updated ${manifest} to ${image}"
echo "${image}"

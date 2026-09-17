#!/usr/bin/env bash
set -euo pipefail

image='ghcr.io/helixdb/helixdb:v0.0.5'
container='helix-helix_db_example-v3'
volume='helix-helix_db_example-v3-data'
url='http://127.0.0.1:6969'

for dependency in docker curl; do
  command -v "$dependency" >/dev/null 2>&1 || {
    printf 'Missing required command: %s\n' "$dependency" >&2
    exit 1
  }
done
if ! docker info >/dev/null 2>&1; then
  printf 'Docker is unavailable. Start Docker Desktop and try again.\n' >&2
  exit 1
fi
if ! image_id=$(docker image inspect --format '{{.Id}}' "$image" 2>/dev/null); then
  printf 'Image missing. Run: docker pull %s\n' "$image" >&2
  exit 1
fi

if docker container inspect "$container" >/dev/null 2>&1; then
  if [[ "$(docker inspect --format '{{.Image}}' "$container")" != "$image_id" ]]; then
    printf 'Container %s uses a different image; resolve the name conflict first.\n' "$container" >&2
    exit 1
  fi
  if [[ "$(docker inspect --format '{{(index (index .HostConfig.PortBindings "8080/tcp") 0).HostIp}}:{{(index (index .HostConfig.PortBindings "8080/tcp") 0).HostPort}}' "$container")" != '127.0.0.1:6969' ]]; then
    printf 'Container %s has an unexpected port mapping.\n' "$container" >&2
    exit 1
  fi
  if [[ "$(docker inspect --format '{{.State.Running}}' "$container")" != true ]]; then
    docker start "$container" >/dev/null
  fi
else
  # The image owns /home/nonroot as UID 65532. Docker copies its ownership
  # into a new volume, allowing the non-root server to persist data.
  docker run --detach --pull=never --name "$container" \
    --publish 127.0.0.1:6969:8080 \
    --env HELIX_DATA_DIR=/home/nonroot \
    --mount "type=volume,source=$volume,target=/home/nonroot" \
    "$image" >/dev/null
fi

# Wait for the server to finish opening its database.
for ((attempt = 0; attempt < 60; attempt++)); do
  if [[ "$(docker inspect --format '{{.State.Running}}' "$container")" != true ]]; then
    break
  fi
  if curl --noproxy '*' --fail --silent --connect-timeout 1 --max-time 2 \
    "$url/readyz" >/dev/null; then
    printf 'HelixDB ready: %s\nRun the Rust CRUD example: cargo run\n' "$url"
    printf 'Stop: docker stop %s\nLogs: docker logs --tail 100 %s\n' "$container" "$container"
    printf 'Persistent data volume: %s\n' "$volume"
    exit 0
  fi
  sleep 1
done

printf 'HelixDB failed to become ready. Recent server logs:\n' >&2
docker logs --tail 50 "$container" >&2
exit 1

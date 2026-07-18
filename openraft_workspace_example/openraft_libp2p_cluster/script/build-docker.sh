#!/usr/bin/env bash
# Build the openraft_libp2p_cluster docker image.
#
#   ./script/build-docker.sh                 # openraft-libp2p-cluster:latest
#   IMAGE=myrepo/olpc TAG=v1 ./script/build-docker.sh
#   PUSH=1 IMAGE=myrepo/olpc ./script/build-docker.sh   # also docker push
#
# The build context is the crate root; .dockerignore keeps it lean.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

IMAGE="${IMAGE:-openraft-libp2p-cluster}"
TAG="${TAG:-0.1}"
# Compile parallelism inside the builder; bounded so the build stays within
# the Docker Desktop VM's memory (see the CARGO_JOBS comment in Dockerfile).
CARGO_JOBS="${CARGO_JOBS:-4}"

# Host cargo cache handed to the build as named contexts: the Dockerfile seeds
# its cache mounts from these, so crates.io packages and git deps (the
# rust-libp2p GitHub clone) are copied from the host instead of re-downloaded.
# Only index/cache/db are shared — registry/src and git/checkouts are big
# (10GB+) and cargo rebuilds them locally from the shared parts.
CARGO_HOME_DIR="${CARGO_HOME_DIR:-$HOME/.cargo}"
mkdir -p "$CARGO_HOME_DIR/registry/index" "$CARGO_HOME_DIR/registry/cache" "$CARGO_HOME_DIR/git/db"

# The crate lives in a cargo workspace, so its Cargo.lock is one level up and
# outside the build context. Hand it to the build as its own tiny context:
# with a lock file cargo skips dependency re-resolution (a crates.io index
# round-trip on every build that stalls or fails behind slow egress).
WORKSPACE_LOCK="${WORKSPACE_LOCK:-$ROOT_DIR/../Cargo.lock}"
LOCK_CTX="$(mktemp -d -t olpc-cargo-lock)"
trap 'rm -rf "$LOCK_CTX"' EXIT
if [[ -f "$WORKSPACE_LOCK" ]]; then
	cp "$WORKSPACE_LOCK" "$LOCK_CTX/Cargo.lock"
else
	echo "Warning: $WORKSPACE_LOCK not found; cargo will resolve deps online." >&2
fi

# Cache mounts in the Dockerfile require BuildKit (default since docker 23,
# forced here for older daemons).
export DOCKER_BUILDKIT=1

echo "Building ${IMAGE}:${TAG} (context: $ROOT_DIR, cargo -j${CARGO_JOBS})"
docker build \
	-f "$ROOT_DIR/Dockerfile" \
	--build-arg CARGO_JOBS="$CARGO_JOBS" \
	--build-context cargo-registry-index="$CARGO_HOME_DIR/registry/index" \
	--build-context cargo-registry-cache="$CARGO_HOME_DIR/registry/cache" \
	--build-context cargo-git-db="$CARGO_HOME_DIR/git/db" \
	--build-context cargo-lock="$LOCK_CTX" \
	-t "${IMAGE}:${TAG}" \
	"$ROOT_DIR"

if [[ "${PUSH:-0}" == "1" ]]; then
	docker push "${IMAGE}:${TAG}"
fi

echo "Done: ${IMAGE}:${TAG}"

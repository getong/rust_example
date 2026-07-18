#!/usr/bin/env bash
# Container entrypoint: derive the node identity and CLI flags from env vars.
#
# Every node runs the same image and the same entrypoint; a node becomes the
# bootstrap node simply because its own peer id equals the bootstrap peer id
# (the binary bootstraps OpenRaft locally when --id matches --bootstrap-node).
#
# Env:
#   DB_DIR                  data dir (default /data)
#   P2P_PORT / HTTP_PORT    listen ports (default 4001 / 3000)
#   ADVERTISE_HOST          DNS name other nodes dial (default $HOSTNAME;
#                           in k8s set to the pod FQDN)
#   BOOTSTRAP               full "<peerid>=<multiaddr>" bootstrap spec, or
#   BOOTSTRAP_PEER_ID + BOOTSTRAP_HOST [+ BOOTSTRAP_P2P_PORT]
#                           composed into /dns4/<host>/tcp/<port>/ws/p2p/<id>.
#                           If neither is given the node bootstraps on itself.
#   BOOTSTRAP_KEY_FILE      read-only libp2p key (docker bind mount / k8s
#                           secret) copied to $DB_DIR/node.key on first start
#   MAX_CONTROL_NODES       openraft voter cap (default 5)
#   REDIS_URL               enables the redis-backed sqlite cache; unset or
#                           DISABLE_SQLITE_CACHE=1 disables it
#   RAFT_KEEPALIVE_MS, RAFT_ELECTION_TIMEOUT_MIN_MS,
#   RAFT_ELECTION_TIMEOUT_MAX_MS, VOTER_REPLACE_TIMEOUT_SECS,
#   AUTO_HEAL_MEMBERSHIP    optional raft/membership knobs
#   TOKIO_CONSOLE=1         keep tokio-console instrumentation on
#   EXTRA_ARGS              extra flags appended verbatim (word-split)
set -euo pipefail

DB_DIR="${DB_DIR:-/data}"
P2P_PORT="${P2P_PORT:-4001}"
HTTP_PORT="${HTTP_PORT:-3000}"
mkdir -p "$DB_DIR"

if [[ -n "${BOOTSTRAP_KEY_FILE:-}" && -s "${BOOTSTRAP_KEY_FILE}" && ! -s "$DB_DIR/node.key" ]]; then
  cp "$BOOTSTRAP_KEY_FILE" "$DB_DIR/node.key"
fi

PEER_ID="$(olpc-peer-id --key "$DB_DIR/node.key" --create | tail -1)"
if [[ -z "$PEER_ID" ]]; then
  echo "Error: failed to generate/load peer id from $DB_DIR/node.key" >&2
  exit 1
fi

ADVERTISE_HOST="${ADVERTISE_HOST:-${HOSTNAME:-$(hostname)}}"
LISTEN="${LISTEN:-/ip4/0.0.0.0/tcp/${P2P_PORT}/ws}"
ADVERTISE="${ADVERTISE:-/dns4/${ADVERTISE_HOST}/tcp/${P2P_PORT}/ws/p2p/${PEER_ID}}"

if [[ -z "${BOOTSTRAP:-}" ]]; then
  if [[ -n "${BOOTSTRAP_PEER_ID:-}" && -n "${BOOTSTRAP_HOST:-}" ]]; then
    BOOTSTRAP="${BOOTSTRAP_PEER_ID}=/dns4/${BOOTSTRAP_HOST}/tcp/${BOOTSTRAP_P2P_PORT:-$P2P_PORT}/ws/p2p/${BOOTSTRAP_PEER_ID}"
  else
    echo "No bootstrap configured; this node bootstraps OpenRaft on itself."
    BOOTSTRAP="${PEER_ID}=${ADVERTISE}"
  fi
fi

args=(
  --id "$PEER_ID"
  --listen "$LISTEN"
  --http "0.0.0.0:${HTTP_PORT}"
  --db "$DB_DIR"
  --key "$DB_DIR/node.key"
  --bootstrap-node "$BOOTSTRAP"
  --advertise "$ADVERTISE"
  --max-control-nodes "${MAX_CONTROL_NODES:-5}"
)

# tokio-console instrumentation is per-poll overhead on every task; off by
# default in containers.
if [[ "${TOKIO_CONSOLE:-0}" != "1" ]]; then
  args+=(--no-tokio-console)
fi

if [[ -n "${REDIS_URL:-}" && "${DISABLE_SQLITE_CACHE:-0}" != "1" ]]; then
  args+=(--redis-url "$REDIS_URL")
else
  args+=(--disable-sqlite-cache)
fi

if [[ -n "${RAFT_KEEPALIVE_MS:-}" ]]; then
  args+=(--raft-keepalive-ms "$RAFT_KEEPALIVE_MS")
fi
if [[ -n "${RAFT_ELECTION_TIMEOUT_MIN_MS:-}" ]]; then
  args+=(--raft-election-timeout-min-ms "$RAFT_ELECTION_TIMEOUT_MIN_MS")
fi
if [[ -n "${RAFT_ELECTION_TIMEOUT_MAX_MS:-}" ]]; then
  args+=(--raft-election-timeout-max-ms "$RAFT_ELECTION_TIMEOUT_MAX_MS")
fi
if [[ -n "${VOTER_REPLACE_TIMEOUT_SECS:-}" ]]; then
  args+=(--voter-replace-timeout-secs "$VOTER_REPLACE_TIMEOUT_SECS")
fi
if [[ -n "${AUTO_HEAL_MEMBERSHIP:-}" ]]; then
  args+=(--auto-heal-membership "$AUTO_HEAL_MEMBERSHIP")
fi
if [[ -n "${MAX_PEER_CONNECTIONS:-}" ]]; then
  args+=(--max-peer-connections "$MAX_PEER_CONNECTIONS")
fi
if [[ -n "${OVERLAY_MIN_CONNECTIONS:-}" ]]; then
  args+=(--overlay-min-connections "$OVERLAY_MIN_CONNECTIONS")
fi
if [[ -n "${EXTRA_ARGS:-}" ]]; then
  read -r -a extra <<<"$EXTRA_ARGS"
  args+=("${extra[@]}")
fi

# Per-node wasm module store (the p2p wasm sync service distributes modules
# between these directories).
export WASM_MODULES_DIR="${WASM_MODULES_DIR:-$DB_DIR/wasm_modules}"
export LIBP2P_SELF_NAME="${LIBP2P_SELF_NAME:-$ADVERTISE_HOST}"
export RUST_LOG="${RUST_LOG:-warn,openraft_libp2p_cluster::wasm_sync=info}"

echo "peer_id=$PEER_ID"
echo "advertise=$ADVERTISE"
echo "bootstrap=$BOOTSTRAP"

exec openraft_libp2p_cluster "${args[@]}"

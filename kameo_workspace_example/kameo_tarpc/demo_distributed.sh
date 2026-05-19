#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="${ROOT_DIR}/.demo-logs"
TARGET_DIR="${ROOT_DIR}/target-local"
DATA_DIR="${ROOT_DIR}/data/actor-node"
ACTOR_SWARM_PORT="${ACTOR_SWARM_PORT:-47011}"
RPC_SWARM_PORT="${RPC_SWARM_PORT:-47012}"
RPC_PORT="${RPC_PORT:-47013}"
mkdir -p "${LOG_DIR}"
export RUST_LOG="${RUST_LOG:-warn,kameo_tarpc=info}"

ACTOR_LOG="${LOG_DIR}/actor-node.log"
RPC_LOG="${LOG_DIR}/rpc-server.log"

extract_total() {
  sed -nE 's/.* total=([0-9-]+) .*/\1/p' | tail -n1
}

cleanup() {
  if [[ -n "${RPC_PID:-}" ]]; then
    kill "${RPC_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${ACTOR_PID:-}" ]]; then
    kill "${ACTOR_PID}" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT

cd "${ROOT_DIR}"

rm -rf "${DATA_DIR}"
cargo build --target-dir "${TARGET_DIR}" >/dev/null

"${TARGET_DIR}/debug/kameo_tarpc" actor-node \
  --actor-name distributed-counter \
  --swarm-listen-addr "/ip4/127.0.0.1/tcp/${ACTOR_SWARM_PORT}" \
  --raft-db-path "${DATA_DIR}" \
  >"${ACTOR_LOG}" 2>&1 &
ACTOR_PID=$!

sleep 2

ACTOR_PEER_ID="$(grep -m1 'actor node ready peer_id=' "${ACTOR_LOG}" | sed -E 's/.*peer_id=([^ ]+).*/\1/')"
if [[ -z "${ACTOR_PEER_ID}" ]]; then
  echo "failed to determine actor peer id"
  exit 1
fi

SEED_ADDR="/ip4/127.0.0.1/tcp/${ACTOR_SWARM_PORT}/p2p/${ACTOR_PEER_ID}"

"${TARGET_DIR}/debug/kameo_tarpc" rpc-server \
  --actor-name distributed-counter \
  --rpc-listen-addr "127.0.0.1:${RPC_PORT}" \
  --seed "${SEED_ADDR}" \
  --swarm-listen-addr "/ip4/127.0.0.1/tcp/${RPC_SWARM_PORT}" \
  >"${RPC_LOG}" 2>&1 &
RPC_PID=$!

sleep 3

FIRST_CALL_OUTPUT="$("${TARGET_DIR}/debug/kameo_tarpc" rpc-client \
  --server-addr "127.0.0.1:${RPC_PORT}" \
  --amount 7 \
  --caller demo-run-1)"
printf '%s\n' "${FIRST_CALL_OUTPUT}"

SECOND_CALL_OUTPUT="$("${TARGET_DIR}/debug/kameo_tarpc" rpc-client \
  --server-addr "127.0.0.1:${RPC_PORT}" \
  --amount 7 \
  --caller demo-run-2)"
printf '%s\n' "${SECOND_CALL_OUTPUT}"

FINAL_TOTAL="$(printf '%s\n' "${SECOND_CALL_OUTPUT}" | extract_total)"
FINAL_ACTOR_LINE="$(grep 'counter actor handled' "${ACTOR_LOG}" | tail -n1 || true)"

echo
echo "final accumulated total: ${FINAL_TOTAL}"
if [[ -n "${FINAL_ACTOR_LINE}" ]]; then
  echo "actor final state: ${FINAL_ACTOR_LINE}"
fi
echo "actor log: ${ACTOR_LOG}"
echo "rpc log:   ${RPC_LOG}"

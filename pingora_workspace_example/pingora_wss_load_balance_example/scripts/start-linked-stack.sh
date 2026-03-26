#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
LOG_DIR="${PROJECT_DIR}/logs"
CARGO_TARGET_DIR="${PROJECT_DIR}/target"

TLS_MODE=true
RUN_TEST=true
LISTEN_ADDR="${WS_LISTEN_ADDR:-127.0.0.1:6188}"
UPSTREAM_1_ADDR="${WS_UPSTREAM_1_ADDR:-127.0.0.1:9001}"
UPSTREAM_2_ADDR="${WS_UPSTREAM_2_ADDR:-127.0.0.1:9002}"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/start-linked-stack.sh [--tls|--no-tls] [--test|--no-test]

Environment:
  WS_LISTEN_ADDR       default: 127.0.0.1:6188
  WS_UPSTREAM_1_ADDR   default: 127.0.0.1:9001
  WS_UPSTREAM_2_ADDR   default: 127.0.0.1:9002
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tls)
      TLS_MODE=true
      shift
      ;;
    --no-tls)
      TLS_MODE=false
      shift
      ;;
    --test)
      RUN_TEST=true
      shift
      ;;
    --no-test)
      RUN_TEST=false
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

mkdir -p "${LOG_DIR}"
mkdir -p "${CARGO_TARGET_DIR}"

PIDS=()

cleanup() {
  local code=$?
  set +e
  if [[ ${#PIDS[@]} -gt 0 ]]; then
    echo "stopping processes: ${PIDS[*]}"
    kill "${PIDS[@]}" >/dev/null 2>&1 || true
    wait "${PIDS[@]}" 2>/dev/null || true
  fi
  exit "${code}"
}
trap cleanup EXIT INT TERM

wait_for_port() {
  local host="$1"
  local port="$2"
  local label="$3"
  local retries="${4:-60}"

  for ((i = 1; i <= retries; i++)); do
    if (echo >"/dev/tcp/${host}/${port}") >/dev/null 2>&1; then
      echo "${label} is ready at ${host}:${port}"
      return 0
    fi
    sleep 0.2
  done

  echo "timeout waiting for ${label} at ${host}:${port}" >&2
  return 1
}

wait_for_tls_port() {
  local host="$1"
  local port="$2"
  local label="$3"
  local retries="${4:-60}"

  if ! command -v openssl >/dev/null 2>&1; then
    wait_for_port "${host}" "${port}" "${label}" "${retries}"
    return
  fi

  for ((i = 1; i <= retries; i++)); do
    if openssl s_client -connect "${host}:${port}" -servername localhost -brief </dev/null >/dev/null 2>&1; then
      echo "${label} is ready at ${host}:${port} (tls)"
      return 0
    fi
    sleep 0.2
  done

  echo "timeout waiting for ${label} at ${host}:${port} (tls)" >&2
  return 1
}

split_host_port() {
  local addr="$1"
  local host="${addr%:*}"
  local port="${addr##*:}"
  echo "${host}" "${port}"
}

cd "${PROJECT_DIR}"
export CARGO_TARGET_DIR

echo "building binaries..."
cargo build --bin mock_ws_upstream --bin pingora_wss_load_balance_example >/dev/null

echo "starting mock upstream #1: ${UPSTREAM_1_ADDR}"
"${CARGO_TARGET_DIR}/debug/mock_ws_upstream" --listen "${UPSTREAM_1_ADDR}" --name upstream-1 \
  >"${LOG_DIR}/upstream-1.log" 2>&1 &
PIDS+=("$!")

echo "starting mock upstream #2: ${UPSTREAM_2_ADDR}"
"${CARGO_TARGET_DIR}/debug/mock_ws_upstream" --listen "${UPSTREAM_2_ADDR}" --name upstream-2 \
  >"${LOG_DIR}/upstream-2.log" 2>&1 &
PIDS+=("$!")

read -r up1_host up1_port <<<"$(split_host_port "${UPSTREAM_1_ADDR}")"
read -r up2_host up2_port <<<"$(split_host_port "${UPSTREAM_2_ADDR}")"
wait_for_port "${up1_host}" "${up1_port}" "upstream-1"
wait_for_port "${up2_host}" "${up2_port}" "upstream-2"

export WS_UPSTREAMS="${UPSTREAM_1_ADDR},${UPSTREAM_2_ADDR}"
export WS_LISTEN_ADDR="${LISTEN_ADDR}"

if [[ "${TLS_MODE}" == "true" ]]; then
  export WS_DOWNSTREAM_TLS=true
  if [[ ! -f "certs/localhost-cert.pem" || ! -f "certs/localhost-key.pem" ]]; then
    echo "TLS cert/key missing, generating..."
    ./scripts/generate-dev-cert.sh
  fi
  echo "starting pingora wss load balancer at ${WS_LISTEN_ADDR}"
else
  export WS_DOWNSTREAM_TLS=false
  echo "starting pingora ws load balancer at ${WS_LISTEN_ADDR}"
fi

"${CARGO_TARGET_DIR}/debug/pingora_wss_load_balance_example" >"${LOG_DIR}/pingora-wss-lb.log" 2>&1 &
PIDS+=("$!")

read -r lb_host lb_port <<<"$(split_host_port "${WS_LISTEN_ADDR}")"
if [[ "${TLS_MODE}" == "true" ]]; then
  wait_for_tls_port "${lb_host}" "${lb_port}" "pingora-wss-lb"
else
  wait_for_port "${lb_host}" "${lb_port}" "pingora-wss-lb"
fi

if [[ "${RUN_TEST}" == "true" ]]; then
  if [[ "${TLS_MODE}" == "true" ]]; then
    if command -v wscat >/dev/null 2>&1 || command -v npx >/dev/null 2>&1; then
      echo "running connectivity smoke test..."
      ./scripts/test-wss-client.sh "wss://${WS_LISTEN_ADDR}" "certs/localhost-cert.pem" "ping-from-linked-stack"
    else
      echo "skip smoke test: neither wscat nor npx is available in PATH"
    fi
  else
    echo "skip smoke test in ws mode (test script validates wss)"
  fi
fi

echo
echo "linked stack is running."
echo "logs:"
echo "  ${LOG_DIR}/upstream-1.log"
echo "  ${LOG_DIR}/upstream-2.log"
echo "  ${LOG_DIR}/pingora-wss-lb.log"
echo
echo "press Ctrl+C to stop all processes."

wait "${PIDS[@]}"

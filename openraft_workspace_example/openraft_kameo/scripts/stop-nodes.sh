#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODES="${NODES:-3}"
BASE_PORT="${BASE_PORT:-21001}"
HOST="${HOST:-127.0.0.1}"
PID_DIR="${PID_DIR:-$ROOT_DIR/../target/openraft-kameo-pids}"
LABEL_PREFIX="${LABEL_PREFIX:-openraft-kameo}"

http_ready() {
  local url="$1"
  curl --connect-timeout 1 --max-time 3 -fsS -o /dev/null "$url/metrics" 2>/dev/null
}

listen_pids() {
  local port="$1"
  command -v lsof >/dev/null 2>&1 || return 0
  lsof -nP -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true
}

port_listening() {
  local port="$1"
  [[ -n "$(listen_pids "$port")" ]]
}

wait_until_stopped() {
  local url="$1"
  local port="$2"
  for ((attempt = 1; attempt <= 25; attempt++)); do
    if ! http_ready "$url" && ! port_listening "$port"; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}

for ((i = 1; i <= NODES; i++)); do
  pid_file="$PID_DIR/node-$i.pid"
  label_file="$PID_DIR/node-$i.label"
  port=$((BASE_PORT + i - 1))
  url="http://$HOST:$port"
  label="$LABEL_PREFIX-node-$i"
  if [[ -f "$label_file" ]]; then
    label="$(cat "$label_file")"
  fi

  pids=""
  if [[ -f "$pid_file" ]]; then
    pids="$(cat "$pid_file")"
  fi
  for listening_pid in $(listen_pids "$port"); do
    case " $pids " in
      *" $listening_pid "*) ;;
      *) pids="$pids $listening_pid" ;;
    esac
  done

  stopped_any=false

  if command -v launchctl >/dev/null 2>&1; then
    if launchctl remove "$label" >/dev/null 2>&1; then
      stopped_any=true
      echo "node $i: removed launchctl label=$label"
    fi
  fi

  if [[ -z "${pids// }" && "$stopped_any" == false ]]; then
    echo "node $i: no running pid found"
    rm -f "$pid_file" "$label_file"
    continue
  fi

  for pid in $pids; do
    if kill "$pid" 2>/dev/null; then
      stopped_any=true
      echo "node $i: sent stop to pid=$pid"
    else
      echo "node $i: could not stop pid=$pid"
    fi
  done

  if [[ "$stopped_any" == true ]]; then
    if wait_until_stopped "$url" "$port"; then
      echo "node $i: stopped"
    else
      echo "node $i: stop sent but $url still responds"
    fi
  fi

  rm -f "$pid_file" "$label_file"
done

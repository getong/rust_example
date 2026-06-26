#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODES="${NODES:-3}"
BASE_PORT="${BASE_PORT:-21001}"
HOST="${HOST:-127.0.0.1}"
PID_DIR="${PID_DIR:-$ROOT_DIR/../target/openraft-kameo-pids}"
LABEL_PREFIX="${LABEL_PREFIX:-openraft-kameo}"

metrics_ok() {
  local url="$1"
  curl --connect-timeout 1 --max-time 2 -fsS -o /dev/null "$url/metrics" 2>/dev/null
}

metrics_note() {
  local url="$1"
  if metrics_ok "$url"; then
    printf "metrics=ok"
  else
    printf "metrics=unreachable"
  fi
}

listen_pid() {
  local port="$1"
  command -v lsof >/dev/null 2>&1 || return 1
  lsof -nP -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | head -n 1
}

port_listening() {
  local port="$1"
  [[ -n "$(listen_pid "$port")" ]]
}

for ((i = 1; i <= NODES; i++)); do
  pid_file="$PID_DIR/node-$i.pid"
  label_file="$PID_DIR/node-$i.label"
  port=$((BASE_PORT + i - 1))
  url="http://$HOST:$port"
  listening_pid="$(listen_pid "$port" || true)"
  label="$LABEL_PREFIX-node-$i"
  if [[ -f "$label_file" ]]; then
    label="$(cat "$label_file")"
  fi
  label_note=""
  if [[ -f "$label_file" ]]; then
    label_note=" label=$label"
  fi

  if [[ ! -f "$pid_file" ]]; then
    if [[ -n "$listening_pid" ]]; then
      echo "node $i: running listen_pid=$listening_pid$label_note $url $(metrics_note "$url") but no pid file"
    elif metrics_ok "$url"; then
      echo "node $i: running$label_note $url metrics=ok but no pid file"
    else
      echo "node $i: stopped"
    fi
    continue
  fi

  pid="$(cat "$pid_file")"
  if [[ -n "$listening_pid" ]]; then
    if [[ -n "$listening_pid" && "$listening_pid" != "$pid" ]]; then
      echo "node $i: running pid_file=$pid listen_pid=$listening_pid$label_note $url $(metrics_note "$url")"
    else
      echo "node $i: running pid=$pid$label_note $url $(metrics_note "$url")"
    fi
  elif metrics_ok "$url"; then
    echo "node $i: running pid_file=$pid$label_note $url metrics=ok but no listening pid found"
  else
    echo "node $i: stopped stale_pid=$pid"
  fi
done

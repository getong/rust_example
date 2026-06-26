#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODES="${NODES:-3}"
BASE_PORT="${BASE_PORT:-21001}"
HOST="${HOST:-127.0.0.1}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/../target/openraft-kameo-logs}"
PID_DIR="${PID_DIR:-$ROOT_DIR/../target/openraft-kameo-pids}"
LABEL_PREFIX="${LABEL_PREFIX:-openraft-kameo}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/../target}"
BIN="$TARGET_DIR/debug/openraft_kameo"

mkdir -p "$LOG_DIR" "$PID_DIR"

cd "$ROOT_DIR"
cargo build -p openraft_kameo

http_ready() {
  local url="$1"
  local attempts="${2:-1}"

  for ((health_attempt = 1; health_attempt <= attempts; health_attempt++)); do
    if curl --connect-timeout 1 --max-time 3 -fsS -o /dev/null "$url/metrics" 2>/dev/null; then
      return 0
    fi
    sleep 0.2
  done

  return 1
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

wait_for_http() {
  local url="$1"
  local deadline="${STARTUP_TIMEOUT_SECS:-20}"

  for ((attempt = 1; attempt <= deadline * 5; attempt++)); do
    if http_ready "$url"; then
      return 0
    fi

    sleep 0.2
  done

  return 1
}

start_node() {
  local node_id="$1"
  local addr="$2"
  local pid_file="$3"
  local label_file="$4"
  local log_file="$5"
  local err_file="$6"
  local label="$7"

  if command -v launchctl >/dev/null 2>&1 && [[ "$(uname -s)" == "Darwin" ]]; then
    launchctl remove "$label" >/dev/null 2>&1 || true
    launchctl submit -l "$label" -o "$log_file" -e "$err_file" -- "$BIN" --id "$node_id" --http-addr "$addr"
    echo "$label" > "$label_file"
    rm -f "$pid_file"
    return 0
  fi

  rm -f "$label_file"
  if command -v setsid >/dev/null 2>&1; then
    nohup setsid "$BIN" --id "$node_id" --http-addr "$addr" >"$log_file" 2>"$err_file" </dev/null &
  else
    nohup "$BIN" --id "$node_id" --http-addr "$addr" >"$log_file" 2>"$err_file" </dev/null &
  fi
  echo "$!" > "$pid_file"
  disown 2>/dev/null || true
}

nodes_json="["
for ((i = 1; i <= NODES; i++)); do
  port=$((BASE_PORT + i - 1))
  addr="$HOST:$port"
  pid_file="$PID_DIR/node-$i.pid"
  label_file="$PID_DIR/node-$i.label"
  log_file="$LOG_DIR/node-$i.log"
  err_file="$LOG_DIR/node-$i.err.log"
  label="$LABEL_PREFIX-node-$i"

  if http_ready "http://$addr" 10; then
    echo "node $i is already responding at http://$addr"
    echo "stop it first: ./scripts/stop-nodes.sh"
    exit 1
  fi

  if port_listening "$port"; then
    echo "node $i port $port is already occupied but /metrics is not responding"
    echo "inspect the process with: lsof -nP -iTCP:$port -sTCP:LISTEN"
    exit 1
  fi

  if [[ -f "$pid_file" ]]; then
    rm -f "$pid_file"
  fi
  if [[ -f "$label_file" ]]; then
    rm -f "$label_file"
  fi

  : > "$log_file"
  : > "$err_file"
  start_node "$i" "$addr" "$pid_file" "$label_file" "$log_file" "$err_file" "$label"

  if ! wait_for_http "http://$addr"; then
    echo "node $i did not become ready; last log lines:"
    tail -n 40 "$log_file" || true
    if [[ -s "$err_file" ]]; then
      echo "last stderr lines:"
      tail -n 40 "$err_file" || true
    fi
    if [[ -f "$label_file" ]] && command -v launchctl >/dev/null 2>&1; then
      launchctl remove "$label" >/dev/null 2>&1 || true
    fi
    exit 1
  fi

  actual_pid="$(listen_pid "$port" || true)"
  if [[ -n "$actual_pid" ]]; then
    echo "$actual_pid" > "$pid_file"
  fi
  echo "started node $i at http://$addr pid=${actual_pid:-unknown} label=${label} log=$log_file err=$err_file"

  if [[ "$i" -gt 1 ]]; then
    nodes_json+=","
  fi
  nodes_json+="[$i,\"$addr\"]"
done
nodes_json+="]"

echo "initializing cluster through node 1"
curl -fsS \
  -H "content-type: application/json" \
  -X POST \
  "http://$HOST:$BASE_PORT/init" \
  -d "$nodes_json"
echo

echo "cluster nodes are running"
echo "status:"
for ((i = 1; i <= NODES; i++)); do
  pid_file="$PID_DIR/node-$i.pid"
  port=$((BASE_PORT + i - 1))
  if http_ready "http://$HOST:$port" 10; then
    if [[ -f "$pid_file" ]]; then
      echo "  node $i pid=$(cat "$pid_file") listening at http://$HOST:$port"
    else
      echo "  node $i listening at http://$HOST:$port"
    fi
  else
    echo "  node $i is not running; inspect $LOG_DIR/node-$i.log"
  fi
done
echo "write example:"
echo "  curl -s -H 'content-type: application/json' -X POST http://$HOST:$BASE_PORT/write -d '{\"key\":\"hello\",\"value\":\"world\"}'"
echo "read example:"
echo "  curl -s -H 'content-type: application/json' -X POST http://$HOST:$BASE_PORT/linearizable-read -d '\"hello\"'"
echo "metrics example:"
echo "  curl -s http://$HOST:$BASE_PORT/metrics"
echo "stop nodes:"
echo "  ./scripts/stop-nodes.sh"

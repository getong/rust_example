#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="${REDIS_CLUSTER_DIR:-$PROJECT_ROOT/.local/redis-cluster}"
BASE_PORT="${REDIS_CLUSTER_BASE_PORT:-7000}"
ACTION="${1:-start}"
command -v redis-cli >/dev/null || { printf 'redis-cli is required.\n' >&2; exit 1; }
if [[ ! "$BASE_PORT" =~ ^[1-9][0-9]{3,4}$ ]] || (( BASE_PORT < 1024 || BASE_PORT > 55530 )); then
  printf 'REDIS_CLUSTER_BASE_PORT must be between 1024 and 55530.\n' >&2
  exit 1
fi
mkdir -p "$DATA_DIR"
DATA_DIR="$(cd "$DATA_DIR" && pwd)"
PORTS=()
for ((index = 0; index < 6; index++)); do
  PORTS+=("$((BASE_PORT + index))")
done

is_running() {
  [[ "$(redis-cli -h 127.0.0.1 -p "$1" ping 2>/dev/null || true)" == PONG ]]
}

is_owned() {
  redis-cli -h 127.0.0.1 -p "$1" --raw INFO server |
    tr -d '\r' | grep -Fxq "config_file:$DATA_DIR/$1/redis.conf"
}

case "$ACTION" in
  stop)
    for port in "${PORTS[@]}"; do
      if is_running "$port"; then
        if ! is_owned "$port"; then
          printf 'Refusing to stop an unrelated server on port %s.\n' "$port" >&2
          exit 1
        fi
        redis-cli -h 127.0.0.1 -p "$port" shutdown
      fi
    done
    printf 'Cluster stopped; persisted data retained in %s\n' "$DATA_DIR"
    exit 0
    ;;
  status)
    for port in "${PORTS[@]}"; do
      printf '\nPort %s\n' "$port"
      redis-cli -h 127.0.0.1 -p "$port" cluster info
    done
    exit 0
    ;;
  start) ;;
  *) printf 'Usage: %s [start|stop|status]\n' "$0" >&2; exit 1 ;;
esac

command -v redis-server >/dev/null || { printf 'redis-server is required.\n' >&2; exit 1; }
for port in "${PORTS[@]}"; do
  if is_running "$port" && ! is_owned "$port"; then
    printf 'Port %s belongs to an unrelated Redis server; refusing to overwrite it.\n' "$port" >&2
    exit 1
  fi
done

for port in "${PORTS[@]}"; do
  if is_running "$port"; then
    continue
  fi
  node_dir="$DATA_DIR/$port"
  mkdir -p "$node_dir"
  cat > "$node_dir/redis.conf" <<EOF
bind 127.0.0.1
protected-mode yes
port $port
daemonize yes
pidfile "$node_dir/redis.pid"
logfile "$node_dir/redis.log"
dir "$node_dir"
cluster-enabled yes
cluster-config-file nodes.conf
cluster-node-timeout 5000
cluster-announce-ip 127.0.0.1
cluster-announce-port $port
cluster-announce-bus-port $((port + 10000))
appendonly yes
appendfsync always
maxmemory-policy noeviction
min-replicas-to-write 1
min-replicas-max-lag 10
EOF
  redis-server "$node_dir/redis.conf"
  for ((attempt = 0; attempt < 50; attempt++)); do
    if is_running "$port"; then break; fi
    sleep 0.1
  done
  if ! is_running "$port"; then
    printf 'Redis failed on port %s; inspect %s/redis.log.\n' "$port" "$node_dir" >&2
    exit 1
  fi
done

addresses=()
all_new=true
for port in "${PORTS[@]}"; do
  addresses+=("127.0.0.1:$port")
  info="$(redis-cli -h 127.0.0.1 -p "$port" cluster info | tr -d '\r')"
  if ! grep -Fxq 'cluster_known_nodes:1' <<< "$info" ||
     ! grep -Fxq 'cluster_slots_assigned:0' <<< "$info"; then
    all_new=false
  fi
done
if [[ "$all_new" == true ]]; then
  redis-cli --cluster create "${addresses[@]}" --cluster-replicas 1 --cluster-yes
fi

for ((attempt = 0; attempt < 60; attempt++)); do
  ready=true
  for port in "${PORTS[@]}"; do
    if ! redis-cli -h 127.0.0.1 -p "$port" cluster info |
         tr -d '\r' | grep -Fxq 'cluster_state:ok'; then
      ready=false
    fi
    replication="$(redis-cli -h 127.0.0.1 -p "$port" INFO replication | tr -d '\r')"
    if grep -Fxq 'role:master' <<< "$replication" &&
       ! grep -Eq '^slave[0-9]+:.*state=online,.*lag=[0-9]$' <<< "$replication"; then
      ready=false
    fi
  done
  if [[ "$ready" == true ]]; then
    replicas="$(redis-cli -h 127.0.0.1 -p "$BASE_PORT" cluster nodes | grep -c 'slave' || true)"
    if [[ "$replicas" == 3 ]]; then
      printf 'Redis Cluster ready: three primaries, three replicas; data: %s\n' "$DATA_DIR"
      printf 'REDIS_CLUSTER_NODES=redis://127.0.0.1:%s/,redis://127.0.0.1:%s/,redis://127.0.0.1:%s/\n' \
        "$BASE_PORT" "$((BASE_PORT + 1))" "$((BASE_PORT + 2))"
      exit 0
    fi
  fi
  sleep 1
done
printf 'Cluster not ready. Existing topology was preserved; inspect cluster nodes and logs.\n' >&2
exit 1

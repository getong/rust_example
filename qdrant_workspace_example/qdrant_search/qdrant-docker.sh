#!/usr/bin/env bash
#
# 管理本地 Qdrant Docker 容器（供 qdrant_search 示例连接）
#
#   ./qdrant-docker.sh start    启动容器（已存在则复用）
#   ./qdrant-docker.sh stop     停止并删除容器
#   ./qdrant-docker.sh restart  重启
#   ./qdrant-docker.sh status   查看状态 / 健康检查
#   ./qdrant-docker.sh logs     跟踪日志
#
set -euo pipefail

CONTAINER_NAME="${QDRANT_CONTAINER:-qdrant-local}"
IMAGE="${QDRANT_IMAGE:-qdrant/qdrant:v1.19}"
REST_PORT="${QDRANT_REST_PORT:-6333}"
GRPC_PORT="${QDRANT_GRPC_PORT:-6334}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STORAGE_DIR="${QDRANT_STORAGE:-$SCRIPT_DIR/qdrant_storage}"

log() { printf '\033[0;32m==>\033[0m %s\n' "$*"; }
err() { printf '\033[0;31m!!!\033[0m %s\n' "$*" >&2; }

require_docker() {
  command -v docker >/dev/null 2>&1 || { err "找不到 docker 命令"; exit 1; }
  docker info >/dev/null 2>&1 || { err "Docker 守护进程未运行，请先启动 Docker"; exit 1; }
}

container_exists() { docker ps -a --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"; }
container_running() { docker ps --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"; }

wait_ready() {
  log "等待 Qdrant 就绪 (http://localhost:${REST_PORT}) ..."
  for _ in $(seq 1 30); do
    if curl -fsS "http://localhost:$REST_PORT/readyz" >/dev/null 2>&1; then
      log "Qdrant 已就绪"
      return 0
    fi
    sleep 1
  done
  err "等待超时，请查看日志: $0 logs"
  return 1
}

start() {
  require_docker
  if container_running; then
    log "容器 $CONTAINER_NAME 已在运行"
  else
    if container_exists; then
      log "启动已存在的容器 $CONTAINER_NAME"
      docker start "$CONTAINER_NAME" >/dev/null
    else
      mkdir -p "$STORAGE_DIR"
      log "创建并启动容器 ${CONTAINER_NAME}（镜像 ${IMAGE}）"
      docker run -d \
        --name "$CONTAINER_NAME" \
        --restart unless-stopped \
        -p "$REST_PORT:6333" \
        -p "$GRPC_PORT:6334" \
        -v "$STORAGE_DIR:/qdrant/storage" \
        "$IMAGE" >/dev/null
    fi
  fi
  wait_ready
  cat <<EOF

  REST      : http://localhost:$REST_PORT
  gRPC      : http://localhost:$GRPC_PORT
  Dashboard : http://localhost:$REST_PORT/dashboard
  存储目录  : $STORAGE_DIR

  运行示例  : QDRANT_URL=http://localhost:$GRPC_PORT cargo run -p qdrant_search
EOF
}

stop() {
  require_docker
  if container_exists; then
    log "停止并删除容器 $CONTAINER_NAME"
    docker rm -f "$CONTAINER_NAME" >/dev/null
  else
    log "容器 $CONTAINER_NAME 不存在"
  fi
}

status() {
  require_docker
  docker ps -a --filter "name=^${CONTAINER_NAME}$" \
    --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}'
  if container_running; then
    echo
    log "健康检查:"
    curl -fsS "http://localhost:$REST_PORT/healthz" && echo
    log "现有 collections:"
    curl -fsS "http://localhost:$REST_PORT/collections" && echo
  fi
}

case "${1:-start}" in
  start)   start ;;
  stop)    stop ;;
  restart) stop; start ;;
  status)  status ;;
  logs)    require_docker; docker logs -f "$CONTAINER_NAME" ;;
  *)       err "用法: $0 {start|stop|restart|status|logs}"; exit 1 ;;
esac

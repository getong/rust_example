#!/usr/bin/env bash
# 使用 docker 运行 NATS 服务器
set -euo pipefail

CONTAINER_NAME="nats-server"
NATS_IMAGE="nats:latest"

# 如果容器已存在，先删除
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
  echo "容器 ${CONTAINER_NAME} 已存在，正在删除..."
  docker rm -f "${CONTAINER_NAME}" >/dev/null
fi

echo "启动 NATS 容器: ${CONTAINER_NAME}"
docker run -d \
  --name "${CONTAINER_NAME}" \
  -p 4222:4222 \
  -p 8222:8222 \
  -p 6222:6222 \
  "${NATS_IMAGE}" \
  -js -m 8222

echo "NATS 已启动:"
echo "  客户端端口: 4222"
echo "  监控端口:   8222 (http://localhost:8222)"
echo "  集群端口:   6222"
docker ps --filter "name=${CONTAINER_NAME}"

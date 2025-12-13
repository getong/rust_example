#!/usr/bin/env sh
set -e

: "${NODE_ID:=1}"
: "${REDIS_PORT:=6379}"
: "${REDIS_PASSWORD:=abc123}"

DATA_DIR="/data/node${NODE_ID}"
mkdir -p "${DATA_DIR}"

exec redis-server /etc/redis/redis-raft.conf \
  --port "${REDIS_PORT}" \
  --bind 0.0.0.0 \
  --protected-mode no \
  --dir "${DATA_DIR}" \
  --pidfile "${DATA_DIR}/redis.pid" \
  --logfile "${DATA_DIR}/redis.log" \
  --requirepass "${REDIS_PASSWORD}"

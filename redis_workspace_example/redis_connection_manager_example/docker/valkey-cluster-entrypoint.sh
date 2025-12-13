#!/usr/bin/env sh
set -e

: "${NODE_ID:=1}"
: "${VALKEY_PORT:=6379}"
: "${CLUSTER_NODE_TIMEOUT:=5000}"
: "${VALKEY_PASSWORD:=}"
: "${CLUSTER_ANNOUNCE_IP:=$(hostname -i)}"

# Use the Redis/Valkey convention where the cluster bus is the TCP port + 10000.
BUS_PORT="${CLUSTER_BUS_PORT:-$((VALKEY_PORT + 10000))}"
ANNOUNCE_PORT="${CLUSTER_ANNOUNCE_PORT:-$VALKEY_PORT}"
ANNOUNCE_BUS_PORT="${CLUSTER_ANNOUNCE_BUS_PORT:-$BUS_PORT}"

DATA_DIR="/data/node${NODE_ID}"
CONF_FILE="${DATA_DIR}/valkey.conf"

mkdir -p "${DATA_DIR}"

{
  echo "port ${VALKEY_PORT}"
  echo "bind 0.0.0.0"
  echo "protected-mode no"
  echo "dir ${DATA_DIR}"
  echo "cluster-enabled yes"
  echo "cluster-config-file nodes-${VALKEY_PORT}.conf"
  echo "cluster-node-timeout ${CLUSTER_NODE_TIMEOUT}"
  echo "cluster-announce-ip ${CLUSTER_ANNOUNCE_IP}"
  echo "cluster-announce-port ${ANNOUNCE_PORT}"
  echo "cluster-announce-bus-port ${ANNOUNCE_BUS_PORT}"
  echo "appendonly yes"
  echo "logfile ${DATA_DIR}/valkey.log"
  echo "pidfile ${DATA_DIR}/valkey.pid"
} > "${CONF_FILE}"

if [ -n "${VALKEY_PASSWORD}" ]; then
  {
    echo "requirepass ${VALKEY_PASSWORD}"
    echo "masterauth ${VALKEY_PASSWORD}"
  } >> "${CONF_FILE}"
fi

exec valkey-server "${CONF_FILE}"

#!/usr/bin/env bash
set -euo pipefail

export PATH="/root/.tiup/bin:${PATH}"

# 1) Launch TiDB playground in the background
echo "[db] starting tiup playground ${TIDB_VERSION} ..."
tiup playground "${TIDB_VERSION}" --host "${TIDB_HOST}" --tag "${TIDB_TAG}" &
PLAY_PID=$!

# 2) Wait for TiDB SQL port to become available
echo "[db] waiting TiDB on ${DB_HOST}:${DB_PORT} ..."
for _ in $(seq 1 120); do
  if mysqladmin ping -h"${DB_HOST}" -P"${DB_PORT}" --silent 2>/dev/null; then
    echo "[db] TiDB is up"
    break
  fi
  sleep 2
done

# 3) Ensure target database exists
echo "[db] ensuring database '${DB_NAME}' exists ..."
mysql -h "${DB_HOST}" -P "${DB_PORT}" -u "${DB_USER}" -e "CREATE DATABASE IF NOT EXISTS \`${DB_NAME}\` CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci;"

# 4) Apply initialization SQL files in /docker-entrypoint-initdb.d
INIT_DIR="/docker-entrypoint-initdb.d"
if [ -d "${INIT_DIR}" ]; then
  shopt -s nullglob
  for f in "${INIT_DIR}"/*.sql "${INIT_DIR}"/*.sql.gz; do
    case "$f" in
      *.sql)
        echo "[db] applying: $f"
        mysql --default-character-set=utf8mb4 -h "${DB_HOST}" -P "${DB_PORT}" -u "${DB_USER}" -D "${DB_NAME}" < "$f"
        ;;
      *.sql.gz)
        echo "[db] applying (gz): $f"
        gunzip -c "$f" | mysql --default-character-set=utf8mb4 -h "${DB_HOST}" -P "${DB_PORT}" -u "${DB_USER}" -D "${DB_NAME}"
        ;;
    esac
  done
fi

echo "[db] init done. following playground ..."
# 5) Keep container alive while playground runs
wait "${PLAY_PID}"

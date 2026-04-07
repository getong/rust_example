#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ROOT_DIR}/.env"
DEFAULT_SQL_FILE="${ROOT_DIR}/scripts/init-local-supabase.sql"
SQL_FILE="${1:-${DEFAULT_SQL_FILE}}"
DEFAULT_DOCKER_CONTAINER="supabase-db"

if [[ ! -f "${SQL_FILE}" ]]; then
  echo "SQL file not found: ${SQL_FILE}" >&2
  exit 1
fi

if command -v psql >/dev/null 2>&1; then
  PSQL_BIN="psql"
elif [[ -x /opt/homebrew/opt/postgresql@18/bin/psql ]]; then
  PSQL_BIN="/opt/homebrew/opt/postgresql@18/bin/psql"
else
  PSQL_BIN=""
fi

read_env_file_value() {
  local key="$1"

  if [[ ! -f "${ENV_FILE}" ]]; then
    return 1
  fi

  local line
  line="$(grep -E "^${key}=" "${ENV_FILE}" | tail -n 1 || true)"

  if [[ -z "${line}" ]]; then
    return 1
  fi

  local value="${line#*=}"
  value="${value%$'\r'}"

  if [[ "${value}" == \"*\" && "${value}" == *\" ]]; then
    value="${value:1:-1}"
  elif [[ "${value}" == \'*\' && "${value}" == *\' ]]; then
    value="${value:1:-1}"
  fi

  printf '%s' "${value}"
}

resolve_config_value() {
  local default_value="$1"
  shift

  local key
  for key in "$@"; do
    if [[ -n "${!key:-}" ]]; then
      printf '%s' "${!key}"
      return 0
    fi

    local file_value
    file_value="$(read_env_file_value "${key}" || true)"
    if [[ -n "${file_value}" ]]; then
      printf '%s' "${file_value}"
      return 0
    fi
  done

  printf '%s' "${default_value}"
}

PGHOST="$(resolve_config_value "127.0.0.1" PGHOST POSTGRES_HOST)"
PGPORT="$(resolve_config_value "5432" PGPORT POSTGRES_PORT)"
PGUSER="$(resolve_config_value "postgres" PGUSER POSTGRES_USER)"
PGDATABASE="$(resolve_config_value "postgres" PGDATABASE POSTGRES_DB POSTGRES_DATABASE)"
PGPASSWORD_VALUE="$(resolve_config_value "" PGPASSWORD POSTGRES_PASSWORD)"
POOLER_TENANT_ID_VALUE="$(resolve_config_value "" POOLER_TENANT_ID)"
DOCKER_CONTAINER_NAME="$(resolve_config_value "${DEFAULT_DOCKER_CONTAINER}" SUPABASE_DB_CONTAINER DB_CONTAINER_NAME)"

if [[ -z "${PGPASSWORD_VALUE}" ]]; then
  echo "PGPASSWORD or POSTGRES_PASSWORD must be set in the shell or .env before running this script" >&2
  exit 1
fi

build_database_url() {
  local user="$1"
  printf 'postgresql://%s:%s@%s:%s/%s' \
    "${user}" \
    "${PGPASSWORD_VALUE}" \
    "${PGHOST}" \
    "${PGPORT}" \
    "${PGDATABASE}"
}

run_psql() {
  local user="$1"
  local database_url
  database_url="$(build_database_url "${user}")"
  "${PSQL_BIN}" "${database_url}" -f "${SQL_FILE}"
}

docker_container_running() {
  command -v docker >/dev/null 2>&1 &&
    docker ps --format '{{.Names}}' | grep -Fxq "${DOCKER_CONTAINER_NAME}"
}

run_psql_in_docker() {
  docker exec -i "${DOCKER_CONTAINER_NAME}" \
    psql \
    -v ON_ERROR_STOP=1 \
    -U "${PGUSER}" \
    -d "${PGDATABASE}" \
    -f - < "${SQL_FILE}"
}

echo "Executing SQL file: ${SQL_FILE}"
echo "Database URL host: ${PGHOST}:${PGPORT}"
echo "Database name: ${PGDATABASE}"
echo "Database user: ${PGUSER}"

if docker_container_running; then
  echo "Using docker container: ${DOCKER_CONTAINER_NAME}"
  run_psql_in_docker
  exit 0
fi

if [[ -z "${PSQL_BIN}" ]]; then
  echo "psql not found in PATH and /opt/homebrew/opt/postgresql@18/bin/psql is unavailable" >&2
  exit 1
fi

TMP_ERROR_FILE="$(mktemp)"
cleanup() {
  rm -f "${TMP_ERROR_FILE}"
}
trap cleanup EXIT

if run_psql "${PGUSER}" 2>"${TMP_ERROR_FILE}"; then
  exit 0
fi

if grep -q "Tenant or user not found" "${TMP_ERROR_FILE}" && [[ -n "${POOLER_TENANT_ID_VALUE}" ]]; then
  TENANT_QUALIFIED_USER="${PGUSER}.${POOLER_TENANT_ID_VALUE}"
  echo "Retrying with Supavisor tenant-qualified user: ${TENANT_QUALIFIED_USER}" >&2
  run_psql "${TENANT_QUALIFIED_USER}"
  exit 0
fi

cat "${TMP_ERROR_FILE}" >&2
exit 1

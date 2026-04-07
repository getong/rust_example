#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -f "${ROOT_DIR}/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "${ROOT_DIR}/.env"
  set +a
fi

EMAIL="${1:-}"
PASSWORD="${2:-}"
SUPABASE_BASE_URL="${SUPABASE_URL:-${SUPABASE_PUBLIC_URL:-${API_EXTERNAL_URL:-}}}"
SERVICE_KEY="${SUPABASE_KEY:-${SERVICE_ROLE_KEY:-}}"

if [[ -z "${EMAIL}" || -z "${PASSWORD}" ]]; then
  echo "Usage: ./scripts/create-local-supabase-auth-user.sh <email> <password>" >&2
  exit 1
fi

if [[ -z "${SUPABASE_BASE_URL}" ]]; then
  echo "SUPABASE_URL or SUPABASE_PUBLIC_URL or API_EXTERNAL_URL must be set" >&2
  exit 1
fi

if [[ -z "${SERVICE_KEY}" ]]; then
  echo "SUPABASE_KEY or SERVICE_ROLE_KEY must be set" >&2
  exit 1
fi

AUTH_ADMIN_URL="${SUPABASE_BASE_URL%/}/auth/v1/admin/users"

curl --noproxy '*' -sS \
  -X POST \
  -H "apikey: ${SERVICE_KEY}" \
  -H "Authorization: Bearer ${SERVICE_KEY}" \
  -H "content-type: application/json" \
  "${AUTH_ADMIN_URL}" \
  -d "$(printf '{"email":"%s","password":"%s","email_confirm":true}' "${EMAIL}" "${PASSWORD}")"

printf '\n'

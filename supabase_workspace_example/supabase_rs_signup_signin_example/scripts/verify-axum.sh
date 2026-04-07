#!/usr/bin/env bash

set -euo pipefail

BASE_URL="${BASE_URL:-https://127.0.0.1:3000}"
TIMESTAMP="${TIMESTAMP:-$(date +%s)}"
CUSTOM_EMAIL="${CUSTOM_EMAIL:-axum-custom-${TIMESTAMP}@example.com}"
SUPABASE_EMAIL="${SUPABASE_EMAIL:-axum-supabase-${TIMESTAMP}@example.com}"
PASSWORD="${PASSWORD:-super-secret-password}"
VERIFY_SUPABASE_SIGNUP="${VERIFY_SUPABASE_SIGNUP:-0}"
VERIFY_SUPABASE_SIGNIN="${VERIFY_SUPABASE_SIGNIN:-0}"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

request() {
  local method="$1"
  local path="$2"
  local expected_status="$3"
  local body="${4:-}"
  local name="$5"
  local response_file="$TMPDIR/${name}.json"
  local status

  echo ""
  echo "==> ${method} ${BASE_URL}${path}"

  if [[ -n "$body" ]]; then
    status="$(curl --noproxy '*' -k -sS \
      -o "$response_file" \
      -w "%{http_code}" \
      -X "$method" \
      -H "content-type: application/json" \
      "${BASE_URL}${path}" \
      -d "$body")"
  else
    status="$(curl --noproxy '*' -k -sS \
      -o "$response_file" \
      -w "%{http_code}" \
      -X "$method" \
      "${BASE_URL}${path}")"
  fi

  cat "$response_file"
  echo ""

  if [[ "$status" != "$expected_status" ]]; then
    echo "Request failed: expected HTTP ${expected_status}, got ${status}" >&2
    exit 1
  fi
}

json_credentials() {
  local email="$1"
  printf '{"email":"%s","password":"%s"}' "$email" "$PASSWORD"
}

echo "Verifying Axum app at ${BASE_URL}"
echo "Using custom email: ${CUSTOM_EMAIL}"
echo "Using Supabase email: ${SUPABASE_EMAIL}"

request "GET" "/health" "200" "" "health"
request "GET" "/" "200" "" "index"
request "GET" "/ws/demo" "200" "" "ws-demo"

request "POST" "/auth/signup" "201" "$(json_credentials "$CUSTOM_EMAIL")" "custom-signup"
request "POST" "/auth/signin" "200" "$(json_credentials "$CUSTOM_EMAIL")" "custom-signin"

if [[ "$VERIFY_SUPABASE_SIGNUP" == "1" ]]; then
  request "POST" "/auth/supabase/signup" "201" "$(json_credentials "$SUPABASE_EMAIL")" "supabase-signup"
else
  echo ""
  echo "Skipping /auth/supabase/signup by default."
  echo "Reason: your self-hosted GoTrue may require the dev mail service (supabase-mail) to send confirmation emails."
  echo "Set VERIFY_SUPABASE_SIGNUP=1 to include native Supabase email signup."
fi

if [[ "$VERIFY_SUPABASE_SIGNIN" == "1" ]]; then
  request "POST" "/auth/supabase/signin" "200" "$(json_credentials "$SUPABASE_EMAIL")" "supabase-signin"
else
  echo ""
  echo "Skipping /auth/supabase/signin by default."
  echo "Set VERIFY_SUPABASE_SIGNIN=1 only after signup succeeds and your GoTrue config allows immediate sign-in."
fi

echo ""
echo "Axum verification completed successfully."

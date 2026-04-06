#!/usr/bin/env bash

set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3000}"
TIMESTAMP="${TIMESTAMP:-$(date +%s)}"
CUSTOM_EMAIL="${CUSTOM_EMAIL:-auth-custom-${TIMESTAMP}@example.com}"
SUPABASE_EMAIL="${SUPABASE_EMAIL:-auth-supabase-${TIMESTAMP}@example.com}"
PASSWORD="${PASSWORD:-super-secret-password}"
WRONG_PASSWORD="${WRONG_PASSWORD:-wrong-password}"
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
    status="$(curl --noproxy '*' -sS \
      -o "$response_file" \
      -w "%{http_code}" \
      -X "$method" \
      -H "content-type: application/json" \
      "${BASE_URL}${path}" \
      -d "$body")"
  else
    status="$(curl --noproxy '*' -sS \
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

assert_contains() {
  local file="$1"
  local pattern="$2"
  local description="$3"

  if ! rg -q "$pattern" "$file"; then
    echo "Assertion failed: ${description}" >&2
    echo "Expected pattern: ${pattern}" >&2
    exit 1
  fi
}

json_credentials() {
  local email="$1"
  local password="$2"
  printf '{"email":"%s","password":"%s"}' "$email" "$password"
}

echo "Checking auth handlers at ${BASE_URL}"
echo "Using custom email: ${CUSTOM_EMAIL}"
echo "Using Supabase email: ${SUPABASE_EMAIL}"

request "POST" "/auth/signup" "201" "$(json_credentials "$CUSTOM_EMAIL" "$PASSWORD")" "custom-signup"
assert_contains "$TMPDIR/custom-signup.json" "\"email\":\"${CUSTOM_EMAIL}\"" "custom signup should echo email"
assert_contains "$TMPDIR/custom-signup.json" "\"access_token\":\"" "custom signup should return access token"

request "POST" "/auth/signup" "409" "$(json_credentials "$CUSTOM_EMAIL" "$PASSWORD")" "custom-signup-conflict"
assert_contains "$TMPDIR/custom-signup-conflict.json" "email already exists" "duplicate custom signup should conflict"

request "POST" "/auth/signin" "200" "$(json_credentials "$CUSTOM_EMAIL" "$PASSWORD")" "custom-signin"
assert_contains "$TMPDIR/custom-signin.json" "\"email\":\"${CUSTOM_EMAIL}\"" "custom signin should echo email"
assert_contains "$TMPDIR/custom-signin.json" "\"access_token\":\"" "custom signin should return access token"

request "POST" "/auth/signin" "401" "$(json_credentials "$CUSTOM_EMAIL" "$WRONG_PASSWORD")" "custom-signin-wrong-password"
assert_contains "$TMPDIR/custom-signin-wrong-password.json" "invalid email or password" "wrong password should be rejected"

request "POST" "/auth/signin" "401" "$(json_credentials "missing-${TIMESTAMP}@example.com" "$PASSWORD")" "custom-signin-missing-user"
assert_contains "$TMPDIR/custom-signin-missing-user.json" "invalid email or password" "missing user should be rejected"

if [[ "$VERIFY_SUPABASE_SIGNUP" == "1" ]]; then
  request "POST" "/auth/supabase/signup" "201" "$(json_credentials "$SUPABASE_EMAIL" "$PASSWORD")" "supabase-signup"
  assert_contains "$TMPDIR/supabase-signup.json" "\"provider\":\"supabase_auth\"" "supabase signup should identify provider"
  assert_contains "$TMPDIR/supabase-signup.json" "\"email\":\"${SUPABASE_EMAIL}\"" "supabase signup should echo email"
  assert_contains "$TMPDIR/supabase-signup.json" "\"access_token\":\"" "supabase signup should return access token"
  assert_contains "$TMPDIR/supabase-signup.json" "\"refresh_token\":\"" "supabase signup should return refresh token"
  assert_contains "$TMPDIR/supabase-signup.json" "\"token_type\":\"bearer\"" "supabase signup should return bearer token type"
  assert_contains "$TMPDIR/supabase-signup.json" "\"message\":\"signed up with supabase_auth\"" "supabase signup should return success message"
else
  echo ""
  echo "Skipping /auth/supabase/signup by default."
  echo "Enable it with VERIFY_SUPABASE_SIGNUP=1 when your GoTrue mail flow is available."
fi

if [[ "$VERIFY_SUPABASE_SIGNIN" == "1" ]]; then
  request "POST" "/auth/supabase/signin" "200" "$(json_credentials "$SUPABASE_EMAIL" "$PASSWORD")" "supabase-signin"
  assert_contains "$TMPDIR/supabase-signin.json" "\"provider\":\"supabase_auth\"" "supabase signin should identify provider"
  assert_contains "$TMPDIR/supabase-signin.json" "\"email\":\"${SUPABASE_EMAIL}\"" "supabase signin should echo email"
  assert_contains "$TMPDIR/supabase-signin.json" "\"access_token\":\"" "supabase signin should return access token"
  assert_contains "$TMPDIR/supabase-signin.json" "\"refresh_token\":\"" "supabase signin should return refresh token"
  assert_contains "$TMPDIR/supabase-signin.json" "\"token_type\":\"bearer\"" "supabase signin should return bearer token type"
  assert_contains "$TMPDIR/supabase-signin.json" "\"message\":\"signed in with supabase_auth\"" "supabase signin should return success message"
else
  echo ""
  echo "Skipping /auth/supabase/signin by default."
  echo "Enable it with VERIFY_SUPABASE_SIGNIN=1 after native signup works and confirmation rules allow sign-in."
fi

echo ""
echo "Auth verification completed successfully."

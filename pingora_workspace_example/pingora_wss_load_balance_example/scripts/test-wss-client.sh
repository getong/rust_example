#!/usr/bin/env bash

set -euo pipefail

URL="${1:-wss://127.0.0.1:6188}"
CA_CERT="${2:-certs/localhost-cert.pem}"
MESSAGE="${3:-ping}"
HOST_PORT="${URL#wss://}"
HOST_PORT="${HOST_PORT%%/*}"

if [[ ! -f "${CA_CERT}" ]]; then
  echo "error: CA cert not found: ${CA_CERT}" >&2
  echo "hint: run ./scripts/generate-dev-cert.sh first" >&2
  exit 1
fi

if command -v openssl >/dev/null 2>&1; then
  set +e
  TLS_PROBE_OUTPUT="$(echo | openssl s_client -connect "${HOST_PORT}" -brief 2>&1)"
  TLS_PROBE_STATUS=$?
  set -e

  if [[ ${TLS_PROBE_STATUS} -ne 0 ]]; then
    if echo "${TLS_PROBE_OUTPUT}" | grep -q "packet length too long"; then
      echo "error: ${HOST_PORT} is speaking plain TCP/HTTP (ws), not TLS (wss)." >&2
      echo "start server with TLS:" >&2
      echo "  ./scripts/generate-dev-cert.sh" >&2
      echo "  WS_DOWNSTREAM_TLS=true cargo run" >&2
      exit 1
    fi

    if echo "${TLS_PROBE_OUTPUT}" | grep -Eq "Connection refused|connect:errno=61|connect error"; then
      echo "error: cannot connect to ${HOST_PORT}. Is the server running?" >&2
      echo "start server:" >&2
      echo "  WS_DOWNSTREAM_TLS=true cargo run" >&2
      exit 1
    fi
  fi
fi

if command -v wscat >/dev/null 2>&1; then
  exec wscat -c "${URL}" --ca "${CA_CERT}" -x "${MESSAGE}"
fi

if command -v npx >/dev/null 2>&1; then
  exec npx --yes wscat -c "${URL}" --ca "${CA_CERT}" -x "${MESSAGE}"
fi

echo "error: neither wscat nor npx found in PATH" >&2
echo "install: npm i -g wscat  (or install Node.js for npx)" >&2
exit 1

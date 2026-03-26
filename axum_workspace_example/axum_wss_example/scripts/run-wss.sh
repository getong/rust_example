#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CERT_DIR="${CERT_DIR:-${ROOT_DIR}/certs}"
CERT_PATH="${TLS_CERT_PATH:-${CERT_DIR}/localhost.crt}"
KEY_PATH="${TLS_KEY_PATH:-${CERT_DIR}/localhost.key}"
BIND_ADDR="${BIND_ADDR:-127.0.0.1:3000}"

if [[ ! -f "${CERT_PATH}" || ! -f "${KEY_PATH}" ]]; then
  echo "TLS cert/key not found, generating dev certificate..."
  "${ROOT_DIR}/scripts/gen-dev-cert.sh" "${CERT_DIR}"
fi

cd "${ROOT_DIR}"

echo "Starting axum with TLS"
echo "  BIND_ADDR=${BIND_ADDR}"
echo "  TLS_CERT_PATH=${CERT_PATH}"
echo "  TLS_KEY_PATH=${KEY_PATH}"

env \
  BIND_ADDR="${BIND_ADDR}" \
  TLS_CERT_PATH="${CERT_PATH}" \
  TLS_KEY_PATH="${KEY_PATH}" \
  cargo run "$@"

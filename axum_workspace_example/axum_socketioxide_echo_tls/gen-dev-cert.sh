#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CERT_DIR="${1:-${ROOT_DIR}/axum_socketioxide_echo_tls/self_signed_certs}"
CERT_PATH="${CERT_DIR}/cert.pem"
KEY_PATH="${CERT_DIR}/key.pem"
DAYS="${DAYS:-3650}"

mkdir -p "${CERT_DIR}"

TMP_OPENSSL_CFG="$(mktemp)"
cleanup() {
  rm -f "${TMP_OPENSSL_CFG}"
}
trap cleanup EXIT

cat > "${TMP_OPENSSL_CFG}" <<'CFG'
[req]
default_bits = 2048
prompt = no
default_md = sha256
distinguished_name = dn
x509_extensions = v3_req

[dn]
C = CN
ST = Shanghai
L = Shanghai
O = axum-local-dev
OU = websocket
CN = localhost

[v3_req]
subjectAltName = @alt_names
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth

[alt_names]
DNS.1 = localhost
IP.1 = 127.0.0.1
IP.2 = ::1
CFG

openssl req \
  -x509 \
  -nodes \
  -newkey rsa:2048 \
  -days "${DAYS}" \
  -keyout "${KEY_PATH}" \
  -out "${CERT_PATH}" \
  -config "${TMP_OPENSSL_CFG}"

chmod 600 "${KEY_PATH}"

printf 'Generated certificate:\n'
printf '  cert: %s\n' "${CERT_PATH}"
printf '  key : %s\n' "${KEY_PATH}"
printf '\nUse with axum:\n'
printf '  TLS_CERT_PATH="%s" TLS_KEY_PATH="%s" cargo run\n' "${CERT_PATH}" "${KEY_PATH}"

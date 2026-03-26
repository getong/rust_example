#!/usr/bin/env bash

set -euo pipefail

if ! command -v openssl >/dev/null 2>&1; then
  echo "error: openssl not found in PATH" >&2
  exit 1
fi

OUT_DIR="${1:-certs}"
CERT_PATH="${OUT_DIR}/localhost-cert.pem"
KEY_PATH="${OUT_DIR}/localhost-key.pem"
DAYS="${DAYS:-3650}"

mkdir -p "${OUT_DIR}"

tmp_conf="$(mktemp)"
cleanup() {
  rm -f "${tmp_conf}"
}
trap cleanup EXIT

cat > "${tmp_conf}" <<'EOF'
[req]
default_bits = 2048
prompt = no
default_md = sha256
distinguished_name = dn
x509_extensions = v3_req

[dn]
CN = localhost

[v3_req]
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
IP.1 = 127.0.0.1
IP.2 = ::1
EOF

openssl req \
  -x509 \
  -newkey rsa:2048 \
  -sha256 \
  -nodes \
  -days "${DAYS}" \
  -keyout "${KEY_PATH}" \
  -out "${CERT_PATH}" \
  -config "${tmp_conf}"

chmod 600 "${KEY_PATH}"

echo "Generated:"
echo "  cert: ${CERT_PATH}"
echo "  key : ${KEY_PATH}"
echo
echo "Start wss server:"
echo "  WS_DOWNSTREAM_TLS=true cargo run"

#!/usr/bin/env bash
set -euo pipefail

# Generate self-signed TLS assets used by the ClickHouse cluster examples.
# Produces:
#   tls/ca.key, tls/ca.crt, tls/server.key, tls/server.crt, tls/server.cnf
# Regenerates from scratch on each run (overwrites existing files).

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TLS_DIR="${ROOT_DIR}/tls"

echo "Recreating TLS assets in ${TLS_DIR}"
rm -rf "${TLS_DIR}"
mkdir -p "${TLS_DIR}"

cat > "${TLS_DIR}/server.cnf" <<'EOF'
[ req ]
default_bits = 4096
default_md = sha256
prompt = no
distinguished_name = dn
req_extensions = req_ext

[ dn ]
CN = clickhouse-dev

[ req_ext ]
subjectAltName = @alt_names
basicConstraints = CA:false
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth

[ alt_names ]
DNS.1 = localhost
DNS.2 = ch1
DNS.3 = ch2
DNS.4 = ch3
DNS.5 = ch4
IP.1 = 127.0.0.1
EOF

pushd "${TLS_DIR}" >/dev/null

echo "Generating CA key and certificate..."
openssl genrsa -out ca.key 4096
openssl req -x509 -new -nodes -key ca.key -sha256 -days 3650 \
  -out ca.crt -subj "/CN=clickhouse-dev-ca"

echo "Generating server key and CSR..."
openssl genrsa -out server.key 4096
openssl req -new -key server.key -out server.csr -config server.cnf

echo "Signing server certificate with CA..."
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out server.crt -days 3650 -sha256 -extensions req_ext -extfile server.cnf

rm -f server.csr ca.srl
chmod 600 server.key ca.key

popd >/dev/null

echo "Done. CA: ${TLS_DIR}/ca.crt, Server cert: ${TLS_DIR}/server.crt"

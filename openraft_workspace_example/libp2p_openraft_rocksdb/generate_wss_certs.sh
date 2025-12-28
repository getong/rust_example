#!/usr/bin/env bash
set -euo pipefail

if ! command -v openssl >/dev/null 2>&1; then
  echo "openssl is required but not found in PATH" >&2
  exit 1
fi

OUT_DIR="${1:-./certs}"
DNS_NAMES="${2:-localhost}"
IP_ADDRS="${3:-127.0.0.1}"

mkdir -p "$OUT_DIR"

KEY_PEM="$OUT_DIR/private.pem"
CERT_PEM="$OUT_DIR/fullchain.pem"
KEY_DER="$OUT_DIR/private.der"
CERT_DER="$OUT_DIR/fullchain.der"

trim_ws() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

build_san() {
  local entry
  local -a parts=()

  IFS=',' read -r -a dns_list <<<"$DNS_NAMES"
  for entry in "${dns_list[@]}"; do
    entry="$(trim_ws "$entry")"
    [[ -z "$entry" ]] && continue
    parts+=("DNS:${entry}")
  done

  IFS=',' read -r -a ip_list <<<"$IP_ADDRS"
  for entry in "${ip_list[@]}"; do
    entry="$(trim_ws "$entry")"
    [[ -z "$entry" ]] && continue
    parts+=("IP:${entry}")
  done

  if ((${#parts[@]} == 0)); then
    echo "Error: no subjectAltName entries provided" >&2
    exit 1
  fi

  (IFS=','; echo "${parts[*]}")
}

SAN="$(build_san)"
CN="$(trim_ws "${DNS_NAMES%%,*}")"
if [[ -z "$CN" ]]; then
  CN="$(trim_ws "${IP_ADDRS%%,*}")"
fi
if [[ -z "$CN" ]]; then
  CN="localhost"
fi

openssl req -x509 -newkey rsa:2048 -nodes -days 365 \
  -keyout "$KEY_PEM" \
  -out "$CERT_PEM" \
  -subj "/CN=${CN}" \
  -addext "subjectAltName=${SAN}" \
  -addext "basicConstraints=CA:FALSE" \
  -addext "keyUsage=digitalSignature,keyEncipherment" \
  -addext "extendedKeyUsage=serverAuth"

openssl pkcs8 -topk8 -inform PEM -outform DER -in "$KEY_PEM" -out "$KEY_DER" -nocrypt
openssl x509 -in "$CERT_PEM" -outform DER -out "$CERT_DER"

echo "Wrote:"
echo "  $KEY_DER"
echo "  $CERT_DER"

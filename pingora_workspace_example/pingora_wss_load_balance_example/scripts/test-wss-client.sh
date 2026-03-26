#!/usr/bin/env bash

set -euo pipefail

URL="wss://127.0.0.1:6188"
CA_CERT="certs/localhost-cert.pem"
MESSAGE="ping"
COUNT=1
ROUND_ROBIN=false
EXPECT=""

usage() {
  cat <<'EOF'
Usage:
  ./scripts/test-wss-client.sh [options] [url] [ca_cert] [message]

Options:
  --url <wss://host:port/path>   websocket URL (default: wss://127.0.0.1:6188)
  --ca <path>                    CA cert path (default: certs/localhost-cert.pem)
  --message <text>               base message payload (default: ping)
  --count <n>                    number of requests (default: 1)
  --round-robin                  check traffic distribution across upstreams
  --expect <text>                require response to contain this text
  -h, --help                     show help

Examples:
  ./scripts/test-wss-client.sh
  ./scripts/test-wss-client.sh --count 5 --message hello
  ./scripts/test-wss-client.sh --round-robin --count 8 --message rr
  ./scripts/test-wss-client.sh --count 3 --expect upstream- --message ping
EOF
}

is_positive_int() {
  [[ "$1" =~ ^[1-9][0-9]*$ ]]
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --url)
      URL="${2:-}"
      shift 2
      ;;
    --ca)
      CA_CERT="${2:-}"
      shift 2
      ;;
    --message)
      MESSAGE="${2:-}"
      shift 2
      ;;
    --count)
      COUNT="${2:-}"
      shift 2
      ;;
    --round-robin)
      ROUND_ROBIN=true
      shift
      ;;
    --expect)
      EXPECT="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      break
      ;;
  esac
done

# Backward compatible positional arguments:
#   ./scripts/test-wss-client.sh [url] [ca_cert] [message]
if [[ $# -ge 1 ]]; then
  URL="$1"
fi
if [[ $# -ge 2 ]]; then
  CA_CERT="$2"
fi
if [[ $# -ge 3 ]]; then
  MESSAGE="$3"
fi

if ! is_positive_int "${COUNT}"; then
  echo "error: --count must be a positive integer, got: ${COUNT}" >&2
  exit 1
fi

if [[ "${URL}" != wss://* ]]; then
  echo "error: URL must start with wss://, got: ${URL}" >&2
  exit 1
fi

HOST_PORT="${URL#wss://}"
HOST_PORT="${HOST_PORT%%/*}"

if [[ ! -f "${CA_CERT}" ]]; then
  echo "warn: CA cert not found: ${CA_CERT}" >&2
  echo "      node client will continue with insecure TLS verification disabled for local testing." >&2
fi

if command -v openssl >/dev/null 2>&1; then
  set +e
  TLS_PROBE_OUTPUT="$(openssl s_client -connect "${HOST_PORT}" -brief </dev/null 2>&1)"
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

    if echo "${TLS_PROBE_OUTPUT}" | grep -Eq "Connection refused|connect:errno=61|connect error|Operation not permitted|connect:errno=1"; then
      echo "error: cannot connect to ${HOST_PORT}. Is the server running?" >&2
      echo "start server:" >&2
      echo "  WS_DOWNSTREAM_TLS=true cargo run" >&2
      exit 1
    fi
  fi
fi

run_node_client() {
  local payload="$1"
  NODE_NO_WARNINGS=1 NODE_TLS_REJECT_UNAUTHORIZED=0 node - "${URL}" "${payload}" <<'NODE'
const [url, payload] = process.argv.slice(2);
const ws = new WebSocket(url);
let done = false;
const timer = setTimeout(() => {
  if (!done) {
    done = true;
    console.error("error: websocket response timeout");
    process.exit(1);
  }
}, 6000);

ws.addEventListener("open", () => {
  ws.send(payload);
});

ws.addEventListener("message", (event) => {
  if (done) return;
  done = true;
  clearTimeout(timer);
  const body = typeof event.data === "string" ? event.data : String(event.data);
  console.log(body);
  ws.close();
});

ws.addEventListener("error", () => {
  if (done) return;
  done = true;
  clearTimeout(timer);
  console.error("error: websocket connection failed");
  process.exit(1);
});

ws.addEventListener("close", () => {
  if (!done) {
    done = true;
    clearTimeout(timer);
    process.exit(1);
  }
  process.exit(0);
});
NODE
}

run_wscat_fallback() {
  local payload="$1"
  local output

  if command -v wscat >/dev/null 2>&1; then
    output="$(wscat -c "${URL}" --ca "${CA_CERT}" -x "${payload}" 2>&1)"
  elif command -v npx >/dev/null 2>&1; then
    output="$(npx --yes wscat -c "${URL}" --ca "${CA_CERT}" -x "${payload}" 2>&1)"
  else
    echo "error: neither wscat nor npx found in PATH" >&2
    echo "install: npm i -g wscat  (or install Node.js for npx)" >&2
    return 1
  fi

  # Hide noisy npm config warning when npx is used.
  printf '%s\n' "${output}" | sed '/^npm warn Unknown user config "disturl"\./d' | sed '/^npm warn .*npmrc.*/d'
}

run_ws_message() {
  local payload="$1"
  if command -v node >/dev/null 2>&1; then
    run_node_client "${payload}"
    return
  fi
  run_wscat_fallback "${payload}"
}

assert_expected() {
  local content="$1"
  if [[ -n "${EXPECT}" ]] && ! printf '%s\n' "${content}" | grep -Fq "${EXPECT}"; then
    echo "error: expected response to contain: ${EXPECT}" >&2
    echo "response:" >&2
    printf '%s\n' "${content}" >&2
    return 1
  fi
}

if [[ "${ROUND_ROBIN}" == "true" ]]; then
  if [[ "${COUNT}" -lt 2 ]]; then
    echo "error: --round-robin requires --count >= 2" >&2
    exit 1
  fi

  tmp_upstreams="$(mktemp)"
  trap 'rm -f "${tmp_upstreams}"' EXIT

  for ((i = 1; i <= COUNT; i++)); do
    payload="${MESSAGE}-${i}"
    response="$(run_ws_message "${payload}")"
    assert_expected "${response}"

    upstream_name="$(printf '%s\n' "${response}" | grep -Eo '\[[^]]+\]' | head -n 1 | tr -d '[]')"
    if [[ -z "${upstream_name}" ]]; then
      upstream_name="unknown"
    fi
    printf '%s\n' "${upstream_name}" >>"${tmp_upstreams}"
    printf '[%d/%d] upstream=%s response=%s\n' "${i}" "${COUNT}" "${upstream_name}" "$(printf '%s\n' "${response}" | head -n 1)"
  done

  echo
  echo "round-robin summary:"
  sort "${tmp_upstreams}" | uniq -c | sed 's/^ *//'

  distinct_count="$(sort "${tmp_upstreams}" | uniq | wc -l | tr -d ' ')"
  if [[ "${distinct_count}" -lt 2 ]]; then
    echo "error: expected requests to hit at least 2 upstreams, got ${distinct_count}" >&2
    exit 1
  fi
  echo "round-robin check passed: ${distinct_count} upstreams observed."
  exit 0
fi

if [[ "${COUNT}" -eq 1 ]]; then
  response="$(run_ws_message "${MESSAGE}")"
  assert_expected "${response}"
  printf '%s\n' "${response}"
  exit 0
fi

for ((i = 1; i <= COUNT; i++)); do
  payload="${MESSAGE}-${i}"
  response="$(run_ws_message "${payload}")"
  assert_expected "${response}"
  printf '[%d/%d] %s\n' "${i}" "${COUNT}" "$(printf '%s\n' "${response}" | head -n 1)"
done

echo "multi-message test finished: ${COUNT} requests."
exit 0

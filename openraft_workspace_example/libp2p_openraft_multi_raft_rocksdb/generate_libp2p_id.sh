#!/usr/bin/env bash
set -euo pipefail

usage() {
	echo "Usage: $0 --key <path> [--out <path>]" >&2
}

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

KEY_PATH=""
OUT_PATH=""

while [[ $# -gt 0 ]]; do
	case "$1" in
	--key)
		KEY_PATH="$2"
		shift 2
		;;
	--out)
		OUT_PATH="$2"
		shift 2
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "Error: unknown argument: $1" >&2
		usage
		exit 1
		;;
	esac
done

if [[ -z "$KEY_PATH" ]]; then
	usage
	exit 1
fi

cd "$WS_DIR"

peer_id="$(cargo run -q -p libp2p_openraft_multi_raft_rocksdb --bin peer_id -- --key "$KEY_PATH" --create)"

if [[ -n "$OUT_PATH" ]]; then
	mkdir -p "$(dirname "$OUT_PATH")"
	printf '%s\n' "$peer_id" >"$OUT_PATH"
fi

printf '%s\n' "$peer_id"

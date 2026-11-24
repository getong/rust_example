#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_DIR/target}"
exec cargo run --manifest-path "$PROJECT_DIR/Cargo.toml" -- "$@"

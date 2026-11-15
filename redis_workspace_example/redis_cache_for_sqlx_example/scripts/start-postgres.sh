#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v docker >/dev/null || { printf 'Docker is required.\n' >&2; exit 1; }
docker info >/dev/null
docker compose -f "$PROJECT_ROOT/compose.yaml" up -d --wait postgres
printf 'Local PostgreSQL is ready. This is not a CloudNativePG cluster.\n'

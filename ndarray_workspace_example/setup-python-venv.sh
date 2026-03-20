#!/usr/bin/env sh
set -eu

if [ ! -d ".venv" ]; then
  uv venv .venv
fi

uv pip install --python .venv/bin/python -r requirements.txt

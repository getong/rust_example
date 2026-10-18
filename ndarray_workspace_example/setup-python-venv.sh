#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")"

if [ ! -d ".venv" ]; then
  uv venv --python 3.13 .venv
fi

.venv/bin/python -c 'import sys; assert sys.version_info[:2] == (3, 13), "Recreate .venv with Python 3.13 (required by .cargo/config.toml)"'
uv pip install --python .venv/bin/python \
  --index-url https://mirrors.aliyun.com/pypi/simple \
  -r requirements.txt

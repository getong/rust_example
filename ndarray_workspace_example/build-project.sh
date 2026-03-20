#!/usr/bin/env sh
set -eu

if [ ! -d ".venv" ]; then
  ./setup-python-venv.sh
fi

export LIBTORCH_USE_PYTORCH=1
TORCH_LIB=$(.venv/bin/python -c "import os, torch; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))")

export DYLD_LIBRARY_PATH="${TORCH_LIB}${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"
export LD_LIBRARY_PATH="${TORCH_LIB}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"

cargo build "$@"

#!/bin/sh

VENV_DIR="$PWD/.venv"
VENV_PYTHON="$VENV_DIR/bin/python"

source "$VENV_DIR/bin/activate"
export PATH="$VENV_DIR/bin:$PATH"

TORCH_LIB=$("$VENV_PYTHON" -W ignore -c "import torch; import os; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))" 2>/dev/null)
export DYLD_LIBRARY_PATH=$TORCH_LIB:$DYLD_LIBRARY_PATH

./target/debug/tch_basic

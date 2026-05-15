#!/bin/sh

VENV_DIR="$PWD/.venv"
VENV_PYTHON="$VENV_DIR/bin/python"

source "$VENV_DIR/bin/activate"
export PATH="$VENV_DIR/bin:$PATH"

export LIBTORCH_USE_PYTORCH=1

cargo build

#!/bin/sh

source .venv/bin/activate

export LIBTORCH_USE_PYTORCH=1
TORCH_LIB=$(python -c "import torch; import os; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))")
export DYLD_LIBRARY_PATH=$TORCH_LIB:$DYLD_LIBRARY_PATH
cargo build

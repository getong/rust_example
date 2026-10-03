#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
PROJECT_DIR="$PWD"
SOURCE_DIR="$PROJECT_DIR/.build/pytorch-v2.13.0"
PYTHON="$PROJECT_DIR/.venv/bin/python"

if [[ "$(uname -s)" != Darwin || "$(uname -m)" != x86_64 ]]; then
    echo "This script builds PyTorch for Intel macOS." >&2
    exit 1
fi
if [[ ! -x "$PYTHON" ]]; then
    uv venv --python 3.14 .venv
fi
if [[ ! -d "$SOURCE_DIR/.git" ]]; then
    git clone --branch v2.13.0 --depth 1 https://github.com/pytorch/pytorch.git "$SOURCE_DIR"
fi
if [[ "$(git -C "$SOURCE_DIR" rev-parse HEAD)" != cf30153c4c131c8164ee7798e5022d810682e2cb ]]; then
    echo "Unexpected PyTorch source revision; expected official v2.13.0." >&2
    exit 1
fi
git -C "$SOURCE_DIR" submodule update --init --recursive --depth 1 --jobs 8
uv pip install --python "$PYTHON" -r "$SOURCE_DIR/requirements-build.txt" wheel

export PATH="$PROJECT_DIR/.venv/bin:$PATH"
export CC=/usr/bin/clang
export CXX=/usr/bin/clang++
# Keep Homebrew headers after PyTorch's bundled dependency headers.
export CFLAGS="${CFLAGS:-} -isystem /usr/local/include"
export CXXFLAGS="${CXXFLAGS:-} -isystem /usr/local/include"
if [[ -f /usr/local/opt/libomp/include/omp.h ]]; then
    export OMP_PREFIX=/usr/local/opt/libomp
    export CMAKE_LIBRARY_PATH="/usr/local/opt/libomp/lib${CMAKE_LIBRARY_PATH:+:$CMAKE_LIBRARY_PATH}"
fi
export MAX_JOBS="${MAX_JOBS:-6}"
export CMAKE_BUILD_TYPE=Release
export PYTORCH_BUILD_VERSION=2.13.0
export PYTORCH_BUILD_NUMBER=1
export USE_CUDA=0
export USE_MPS=0
export USE_DISTRIBUTED=0
export USE_NNPACK=0
export BUILD_TEST=0
export BLAS=vecLib

cd "$SOURCE_DIR"
"$PYTHON" -m pip install -e . -v --no-build-isolation
cd "$PROJECT_DIR"
"$PYTHON" -c 'import torch; assert torch.__version__.split("+")[0] == "2.13.0"; print(torch.__version__); print(torch.ones(2) + 1)'

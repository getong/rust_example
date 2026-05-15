#!/bin/bash
# tch-rs 项目一键编译和运行脚本
# 使用方法: ./build_and_run.sh

set -e  # 遇到错误立即退出

VENV_DIR="$PWD/.venv"
VENV_PYTHON="$VENV_DIR/bin/python"

echo "=== tch-rs 项目构建脚本 ==="
echo ""

# 检查是否在项目根目录
if [ ! -d "tch_basic" ]; then
    echo "错误: 请在项目根目录执行此脚本"
    exit 1
fi

# 1. 检查虚拟环境
if [ ! -x "$VENV_PYTHON" ]; then
    echo "错误: 未找到虚拟环境 $VENV_DIR"
    echo "请先执行:"
    echo "  uv venv .venv"
    echo "  source .venv/bin/activate"
    echo "  uv pip install -r requirements.txt"
    exit 1
fi

# 2. 激活虚拟环境
echo "激活虚拟环境..."
source "$VENV_DIR/bin/activate"
export PATH="$VENV_DIR/bin:$PATH"

# 3. 检查 PyTorch
if ! "$VENV_PYTHON" -W ignore -c "import torch" &> /dev/null; then
    echo "错误: 当前虚拟环境中未安装 PyTorch"
    echo "请执行: uv pip install -r requirements.txt"
    exit 1
fi

# 4. 设置编译环境变量
echo "设置编译环境变量..."
export LIBTORCH_USE_PYTORCH=1

# 5. 设置运行时库路径
echo "设置运行时库路径..."
TORCH_LIB=$("$VENV_PYTHON" -W ignore -c "import torch; import os; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))" 2>/dev/null)
export DYLD_LIBRARY_PATH=$TORCH_LIB:$DYLD_LIBRARY_PATH

# 6. 编译项目
echo ""
echo "=== 开始编译项目 ==="
cargo build --bin tch_basic --release

# 7. 运行项目
echo ""
echo "=== 运行项目 ==="
cargo run --bin tch_basic --release

echo ""
echo "=== 完成 ==="

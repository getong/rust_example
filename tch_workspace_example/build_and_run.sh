#!/bin/bash
# tch-rs 项目一键编译和运行脚本
# 使用方法: ./build_and_run.sh

set -e  # 遇到错误立即退出

echo "=== tch-rs 项目构建脚本 ==="
echo ""

# 检查是否在项目根目录
if [ ! -d "tch_basic" ]; then
    echo "错误: 请在项目根目录执行此脚本"
    exit 1
fi

# 1. 检查 uv 是否安装
if ! command -v uv &> /dev/null; then
    echo "正在安装 uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.cargo/bin:$PATH"
else
    echo "✓ uv 已安装"
fi

# 2. 创建虚拟环境（如果不存在）
if [ ! -d "tch_basic/.venv" ]; then
    echo "正在创建虚拟环境..."
    cd tch_basic
    uv venv
    cd ..
    echo "✓ 虚拟环境已创建"
else
    echo "✓ 虚拟环境已存在"
fi

# 3. 激活虚拟环境
echo "激活虚拟环境..."
source tch_basic/.venv/bin/activate

# 4. 安装 PyTorch（如果需要）
if ! python -c "import torch" &> /dev/null; then
    echo "正在安装 PyTorch..."
    uv pip install torch
    echo "✓ PyTorch 已安装"
else
    TORCH_VERSION=$(python -c "import torch; print(torch.__version__)")
    echo "✓ PyTorch 已安装 (版本: $TORCH_VERSION)"
fi

# 5. 设置编译环境变量
echo "设置编译环境变量..."
export LIBTORCH_USE_PYTORCH=1
export PATH="/opt/homebrew/bin:$PATH"

# 6. 设置运行时库路径
echo "设置运行时库路径..."
TORCH_LIB=$(python -c "import torch; import os; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))")
export DYLD_LIBRARY_PATH=$TORCH_LIB:$DYLD_LIBRARY_PATH

# 7. 编译项目
echo ""
echo "=== 开始编译项目 ==="
cargo build --bin tch_basic --release

# 8. 运行项目
echo ""
echo "=== 运行项目 ==="
cargo run --bin tch_basic --release

echo ""
echo "=== 完成 ==="

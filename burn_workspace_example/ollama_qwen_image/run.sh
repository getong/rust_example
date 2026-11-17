#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
arguments=("$@")
DRY_RUN=0
SKIP_TEXT=0
REPLAY=0
LIST_MODELS=0
TEXT_MODEL="${OLLAMA_MODEL:-}"

while (( $# )); do
  case "$1" in
    -h|--help)
      cat <<'HELP'
用法：./run.sh [参数]
  ./run.sh
  ./run.sh --prompt '画一匹马，在大草原上奔跑' --output horse.png
  ./run.sh --list-models
  ./run.sh --prompt '离线检查' --dry-run

先执行 ./download.sh。下载和推理均使用 Ollama CLI，不使用 Python。
默认图像模型 x/z-image-turbo；--image-model 覆盖。
默认直接使用提示词；--model NAME 可启用已下载文本模型的提示词优化。
支持 --width、--height（64 的倍数）、--steps（0 为模型默认）、--seed、--timeout。
--ollama / --host 或 OLLAMA_BIN / OLLAMA_HOST 可连接其他兼容运行时。
默认项目独立服务 127.0.0.1:11435，不修改系统 11434 上的 Ollama。
未传 --prompt 时逐行读取输入。相对路径以项目目录为基准。
HELP
      exit 0 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --skip-ollama) SKIP_TEXT=1; shift ;;
    --list-models) LIST_MODELS=1; shift ;;
    --ollama|--host|--model|--response|--prompt|--output|--image-model|--timeout|--width|--height|--steps|--seed)
      (( $# >= 2 )) || break
      case "$1" in
        --ollama) export OLLAMA_BIN="$2" ;;
        --host) export OLLAMA_HOST="$2" ;;
        --model) TEXT_MODEL="$2" ;;
        --response) REPLAY=1 ;;
      esac
      shift 2 ;;
    --ollama=*) export OLLAMA_BIN="${1#*=}"; shift ;;
    --host=*) export OLLAMA_HOST="${1#*=}"; shift ;;
    --model=*) TEXT_MODEL="${1#*=}"; shift ;;
    --response=*) REPLAY=1; shift ;;
    *) shift ;;
  esac
done

cd "$PROJECT_DIR"
source "$PROJECT_DIR/scripts/ollama-env.sh"
if (( ! DRY_RUN || LIST_MODELS )) || { [[ -n "$TEXT_MODEL" ]] && (( ! SKIP_TEXT && ! REPLAY )); }; then
  check_runtime
  ensure_server
fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_DIR/target}"
export RUSTC_WRAPPER="${QWEN_IMAGE_RUSTC_WRAPPER:-}"
exec cargo +stable run --release --manifest-path "$PROJECT_DIR/Cargo.toml" \
  -p ollama_qwen_image -- ${arguments[@]+"${arguments[@]}"}

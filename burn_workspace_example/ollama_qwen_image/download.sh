#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_MODEL="${OLLAMA_IMAGE_MODEL:-x/z-image-turbo}"
TEXT_MODEL="${OLLAMA_MODEL:-}"
REFRESH=0
OFFLINE=0

while (( $# )); do
  case "$1" in
    -h|--help)
      cat <<'HELP'
用法：./download.sh [选项]
  --model NAME / --image-model NAME  Ollama 图像模型，默认 x/z-image-turbo
  --text-model NAME                 可选的提示词优化模型，默认不下载文本模型
  --refresh                         对已安装模型也执行 ollama pull 检查更新
  --offline                         只检查本地运行时和服务中的模型，不下载

Apple Silicon macOS 默认准备项目内 Ollama 0.15.0，使用 127.0.0.1:11435。
模型由 ollama pull 下载到 .ollama/models，可用 ./run.sh --list-models 查看。
可设置 OLLAMA_BIN / OLLAMA_HOST 连接已有的兼容服务；不安装 Python。
原 Hugging Face --reuse-dir / --revision 等参数已移除，不会导入旧权重。
HELP
      exit 0 ;;
    --model|--image-model|--text-model)
      (( $# >= 2 )) || { printf '参数缺少值：%s\n' "$1" >&2; exit 2; }
      if [[ "$1" == --text-model ]]; then TEXT_MODEL="$2"; else IMAGE_MODEL="$2"; fi
      shift 2 ;;
    --refresh) REFRESH=1; shift ;;
    --offline) OFFLINE=1; shift ;;
    *) printf '未知参数：%s；请使用 --help。\n' "$1" >&2; exit 2 ;;
  esac
done
(( ! REFRESH || ! OFFLINE )) || { printf '%s\n' '--refresh 与 --offline 不能同时使用。' >&2; exit 2; }

cd "$PROJECT_DIR"
source "$PROJECT_DIR/scripts/ollama-env.sh"
validate_model "$IMAGE_MODEL"
[[ -z "$TEXT_MODEL" ]] || validate_model "$TEXT_MODEL"
if ! command -v "$OLLAMA_BIN" >/dev/null 2>&1; then
  (( MANAGED_RUNTIME && ! OFFLINE )) || fail "离线模式或自定义 OLLAMA_BIN 不会安装运行时：$OLLAMA_BIN"
  install_runtime
fi
check_runtime
printf 'Ollama 模型目录（服务启动时使用）：%s\n' "$OLLAMA_MODELS"
ensure_server

download_model() {
  local model="$1"
  if "$OLLAMA_BIN" show -- "$model" >/dev/null 2>&1; then
    if (( ! REFRESH )); then
      printf '复用已安装的 Ollama 模型：%s\n' "$model"
      return
    fi
  elif (( OFFLINE )); then
    fail "Ollama 服务中未找到模型：${model}；请先联网运行 ./download.sh。"
  fi
  printf '通过 Ollama 下载模型：%s\n' "$model"
  "$OLLAMA_BIN" pull -- "$model"
  "$OLLAMA_BIN" show -- "$model" >/dev/null || fail "下载后无法检查模型：$model"
}

download_model "$IMAGE_MODEL"
[[ -z "$TEXT_MODEL" ]] || download_model "$TEXT_MODEL"
printf '\nOllama 模型列表（%s）：\n' "$OLLAMA_HOST"
"$OLLAMA_BIN" list
printf '\n准备完成，无需 Python。运行：./run.sh --prompt '\''画一匹马，在大草原上奔跑'\''\n'
if [[ "$IMAGE_MODEL" != "$OLLAMA_IMAGE_MODEL" ]]; then
  printf '本次使用自定义图像模型；运行时请加 --image-model %s。\n' "$IMAGE_MODEL"
fi

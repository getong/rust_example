#!/usr/bin/env bash

OLLAMA_VERSION=0.15.0
RUNTIME_DIR="$PROJECT_DIR/.tools/ollama-v$OLLAMA_VERSION"
MANAGED_RUNTIME=0
if [[ -z "${OLLAMA_BIN:-}" || "${OLLAMA_BIN:-}" == "$RUNTIME_DIR/ollama" ]]; then
  OLLAMA_BIN="$RUNTIME_DIR/ollama"
  MANAGED_RUNTIME=1
fi
export OLLAMA_BIN
export OLLAMA_HOST="${OLLAMA_HOST:-127.0.0.1:11435}"
export OLLAMA_MODELS="${OLLAMA_MODELS:-$PROJECT_DIR/.ollama/models}"
export OLLAMA_IMAGE_MODEL="${OLLAMA_IMAGE_MODEL:-x/z-image-turbo}"

fail() {
  printf '%s\n' "$*" >&2
  exit 1
}

install_runtime() {
  [[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] ||
    fail '自动安装仅支持 Apple Silicon macOS；其他平台请设置兼容的 OLLAMA_BIN 和 OLLAMA_HOST。'
  mkdir -p "$PROJECT_DIR/.cache/ollama" "$PROJECT_DIR/.tools"
  local archive="$PROJECT_DIR/.cache/ollama/ollama-v$OLLAMA_VERSION-darwin.tgz"
  local checksum=bf3bc2f9472d83fdee3d0317f5dcb4ecea637c9bfc7d400a7c269acbcdcb49da
  if [[ ! -f "$archive" ]]; then
    printf '下载官方 Ollama %s 运行时（约 72 MB），不覆盖系统版本……\n' "$OLLAMA_VERSION"
    curl --fail --location --retry 3 --connect-timeout 15 \
      "https://github.com/ollama/ollama/releases/download/v$OLLAMA_VERSION/ollama-darwin.tgz" \
      -o "$archive.part"
    mv "$archive.part" "$archive"
  fi
  printf '%s  %s\n' "$checksum" "$archive" | shasum -a 256 -c - ||
    fail "运行时 SHA-256 不匹配；请移走 $archive 后重新执行，未安装或执行该文件。"
  local staging
  staging="$(mktemp -d "$PROJECT_DIR/.tools/.ollama-install-XXXXXX")"
  tar -xzf "$archive" -C "$staging"
  [[ -x "$staging/ollama" && -f "$staging/libmlxc.dylib" ]] ||
    fail "运行时文件不完整，临时目录：$staging"
  [[ ! -e "$RUNTIME_DIR" ]] || fail "运行时目录已存在但不可用，请检查：$RUNTIME_DIR"
  mv "$staging" "$RUNTIME_DIR"
}

check_runtime() {
  command -v "$OLLAMA_BIN" >/dev/null 2>&1 ||
    fail "未找到 Ollama CLI：${OLLAMA_BIN}；请先执行 ./download.sh。"
  local help
  help="$("$OLLAMA_BIN" run --help)" || fail '无法读取 Ollama CLI 帮助。'
  [[ "$help" == *--width* && "$help" == *--height* && "$help" == *--steps* && "$help" == *--seed* ]] ||
    fail "当前 Ollama CLI 不支持图像生成：${OLLAMA_BIN}；请取消自定义 OLLAMA_BIN 并执行 ./download.sh；不会使用 Python。"
}

check_server_version() {
  if (( MANAGED_RUNTIME )); then
    local version
    version="$("$OLLAMA_BIN" --version 2>&1)" || fail '无法查询 Ollama 服务版本。'
    [[ "$version" == *"ollama version is $OLLAMA_VERSION"* ]] ||
      fail "地址 $OLLAMA_HOST 上不是配套 Ollama $OLLAMA_VERSION 服务：${version}；请改用空闲端口，不要覆盖系统服务。"
  fi
}

ensure_server() {
  if "$OLLAMA_BIN" list >/dev/null 2>&1; then
    check_server_version
    return
  fi
  if (( ! MANAGED_RUNTIME )) || [[ "$OLLAMA_HOST" != 127.0.0.1:11435 ]]; then
    fail "无法连接 Ollama ${OLLAMA_HOST}；请使用指定版本执行 ollama serve；脚本不会启动或替换外部服务。"
  fi
  mkdir -p "$PROJECT_DIR/.ollama" "$OLLAMA_MODELS"
  printf '启动项目独立 Ollama %s：%s；日志：.ollama/server.log\n' "$OLLAMA_VERSION" "$OLLAMA_HOST"
  nohup "$OLLAMA_BIN" serve >"$PROJECT_DIR/.ollama/server.log" 2>&1 </dev/null &
  local server_pid=$!
  printf '%s\n' "$server_pid" >"$PROJECT_DIR/.ollama/server.pid"
  local attempt
  for ((attempt = 0; attempt < 60; attempt++)); do
    if "$OLLAMA_BIN" list >/dev/null 2>&1; then
      check_server_version
      return
    fi
    kill -0 "$server_pid" 2>/dev/null || fail '项目 Ollama 启动失败，请检查 .ollama/server.log。'
    sleep 1
  done
  fail '等待项目 Ollama 启动超时，请检查 .ollama/server.log。'
}

validate_model() {
  [[ -n "$1" && "$1" != -* && "$1" != *[[:space:]]* ]] || fail "模型名称无效：$1"
}

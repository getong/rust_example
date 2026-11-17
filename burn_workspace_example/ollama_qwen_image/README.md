# Ollama CLI 文生图（无需 Python）

下载、可选的提示词优化、图像推理全部使用 **Ollama CLI**。Rust 只负责参数、
进程管理、运行记录和 PNG 校验；不使用 Python、Diffusers、PyTorch 或 Burn。

```text
download.sh → ollama pull x/z-image-turbo
run.sh → Rust → ollama run x/z-image-turbo → 校验并发布 PNG
                  ↑
            可选 ollama run 文本模型优化提示词
```

**图像模型已从 Qwen-Image-2.1 改为 `x/z-image-turbo`**。目录/包名保留为
`ollama_qwen_image`，不表示 Ollama 支持原来的 Qwen-Image 权重。旧 Hugging Face
缓存不会导入 Ollama，也不会自动删除；旧 `.venv` 不再被使用。

## 下载与运行

默认自动安装路径面向 **Apple Silicon macOS**，需要 Rust stable、网络和足够的磁盘/统一内存。
官方 macOS 运行时归档约 72 MB；`x/z-image-turbo` 的实际下载约 12 GB，应预留更多空间。

```bash
rtk proxy ./download.sh
rtk proxy ./run.sh --list-models
rtk proxy ./run.sh --prompt '画一匹马，在大草原上奔跑' --output horse.png
rtk proxy ./run.sh
```

最后一个命令逐行读取提示词，每行生成一张图，Ctrl-D 退出。
不指定 `--output` 时使用 `output-0001.png` 等未使用的名称。

### 独立运行时与模型列表

系统安装的 Ollama 0.40.16 不支持图像生成，因此脚本固定使用官方仍包含图像实验功能的
**Ollama 0.15.0**。安装包按官方发行包的 SHA-256 校验后才解压执行；不修改 Homebrew、
不覆盖系统 Ollama，也不停止系统已有服务。

| 资源 | 默认位置 |
| --- | --- |
| Ollama CLI 与 MLX 动态库 | `.tools/ollama-v0.15.0/` |
| 项目独立服务 | `127.0.0.1:11435` |
| Ollama 模型仓库 | `.ollama/models/` |
| 服务日志 / PID | `.ollama/server.log` / `.ollama/server.pid` |
| 运行时下载缓存 | `.cache/ollama/` |

`run.sh` / `download.sh` 会在需要时启动项目独立服务，服务在脚本退出后继续运行。
遇到端口上的服务版本不匹配时明确报错，不覆盖或终止该服务。

**普通 `ollama list` 通常连接系统的 11434，不是项目的 11435。** 查看本项目下载的图像模型：

```bash
rtk proxy ./run.sh --list-models
rtk proxy env OLLAMA_HOST=127.0.0.1:11435 .tools/ollama-v0.15.0/ollama list
```

模型确实由 `ollama pull` 下载、由 Ollama 服务管理，不是独立的 safetensors 目录。
默认不将实验版服务暴露到局域网。旧实验版不是最新版本，建议仅用于受信任的本机任务。

### 下载选项

```bash
rtk proxy ./download.sh --offline
rtk proxy ./download.sh --refresh
rtk proxy ./download.sh --model x/flux2-klein
rtk proxy ./download.sh --text-model qwen2.5:7b
```

默认先 `ollama show`，模型已安装时直接复用；缺失时才 `ollama pull`。
下载失败时返回非零，保留 Ollama 的缓存，重新运行可继续；不手工拼装模型或绕过注册。
`--refresh` 对已安装模型也执行 pull；`--offline` 不下载运行时或模型，但仍需可连接的本地服务。
模型注册名是否存在、是否支持当前实验版，由 Ollama 返回实际错误；不保证任意模型都能运行。

### 可选的提示词优化

默认直接将你的提示词送给图像模型，不要求安装第二个文本模型。
如果需要优化，可以单独下载并显式指定：

```bash
rtk proxy ./download.sh --text-model qwen2.5:7b
rtk proxy ./run.sh --model qwen2.5:7b --prompt '画一匹马，在大草原上奔跑'
```

文本处理同样通过 `ollama run --format json --hidethinking`，没有直接 HTTP 客户端。
文本模型必须返回 `{"prompt":"..."}`；无效输出会保留日志并停止，不冒充图片。
`OLLAMA_MODEL` 可指定文本模型；`--skip-ollama` 只跳过文本优化，**图片依然使用 Ollama**。

### 使用自己的兼容服务

```bash
rtk proxy env OLLAMA_BIN=/path/to/compatible/ollama OLLAMA_HOST=127.0.0.1:19435 ./download.sh
rtk proxy ./run.sh --ollama /path/to/compatible/ollama --host 127.0.0.1:19435 --prompt 'A running horse'
```

使用自定义运行时/地址时，先自行启动匹配的 `ollama serve`。脚本不会安装、替换或启动外部服务。
`OLLAMA_MODELS` 仅决定**新启动服务**的模型目录，不会改变已有服务的仓库。
图像 CLI 必须支持 `--width`、`--height`、`--steps`、`--seed`，服务也必须支持对应图像模型。
这些配置不是让任意新版/旧版 Ollama 自动获得图像能力。

## 参数与记录

| 参数 | 默认 / 说明 |
| --- | --- |
| `--image-model` / `OLLAMA_IMAGE_MODEL` | `x/z-image-turbo` |
| `--ollama` / `OLLAMA_BIN` | 脚本默认项目内运行时；直接运行 Rust 时默认 `ollama` |
| `--host` / `OLLAMA_HOST` | 脚本默认 `127.0.0.1:11435`；直接运行 Rust 时默认 `127.0.0.1:11434` |
| `--model` / `OLLAMA_MODEL` | 可选文本模型，默认不优化 |
| `--width` / `--height` | 512 / 512；范围 64–4096，必须为 64 的倍数 |
| `--steps` / `--seed` | 0（模型默认步数）/ 42；seed=0 表示随机 |
| `--timeout` | 每个 CLI 进程 7200 秒，超时终止 CLI，不自动重试推理 |
| `--dry-run` | 保存请求，不生成图；指定文本模型时仍执行文本处理 |
| `--response` | 复用保存的提示词 JSON，跳过文本调用 |
| `--list-models` | 使用当前服务执行 `ollama list` 后退出 |

设备、精度、内存管理由 Ollama 决定，不再提供 Python 的 `--device`、`--dtype`、
`--python`、`--model-dir`、`--revision`、`--cfg-scale` 等参数；这些旧参数会明确报错。
没有映射负向提示词：核对的实验版虽然暴露该开关，但单次图像请求并未传递该值，不能假装有效。

```bash
rtk proxy ./run.sh --prompt 'A running horse' --width 512 --height 512 --steps 9 --seed 42
rtk proxy ./run.sh --prompt '离线检查' --dry-run
```

结果旁保存同名 `.artifacts/` 目录：原始提示词、最终提示词、`generation.json`、
CLI 的 stdout/stderr 日志；可选文本处理还保存 `request.json`、`response.json`。
Ollama CLI 在独立临时目录生成 PNG；Rust 检查恰好一张 PNG、格式和尺寸正确后，
以不覆盖已有文件的方式发布。仅完成全部检查后才写 `result.json`。
出错保留日志和推理临时文件，不写成功标记。

## 验证与设计

```bash
rtk proxy env RUSTC_WRAPPER= cargo +stable test -p ollama_qwen_image --offline
rtk proxy env RUSTC_WRAPPER= cargo +stable clippy -p ollama_qwen_image --all-targets --offline -- -D warnings
rtk proxy bash -n download.sh run.sh scripts/ollama-env.sh
```

测试也不需要 Python：Rust 集成测试使用假的 Ollama CLI 验证参数透传、下载策略、
离线检查、错误日志、超时、不覆盖输出和 PNG 校验。模拟测试不代表实际模型已完成出图。
设计与官方实现参考见 `docs/design.md`。

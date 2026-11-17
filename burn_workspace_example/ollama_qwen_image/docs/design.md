# 全 Ollama CLI 文生图设计

## 目标与模型变更

按后续需求，模型下载、文本优化、图像生成必须使用 Ollama CLI，不使用 Python。
原 `todo.org` 中的 Qwen-Image-2.1 要求因此发生变更：默认图像模型改为
Ollama 图像实验版支持的 `x/z-image-turbo`，不将它描述成 Qwen-Image。
包名保留，已有 Hugging Face 权重和 Python 环境不再使用，也不自动删除。

系统 Ollama 0.40.16 的 `cmd/cmd.go` 显式拒绝图像生成模型。官方 0.15.0
仍包含 `x/imagegen` CLI、MLX 运行时，以及 `--width/--height/--steps/--seed`。
因此在项目内固定安装官方 0.15.0 macOS 归档并验证 SHA-256，独立监听 11435；
不偷偷降级系统程序，不停止已有 11434 服务。自动安装只支持 Apple Silicon macOS。
这是实验版本兼容方案，不声称最新版 Ollama 支持相同接口。

## 模块职责

| 文件 | 职责 |
| --- | --- |
| `download.sh` | 校验运行时、`ollama show/pull/list` 管理模型 |
| `run.sh` | 配置 Ollama 环境、启动独立服务、转发 Rust CLI 参数 |
| `scripts/ollama-env.sh` | 官方运行时安装、服务隔离和版本/能力检查 |
| `src/cli.rs` | 参数、环境变量、尺寸和模型名称校验 |
| `src/main.rs` | 单次/逐行输入、工件目录和成功记录 |
| `src/ollama.rs` | CLI 子进程、超时、日志、可选文本优化 |
| `src/generation.rs` | 图像 CLI 参数、临时目录、PNG 验证和不覆盖发布 |
| `tests/cli.rs` / `tests/scripts.rs` | 无网络、无大模型、无 Python 的回归测试 |

## 下载与服务

`download.sh` 默认先准备约 72 MB 的官方运行时，再通过 `ollama pull x/z-image-turbo`
下载约 12 GB 的图像模型。依赖 Ollama 注册表、缓存和校验逻辑，而非 Hugging Face API。
已安装模型通过 `ollama show` 判断后复用，`--refresh` 才主动 pull 更新。
`--offline` 不下载；缺少运行时或模型时失败。模型 pull 失败不会输出“准备完成”。

默认模型目录 `.ollama/models` 与系统仓库隔离；普通 `ollama list` 不会显示该独立服务的模型。
`run.sh --list-models` 保证使用同一 CLI 和 OLLAMA_HOST，避免查询错服务。
自定义 OLLAMA_BIN/HOST 时不自动安装或启动外部服务；默认运行时发现服务版本不匹配时拒绝继续。
服务在脚本结束后继续运行，PID 与日志写入 `.ollama/`。

## 推理流程

1. 校验输入和输出路径，拒绝覆盖已有 PNG 或工件目录。
2. 非 dry-run 时检查 CLI 图像参数和已安装图像模型，不依赖 `ollama run` 隐式下载。
3. 默认直接使用输入提示词；显式指定文本模型时，先 show，再以 `ollama run --format json`
   获得单个非空 prompt；错误 JSON 保留，不继续图像推理。
4. 记录 generation.json，通过 Command 参数数组调用 Ollama，不执行 shell，也不执行模型返回的代码。
   模型和提示词放在 `--` 后，避免将提示词解释为 CLI 选项。
5. 实验版 CLI 会将模型输出保存为带时间戳的 PNG，且没有稳定的输出路径参数。
   所以为每次推理创建工件目录下的唯一临时目录，并设置为 CLI 工作目录。
6. 忽略可被提示词影响的 stdout 文件名；直接枚举该目录，要求恰好一张 PNG，并解码校验尺寸。
7. 临时目录与目标位于同一文件系统，使用 hard_link 发布，目标存在时原子失败，不覆盖并发写入。
8. 全部成功后保存 result.json；失败保留日志及临时文件，不报告出图成功。

CLI stdout/stderr 分别写入文件，避免 pipe 缓冲区塞满导致死锁。
主进程通过 try_wait 轮询并在超时后 kill/wait 回收 CLI；这不保证 Ollama 服务端立即取消推理，
因此不自动重试。服务控制设备与精度，本程序不暴露无法映射的 Python 参数。
关闭终端内联图像环境变量，避免 CLI 将整张图 base64 输出到日志。

## 官方实现依据

实现时（2026-11-17）核对：

- Ollama 官方 `Image generation (experimental)`，2026-01-20。
- `ollama/ollama` 标签 `v0.15.0`：`cmd/cmd.go`、`x/imagegen/cli.go`。
- 同标签 GitHub Release 的 `ollama-darwin.tgz`，SHA-256：
  `bf3bc2f9472d83fdee3d0317f5dcb4ecea637c9bfc7d400a7c269acbcdcb49da`。
- `v0.40.16/cmd/cmd.go` 中图像生成不受支持的明确错误路径。

不使用未核实的 `ollama run --output`，也不将图像理解模型当作文生图模型。
官方版本或注册表变化后，应重新验证兼容版本，而非取消能力检查或回退到 Python。

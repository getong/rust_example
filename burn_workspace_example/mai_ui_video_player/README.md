# MAI-UI 视频播放器界面设计

参考 `mai_ui_unreal` 的 Rust 客户端，实现独立的视频播放器设计主题。默认通过本地 Ollama 的 OpenAI 兼容接口生成单文件 HTML/CSS 视觉预览，也支持 PNG 截图定位。

## 默认设计

- 中文桌面播放器「映刻 · FRAME」：石墨黑、冰蓝强调色，1440×900 基准布局。
- 正在播放：16:9 封面、影片信息、播放控制栏、时间与进度、音量、倍速、字幕、画质、播放列表及章节。
- 媒体库：最近观看、收藏、本地视频和观看进度。
- 播放设置：原生选择框、滑块、字幕及主题选项的控件状态演示。
- 页面锚点切换、键盘焦点和响应式布局；窄窗口将播放列表移到播放器下方。

这是 **UI 设计实验，不是真实视频播放器**：没有媒体加载、解码或播放功能。生成要求禁用 JavaScript 和外部资源，播放、全屏、画中画等不可用操作应禁用或标注为预览；滑块和选择框仅改变本地控件状态。MAI-UI 的官方用途是 GUI 操作，设计生成属于实验任务，模型不一定完全遵循设计要求，生成后仍需在浏览器检查视觉效果与交互。

## 运行

在工作区根目录运行，确保 Ollama 已启动，且模型已安装；可用 `--url`、`--model` 指定其他兼容服务和模型。

```bash
cargo run -p mai_ui_video_player -- --mode design
```

默认模型为 `Maternion/mai-ui:8b`，接口为 `http://localhost:11434/v1/chat/completions`，单次输出上限 16384 tokens。连接超时 30 秒，完整请求等待上限 2 小时；如果输出因 token 上限截断，自动精简重试一次，不会拼接或展示不完整 HTML。

默认输出在本包的 `design-output/`，与 `mai_ui_unreal` 的输出隔离：

- `request.json`、`request.attempt-N.json`：初始请求与各轮实际请求。
- `response.json`、`response.attempt-N.json`：最终响应与各轮原始响应。
- `result.html`：通过完整性检查后写出的视觉预览，包含阻止脚本与外部资源的 CSP。

`--output` 可自定义输出目录，相对路径基于运行时工作目录。完整性检查不是完整的 HTML 校验或清洗器；仅打开可信服务的输出。

### 自定义设计

```bash
cargo run -p mai_ui_video_player -- \
  --brief '设计一个浅色极简视频播放器，包含正在播放、播放列表和字幕设置，中文界面' \
  --model 'Maternion/mai-ui:8b' \
  --max-tokens 16384 \
  --output mai_ui_video_player/design-output
```

### 离线复用响应

```bash
cargo run -p mai_ui_video_player -- \
  --response mai_ui_video_player/design-output/response.json \
  --output mai_ui_video_player/design-output/replay
```

该模式不调用 API，仍会检查响应并生成 `result.html`。`--brief` 不会修改已经生成的响应内容。

### 截图定位

```bash
cargo run -p mai_ui_video_player -- --mode grounding \
  --image /path/to/player.png \
  --target '找到视频播放器的播放/暂停按钮，返回按钮中心的坐标。'
```

默认输出在本包的 `demo-output/`，HTML 标出模型预测坐标。截图需自行提供；定位模式不会点击按钮或操作播放器。

## 检查

```bash
cargo test -p mai_ui_video_player
cargo fmt -p mai_ui_video_player -- --check
```

测试使用构造的响应，不依赖 Ollama 或真实视频。

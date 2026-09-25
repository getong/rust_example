# MAI-UI Rust 客户端：App 设计实验与截图定位

## Ollama 服务

```
ollama serve
ollama run Maternion/mai-ui:8b
```

默认连接 `http://localhost:11434/v1/chat/completions`，模型为本机已安装的 `Maternion/mai-ui:8b`。使用 Ollama 的 [OpenAI 兼容接口](https://docs.ollama.com/api/openai-compatibility)，文本与图片请求均保留 Chat Completions 格式，无需 API key。

```sh
ollama list
cargo run -p mai_ui_client -- --mode design --model Maternion/mai-ui:8b
# 其他兼容服务需同时指定完整 URL 和服务端模型名
cargo run -p mai_ui_client -- --url http://localhost:30000/v1/chat/completions --model MAI-UI-8B
```

`--model` 同时作用于设计和截图定位模式。截图定位需要支持视觉的模型。

## App 界面设计实验（默认模式）

MAI-UI 的官方定位是 GUI 操作智能体，主要用于理解已有界面、控件定位及导航，不是专门的 UI 设计生成模型。参见 [官方说明](https://github.com/Tongyi-MAI/MAI-UI/blob/main/MAI-UI/README.md)。此前手机桌面效果来自输入的 `official-screen.png`，不是模型生成的设计。

现在默认使用独立的文本到 HTML 设计任务，不读取手机截图、不解析坐标。新提示词在 `src/design.rs`，要求输出具体 App 内部页面的完整 HTML/CSS，包括中文文案、清晰层级、主操作和导航。默认主题是旅行规划 App「远行」，390 × 844 的移动端界面。

```sh
cargo run -p mai_ui_client -- --mode design
cargo run -p mai_ui_client -- --mode design --brief "设计一个中文任务管理 App 首页，包含今日任务、进度和新建任务按钮"
```

输出位于 `design-output/request.json`、`response.json`、`result.html`。网页是模型实际返回的 HTML/CSS；Rust 只校验完整性并注入静态预览的内容安全策略，不用预制界面替换模型结果。设计模式属于能力实验，模型可能输出无效格式或较差设计；遇到错误会保留响应并报告失败。HTML 中的按钮仅为视觉原型。

本地实测（2026-09-25）：通过 Ollama 的 `Maternion/mai-ui:8b`，默认旅行 App 设计请求约 18.5 秒成功生成完整 HTML；使用本地登录页 PNG 的截图定位请求约 5.8 秒返回坐标并生成标注网页。这验证了文本和图片请求链路，未评估设计质量或定位精度。客户端超时不代表服务端推理已停止，重试前应检查服务状态。

查看 `result.html`，或用 Chrome 截取效果图（路径替换为当前项目的绝对路径）：

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless \
  --screenshot=/absolute/path/design-output/preview.png --window-size=390,844 \
  --hide-scrollbars --force-device-scale-factor=1 file:///absolute/path/design-output/result.html
```

复用设计响应：`cargo run -p mai_ui_client -- --mode design --response design-output/response.json`。

## 原始截图定位模式

参考 `huggingface-files/examples/demo_grounding.py` 实现的纯 Rust CLI。读取 PNG，将截图以 base64 发送到 MAI-UI-8B 的 Chat Completions 接口，解析目标中心坐标，生成可离线打开的红圈标注网页。只定位，不执行点击；模型由独立的 API 服务运行。

系统提示词位于 `src/grounding.rs` 的 `GROUNDING_PROMPT`：要求 MAI-UI 根据截图中的外观、文字和布局寻找目标，返回可点击区域的中心；定位应用图标时取图标本身的中心，不取下方文字。坐标按整张截图归一化到 0–999，找不到或无法唯一确定时返回 `null`，只输出约定的 `<answer>` JSON。

这个演示展示的是“自然语言描述 → 截图中的位置”。此前旧响应给出 Chrome 的归一化坐标 `[616, 829]`，在 450 × 1000 截图上换算为 `(277, 830)`；打开生成的网页，检查红圈是否落在目标图标中心，即可判断这一次定位的效果。该响应来自原 Python 示例，不代表新提示词的实测效果；新提示词是否改善准确率，需要实际调用模型并对多张截图验证。使用 `--response` 仅重放旧结果，不会让模型执行新提示词。

在本项目目录调用真实 API（默认 `http://localhost:11434/v1/chat/completions`）：

```sh
cargo run -p mai_ui_client -- --mode grounding --image examples/official-screen.png \
  --target "找到 Chrome 浏览器图标，返回图标中心的坐标。"
```

如果本地保留了原始示例截图及响应，可无需启动模型进行重放：

```sh
cargo run -p mai_ui_client -- --mode grounding --response examples/response.json
```

自定义服务、截图和输出目录：

```sh
cargo run -p mai_ui_client -- --mode grounding --url http://localhost:11434/v1/chat/completions \
  --image /path/to/screenshot.png --target "找到设置按钮" --output demo-output
cargo run -p mai_ui_client -- --help
```

输出文件：`request.json`（含截图）、`response.json`（完整 API 响应）、`result.html`（内嵌截图及标注，可直接用浏览器打开）。默认路径相对于运行时的当前目录。

定位模式默认使用 Ollama 模型 `Maternion/mai-ui:8b`，生成参数为：`temperature=0`、`max_tokens=128`、禁用流式输出、绕过代理；客户端超时延长为 1800 秒。接受正常结束标签和重复的 `<answer>`。坐标必须是 0 到 999 的两个整数，使用原示例的像素换算与边缘裁剪。目标不存在、输出截断、坐标无效或请求失败时返回非零退出码；预测校验失败前会保存 JSON 响应，不生成新网页。重复使用输出目录时，失败不会更新以前的 `result.html`，请以本次终端结果为准。

`examples/` 不纳入版本控制，当前检出不包含示例截图和响应。截图定位时请用 `--image /path/to/screenshot.png` 提供 PNG；使用 `--response` 时也必须提供该响应对应的原图。单元测试无需这些外部文件。

验证：

```sh
cargo test -p mai_ui_client
cargo clippy -p mai_ui_client --all-targets -- -D warnings
cargo fmt -p mai_ui_client --check
```

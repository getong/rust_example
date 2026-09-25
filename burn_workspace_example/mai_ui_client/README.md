# MAI-UI Rust 截图定位客户端

参考 `huggingface-files/examples/demo_grounding.py` 实现的纯 Rust CLI。读取 PNG，将截图以 base64 发送到 MAI-UI-8B 的 Chat Completions 接口，解析目标中心坐标，生成可离线打开的红圈标注网页。只定位，不执行点击；模型由独立的 API 服务运行。

系统提示词位于 `src/grounding.rs` 的 `GROUNDING_PROMPT`：要求 MAI-UI 根据截图中的外观、文字和布局寻找目标，返回可点击区域的中心；定位应用图标时取图标本身的中心，不取下方文字。坐标按整张截图归一化到 0–999，找不到或无法唯一确定时返回 `null`，只输出约定的 `<answer>` JSON。

这个演示展示的是“自然语言描述 → 截图中的位置”。附带的旧响应给出 Chrome 的归一化坐标 `[616, 829]`，在 450 × 1000 截图上换算为 `(277, 830)`；打开生成的网页，检查红圈是否落在目标图标中心，即可判断这一次定位的效果。该响应来自原 Python 示例，不代表新提示词的实测效果；新提示词是否改善准确率，需要实际调用模型并对多张截图验证。使用 `--response` 仅重放旧结果，不会让模型执行新提示词。

在本项目目录调用真实 API（默认 `http://localhost:30000/v1/chat/completions`）：

```sh
cargo run -p mai_ui_client -- --image examples/official-screen.png \
  --target "找到 Chrome 浏览器图标，返回图标中心的坐标。"
```

无需启动模型，复用随项目附带的原始示例响应：

```sh
cargo run -p mai_ui_client -- --response examples/response.json
```

自定义服务、截图和输出目录：

```sh
cargo run -p mai_ui_client -- --url http://localhost:30000/v1/chat/completions \
  --image /path/to/screenshot.png --target "找到设置按钮" --output demo-output
cargo run -p mai_ui_client -- --help
```

输出文件：`request.json`（含截图）、`response.json`（完整 API 响应）、`result.html`（内嵌截图及标注，可直接用浏览器打开）。默认路径相对于运行时的当前目录。

与 Python 示例一致：固定模型 `MAI-UI-8B`、`temperature=0`、`max_tokens=128`、禁用流式输出、绕过代理、600 秒超时；接受正常结束标签和重复的 `<answer>`。坐标必须是 0 到 999 的两个整数，使用原示例的像素换算与边缘裁剪。目标不存在、输出截断、坐标无效或请求失败时返回非零退出码；预测校验失败前会保存 JSON 响应，不生成新网页。重复使用输出目录时，失败不会更新以前的 `result.html`，请以本次终端结果为准。

`examples/official-screen.png` 和 `examples/response.json` 复制自用户提供的示例目录，离线响应仅对应此截图和默认目标。

验证：

```sh
cargo test -p mai_ui_client
cargo clippy -p mai_ui_client --all-targets -- -D warnings
cargo fmt -p mai_ui_client --check
```

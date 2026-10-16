# Ratatui 交互示例

本示例有终端版和 GPU 窗口版，共用 `App::draw` 与 `App::handle`，包含按钮、Tooltip 和模态弹窗。

架构与算法说明：[Ratatui 与 wgpu 如何协作：从字符网格到 GUI](docs/ratatui-wgpu.md)。包含差分网格、中文双宽字符、GPU 绘制流程、输入坐标转换及性能边界。

终端版：

```sh
cargo run
```

原生窗口版（不需要终端模拟器）：

```sh
cargo run --features gui --bin gui
```

窗口版使用自定义 `ratatui::backend::Backend` 保存字符网格，再由 eframe / egui 的画笔提交给 wgpu 绘制；macOS 上使用 Metal。没有用 egui 重写按钮和弹窗。窗口事件转换为同一套 Crossterm 事件，再交给共享应用。默认终端构建不编译 GUI 依赖。

窗口版自动查找 macOS、Windows、Linux 常见位置的中文字体，也可指定：

```sh
RATATUI_GUI_FONT=/path/to/chinese-font.ttf cargo run --features gui --bin gui
```

支持 TTF / OTF / TTC（集合中的第一张字面须包含所需字符）。字体未随程序打包；找不到字体时会报告错误。窗口缩放与鼠标坐标统一使用逻辑像素，以适配高 DPI 显示器。窗口过小时显示调整尺寸提示。

请使用支持 UTF-8 / 中文、鼠标事件的终端，窗口至少 52 列 × 22 行。鼠标悬停支持取决于终端，键盘始终可用。计数仅保存在内存中。

| 操作 | 鼠标 | 键盘 |
| --- | --- | --- |
| 选择按钮 / 查看提示 | 悬停 | Tab、Shift+Tab、方向键 |
| 执行按钮 | 左键点击 | Enter、空格 |
| 增加计数 | 点击增加计数 | `+` 或 `=` |
| 打开重置确认框 | 点击重置计数 | `r` |
| 关于弹窗 | 点击关于 | `?` |
| 关闭弹窗 | 点击取消 / 知道了 | Esc |
| 退出 | 点击退出 | 主界面 `q` / Esc，任意界面 Ctrl+C |

重置弹窗默认选中“取消”，Tab 或左右键选择“确认重置”。弹窗打开时背景按钮和快捷键不会响应。

代码结构：

- `src/app.rs`：共享状态、布局、按钮命中检测、焦点、提示和弹窗。
- `src/main.rs`：终端入口和事件循环。
- `src/bin/gui.rs`：窗口入口。
- `src/gui/grid.rs`：自定义 Backend、增量网格和中文双宽字符清理。
- `src/gui/mod.rs`：窗口生命周期、字体、缩放和共享应用驱动。
- `src/gui/input.rs`：窗口键盘和鼠标事件转换。
- `src/gui/paint.rs`：网格背景、文字、颜色和修饰绘制。

架构参考本机 Obelus 的 `crates/obelus-gui/src/grid.rs` 和 `main.rs`。这里的实现独立编写，使用单线程窗口循环和 eframe/wgpu，未引入 Obelus 的业务模块、通道或字体资源。支持本示例使用的样式；不是完整终端模拟器，尚未实现斜体、闪烁、输入法编辑和文本选择。

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

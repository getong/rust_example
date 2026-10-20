# egui 基本用法示例

这是一个基于 `egui 0.37` 和 `eframe 0.37` 的最小可运行示例，用来演示 egui 的常见写法。

## 运行

```bash
cargo run
```

## 示例内容

- `Panel::top`、`Panel::left`、`CentralPanel`：搭建窗口整体布局
- `label`、`button`、`checkbox`、`radio_value`：基础控件和交互
- `TextEdit`、`Slider`、`ComboBox`、颜色选择器：输入控件
- `ProgressBar`、`Grid`、`ScrollArea`、`CollapsingHeader`、`Frame`：常用布局与容器
- `Window`：创建可移动、可调整大小的独立窗口
- `Response::clicked`：根据控件响应更新应用状态

## egui 的特点

egui 是 Rust 编写的即时模式 GUI 库。应用每一帧通过 Rust 代码描述当前界面，egui 根据应用状态生成控件并返回交互响应，因此不需要手动维护传统 GUI 的控件树。它的 API 组合性强、跨平台，并且适合工具、编辑器、调试面板等场景。

`egui` 主要负责 UI 和交互；本示例使用 `eframe` 提供原生窗口、事件循环和渲染集成。

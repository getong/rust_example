# Taffy + GPUI Kit

这个示例把 Taffy 的外部布局树与 `gpui-kit` 的组件系统组合起来。
`gpui-kit` 使用 `gpui-fast` feature，因此所有 GPUI 类型、平台初始化和组件都来自同一套 GPUI Fast 后端。

## 运行

在当前目录执行：

```sh
cargo run -p gpui_fast_example
```

窗口包含由 Taffy 排布的侧栏、标题区和三个卡片；第一个卡片内的 GPUI Kit 按钮可以递增计数。
调整窗口大小会按新的 viewport 重新计算 Taffy 布局。

## 实现边界

- `Cargo.toml`：`gpui-kit` 开启 `gpui-fast`，并将外部 Taffy 锁定到 GPUI Fast 使用的 `0.14.0`。
- `src/main.rs`：Taffy 布局树、矩形到 GPUI 元素的映射、Kit Root 和按钮交互。
- Taffy 只计算外层节点的 `x/y/width/height`。
- GPUI Kit 负责组件样式、文本、按钮事件和最终绘制。

修改状态后调用 `cx.notify()`，让 GPUI Fast 更新保留的视图。
可以关闭 Retained Mode 对比运行行为：

```sh
GPUI_VIEW_RETENTION=0 cargo run -p gpui_fast_example
```

## 检查

```sh
cargo fmt -p gpui_fast_example -- --check
cargo check -p gpui_fast_example
```

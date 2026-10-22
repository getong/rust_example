# Taffy + GPUI

这个示例验证了 Taffy 可以和 GPUI 协作。运行：

```bash
cargo run
```

## 功能

- Taffy 建立一个 Flexbox 树：根容器、侧栏、内容区、标题区和三个卡片。
- `compute_layout` 计算每个节点的 `x`、`y`、`width` 和 `height`。
- GPUI 创建原生窗口，并把这些矩形转换为绝对定位的 `div`，负责背景、文字和最终绘制。
- 改变 Taffy 的 `gap`、`padding`、`flex_grow` 或固定尺寸后，GPUI 会按照新的矩形显示。

## Taffy + GPUI 的作用

Taffy 是布局引擎，解决 Flexbox、Grid 等“元素应该放在哪里、占多大”的问题；GPUI 是窗口和渲染框架，负责事件、文本、GPU 绘制以及应用生命周期。两者组合后，可以把业务组件树交给 Taffy 计算，再把计算结果映射为 GPUI 元素。

这也是 GPUI 自身采用的基本方向：GPUI 内部已经集成 Taffy 来计算 `div` 元素布局。本示例额外保留了一棵独立的 Taffy 树，让布局输入和输出清晰可见，适合学习“自定义布局引擎 + GPUI 绘制”的边界。

示例固定了 `760 x 460` 的视口，便于直接对照布局结果。真实应用通常会在窗口尺寸变化时重新调用 `compute_layout`，并缓存节点 ID 与布局结果；文本节点还需要通过 Taffy 的 measure 回调接入文本测量。

## 版本说明

本工程使用 `/Users/gerald/test/rust/zed` 中的 GPUI 源码。该 GPUI checkout 依赖 `taffy = 0.13.0`，所以示例也锁定 Taffy `0.13.0`，保证类型和 API 与 GPUI 当前源码一致。参考目录 `/Users/gerald/test/rust/taffy` 的 README 也明确列出了 GPUI/Zed 作为 Taffy 的实际使用者。

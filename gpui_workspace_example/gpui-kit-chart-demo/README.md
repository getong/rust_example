# gpui-kit Chart 演示 / gpui-kit Chart Demo

基于 [gpui-kit](https://crates.io/crates/gpui-kit) `0.11.0` 的 **Chart 图表组件**演示，
参考 gpui-kit Chart 模块（`gpui_kit::component::chart`）。七种图表：

- `LineChart` 折线图
- `BarChart` 柱状图
- `AreaChart` 面积图
- `PieChart` 饼图 / 环形图
- `RadarChart` 雷达图
- `CandlestickChart` K 线图
- `SankeyChart` 桑基图

## 分模块 / Modular

```
src/
├── main.rs                 # 入口：gpui_kit::init + Root 窗口
├── app.rs                  # 根视图：侧边栏切换功能页 + 滚动内容区
├── chart_card.rs           # 统一风格的图表容器 chart_card()
├── palette.rs              # 主题色板助手（chart_1..5 / 自定义调色板）
├── data.rs                 # 共享示例数据集（确定性，可 Clone）
└── features/               # 分功能：每种图表一个模块，每个模块拆多个特性卡片
    ├── mod.rs              # ChartFeature 注册表 all()
    ├── line_chart.rs       # feature = "line"
    ├── bar_chart.rs        # feature = "bar"
    ├── area_chart.rs       # feature = "area"
    ├── pie_chart.rs        # feature = "pie"
    ├── radar_chart.rs      # feature = "radar"
    ├── candlestick_chart.rs# feature = "candlestick"
    └── sankey_chart.rs     # feature = "sankey"
```

## 分功能 / Cargo features

每种图表一个同名 Cargo feature，默认全部启用；模块受 `#[cfg(feature = ...)]` 门控。

```toml
[features]
default = ["line", "bar", "area", "pie", "radar", "candlestick", "sankey"]
```

## 运行 / Run

```bash
# 完整演示
cargo run -p gpui-kit-chart-demo

# 只编译部分图表
cargo run -p gpui-kit-chart-demo --no-default-features --features line,bar

# 仅检查
cargo check -p gpui-kit-chart-demo
```

## 核心用法 / Core API

```rust
use gpui_kit::component::chart::{
    AreaChart, BarChart, CandlestickChart, LineChart, PieChart, RadarChart, SankeyChart,
};

// 折线图：用闭包把数据字段映射到图表维度，链式定制样式与交互
LineChart::new(data)
    .id("revenue")            // ElementId；默认取构造位置，兄弟间需唯一；.interactive(false) 关闭交互
    .name("Desktop")          // tooltip 中的序列名
    .x(|d| d.month.clone())
    .y(|d| d.value)
    .natural()                // 或 .linear() / .step_after()
    .dot()
    .tick_margin(2)
    .stroke(cx.theme().chart_1);
```

图表尺寸由容器决定；图表自动适配主题（`chart_1..chart_5`、`chart_bullish`/`chart_bearish`），
深色/浅色模式自动切换。详见各 `features/*.rs` 模块内的注释。


## 测试 / Tests

```bash
cargo test -p gpui-kit-chart-demo                 # 无头渲染 + 布局测试（test-support）
cargo test -p gpui-kit-chart-demo --test pixels   # 真实离屏像素渲染（macOS Metal / Linux WGPU）
```

- `tests/render.rs`：无头窗口真实渲染每个图表功能页 + 完整应用，断言不 panic、根容器非零尺寸、feature 注册完整；并验证二次渲染稳定。
- `tests/pixels.rs`（`harness = false`）：真实离屏渲染，断言非空白（颜色数 + 非背景像素比例），保存 PNG（`PIXELS_OUT_DIR`，默认 `/tmp`），渲染器不可用时诚实降级 skipped。

### 渲染修正要点

- 图表尺寸由容器决定：图表区给**明确高度** `h(px(300.))` + `w_full()`。
- 卡片 `flex_1 + min_w` 在 `flex_wrap` 网格里自适应、可换行；根 `size_full`、内容区 `overflow_y_scroll`。
- 饼图/雷达图在容器内自动居中，不再额外嵌套居中容器（避免尺寸塌缩）。
- 优先主题色（`chart_1..5`），卡片边框/圆角/背景，窗口标题。

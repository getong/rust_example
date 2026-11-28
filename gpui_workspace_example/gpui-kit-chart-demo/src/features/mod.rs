//! 分功能演示模块 / Feature-split demo modules.
//!
//! 每种图表一个模块，每个模块展示该图表的多个特性；
//! 模块受同名 Cargo feature 门控（`line` / `bar` / `area` / `pie` / `radar` /
//! `candlestick` / `sankey`），可按需裁剪编译。

use gpui_kit::{AnyElement, App};

#[cfg(feature = "area")]
pub mod area_chart;
#[cfg(feature = "bar")]
pub mod bar_chart;
#[cfg(feature = "candlestick")]
pub mod candlestick_chart;
#[cfg(feature = "line")]
pub mod line_chart;
#[cfg(feature = "pie")]
pub mod pie_chart;
#[cfg(feature = "radar")]
pub mod radar_chart;
#[cfg(feature = "sankey")]
pub mod sankey_chart;

/// 一个可切换展示的图表功能页。
pub struct ChartFeature {
  /// 稳定标识，用作 ElementId 与侧边栏 key。
  pub id: &'static str,
  /// 侧边栏标题。
  pub title: &'static str,
  /// 一句话描述。
  pub description: &'static str,
  /// 渲染该功能页。
  pub render: fn(&App) -> AnyElement,
}

/// 已启用的功能列表（顺序即侧边栏顺序）。
#[allow(clippy::vec_init_then_push)] // push 受 `#[cfg(feature)]` 门控
pub fn all() -> Vec<ChartFeature> {
  let mut features = Vec::new();

  #[cfg(feature = "line")]
  features.push(ChartFeature {
    id: "line",
    title: "LineChart 折线图",
    description: "插值变体（natural/linear/step_after）、数据点、刻度密度、tooltip、主题色",
    render: line_chart::render,
  });

  #[cfg(feature = "bar")]
  features.push(ChartFeature {
    id: "bar",
    title: "BarChart 柱状图",
    description: "自定义填充、渐变、BarAlignment、圆角、负值、数值轴",
    render: bar_chart::render,
  });

  #[cfg(feature = "area")]
  features.push(ChartFeature {
    id: "area",
    title: "AreaChart 面积图",
    description: "渐变填充、多序列堆叠、插值变体、tooltip",
    render: area_chart::render,
  });

  #[cfg(feature = "pie")]
  features.push(ChartFeature {
    id: "pie",
    title: "PieChart 饼图与环形图",
    description: "环形图 inner_radius、自定义配色、pad_angle、引导线标签",
    render: pie_chart::render,
  });

  #[cfg(feature = "radar")]
  features.push(ChartFeature {
    id: "radar",
    title: "RadarChart 雷达图",
    description: "多序列、自定义项、网格环、最大值、元素标签",
    render: radar_chart::render,
  });

  #[cfg(feature = "candlestick")]
  features.push(ChartFeature {
    id: "candlestick",
    title: "CandlestickChart K 线图",
    description: "OHLC、body_width_ratio、tick_margin、阳线/阴线配色",
    render: candlestick_chart::render,
  });

  #[cfg(feature = "sankey")]
  features.push(ChartFeature {
    id: "sankey",
    title: "SankeyChart 桑基图",
    description: "节点对齐、样式配置、多行标签、SankeyValueScale::Sqrt",
    render: sankey_chart::render,
  });

  features
}

//! 桑基图 SankeyChart：节点/链接 / 节点对齐 / 样式 / 多行标签 / SankeyValueScale。
//!
//! 可视化节点间的流量，布局算法镜像 d3-sankey；
//! **链接通过节点在 `nodes` 列表中的下标引用节点**，需先把 id 映射为索引。
//! `SankeyLink::new(source, target, value)`。

use gpui_kit::{
  AnyElement, App, IntoElement, ParentElement, SharedString, Styled,
  component::{
    ActiveTheme,
    chart::{SankeyChart, SankeyLabel},
    h_flex,
    plot::shape::{SankeyAlign, SankeyLink, SankeyValueScale},
    v_flex,
  },
  px,
};

use crate::{chart_card::chart_card, data::FlowNode, palette};

/// 财务报表节点 + 链接（链接按节点下标引用）。
fn finance_flow() -> (Vec<FlowNode>, Vec<SankeyLink>) {
  let names = [
    "Revenue",
    "Gross Profit",
    "Cost",
    "Operating Income",
    "Expenses",
    "Net Income",
    "Tax",
  ];
  let nodes = names
    .iter()
    .enumerate()
    .map(|(i, name)| FlowNode {
      name: name.to_string(),
      color_index: i,
    })
    .collect();

  // 0 Revenue -> 1 Gross Profit / 2 Cost
  // 1 Gross Profit -> 3 Operating Income
  // 3 Operating Income -> 5 Net Income / 4 Expenses
  // 5 Net Income -> 6 Tax
  let links = vec![
    SankeyLink::new(0, 1, 45.0),
    SankeyLink::new(0, 2, 55.0),
    SankeyLink::new(1, 3, 45.0),
    SankeyLink::new(3, 5, 32.0),
    SankeyLink::new(3, 4, 13.0),
    SankeyLink::new(5, 6, 8.0),
  ];
  (nodes, links)
}

/// 值域跨度很大的流量（200:1），用于 SankeyValueScale::Sqrt 演示。
fn wide_range_flow() -> (Vec<FlowNode>, Vec<SankeyLink>) {
  let nodes = ["Source", "Big", "Small", "Sink"]
    .into_iter()
    .enumerate()
    .map(|(i, name)| FlowNode {
      name: name.to_string(),
      color_index: i,
    })
    .collect();
  let links = vec![
    SankeyLink::new(0, 1, 100.0),
    SankeyLink::new(0, 2, 1.0),
    SankeyLink::new(1, 3, 100.0),
    SankeyLink::new(2, 3, 1.0),
  ];
  (nodes, links)
}

pub fn render(cx: &App) -> AnyElement {
  let theme = cx.theme();
  let grid = || h_flex().flex_wrap().gap_4().w_full();

  v_flex()
    .gap_4()
    .w_full()
    .child(
      grid()
        // 1. 基础桑基图 + 标签
        .child(chart_card(
          cx,
          "基础桑基图",
          "SankeyChart::new(nodes, links).node_label(..).value_label(..)；链接按下标引用节点",
          {
            let (nodes, links) = finance_flow();
            SankeyChart::new(nodes, links)
              .interactive(false)
              .id("sankey-basic")
              .node_label(|d| SharedString::from(d.name.clone()))
              .value_label(|_, value| SharedString::from(format!("{value:.1}")))
          },
        ))
        // 2. 节点对齐 SankeyAlign
        .child(chart_card(
          cx,
          "节点对齐 SankeyAlign::Left",
          ".node_align(SankeyAlign::Left)；也可 Right / Center / Justify（默认）",
          {
            let (nodes, links) = finance_flow();
            SankeyChart::new(nodes, links)
              .interactive(false)
              .id("sankey-align")
              .node_align(SankeyAlign::Left)
              .node_label(|d| SharedString::from(d.name.clone()))
          },
        ))
        // 3. 样式配置
        .child(chart_card(
          cx,
          "样式配置",
          ".node_width/.node_padding/.node_corner_radius/.node_color/.link_opacity/.\
           min_link_width/.iterations",
          {
            let (nodes, links) = finance_flow();
            let colors = palette::chart_colors(cx);
            SankeyChart::new(nodes, links)
              .interactive(false)
              .id("sankey-style")
              .node_width(8.0)
              .node_padding(20.0)
              .node_corner_radius(px(2.0))
              .node_color(move |d| colors[d.color_index % colors.len()])
              .link_opacity(0.4)
              .min_link_width(2.0)
              .iterations(10)
              .node_label(|d| SharedString::from(d.name.clone()))
          },
        ))
        // 4. 自定义多行标签 SankeyLabel
        .child(chart_card(
          cx,
          "自定义多行标签 SankeyLabel",
          ".labels(|d, value| vec![SankeyLabel::new(..).color(..).font_size(..)])；优先于 \
           node_label/value_label",
          {
            let (nodes, links) = finance_flow();
            let muted = theme.muted_foreground;
            SankeyChart::new(nodes, links)
              .interactive(false)
              .id("sankey-labels")
              .labels(move |d, value| {
                vec![
                  SankeyLabel::new(format!("{value:.1}")).font_size(11.0),
                  SankeyLabel::new(d.name.clone())
                    .color(muted)
                    .font_size(10.0),
                ]
              })
          },
        )),
    )
    .child(
      grid()
        // 5. 压缩大数值范围 SankeyValueScale::Sqrt
        .child(chart_card(
          cx,
          "SankeyValueScale::Sqrt 压缩大数值范围",
          ".value_scale(SankeyValueScale::Sqrt)；值域跨度 200:1 时小流量保持可见，标签仍收到原始值",
          {
            let (nodes, links) = wide_range_flow();
            SankeyChart::new(nodes, links)
              .interactive(false)
              .id("sankey-sqrt")
              .value_scale(SankeyValueScale::Sqrt)
              .node_label(|d| SharedString::from(d.name.clone()))
              .value_label(|_, value| SharedString::from(format!("{value:.0}")))
          },
        ))
        // 6. 线性缩放对照
        .child(chart_card(
          cx,
          "SankeyValueScale::Linear（默认）对照",
          "节点高度与流量线性相关；大流量会压缩小流量",
          {
            let (nodes, links) = wide_range_flow();
            SankeyChart::new(nodes, links)
              .interactive(false)
              .id("sankey-linear")
              .value_scale(SankeyValueScale::Linear)
              .node_label(|d| SharedString::from(d.name.clone()))
              .value_label(|_, value| SharedString::from(format!("{value:.0}")))
          },
        )),
    )
    .into_any_element()
}

//! 雷达图 RadarChart：多序列 / 自定义项 / 网格环 / 最大值 / 元素标签。
//!
//! 每个数据项代表一个维度（辐条），从 12 点钟方向顺时针分布；
//! 每次 `.value()` 调用添加一个序列，与紧随的 `.stroke()`/`.fill()` 配对。
//! 雷达图在容器内自动居中；外环默认为图表高度的 40%。

use gpui_kit::{
  AnyElement, App, IntoElement, ParentElement, Styled,
  component::{
    ActiveTheme,
    chart::{RadarChart, RadarLabel},
    h_flex, v_flex,
  },
  div,
};

use crate::{chart_card::chart_card, data};

pub fn render(cx: &App) -> AnyElement {
  let theme = cx.theme();
  let grid = || h_flex().flex_wrap().gap_4().w_full();

  v_flex()
    .gap_4()
    .w_full()
    .child(
      grid()
        // 1. 基础雷达图
        .child(chart_card(
          cx,
          "基础雷达图",
          "RadarChart::new(data).label(..).value(..)；本版本颜色按主题 chart 色板逐序列循环",
          RadarChart::new(data::radar_dimensions())
            .id("radar-basic")
            .label(|d| d.dimension.clone())
            .value(|d| d.alpha),
        ))
        // 2. 多序列 + 逐序列配色
        .child(chart_card(
          cx,
          "多序列 + 逐序列 stroke/fill",
          "每次 .value() 添加一个序列；.name() 命名序列用于 tooltip",
          RadarChart::new(data::radar_dimensions())
            .id("radar-multi")
            .label(|d| d.dimension.clone())
            .value(|d| d.alpha)
            .name("Alpha")
            .stroke(theme.chart_1)
            .fill(theme.chart_1.opacity(0.2))
            .value(|d| d.beta)
            .name("Beta")
            .stroke(theme.chart_2)
            .fill(theme.chart_2.opacity(0.2)),
        ))
        // 3. 自定义项：dot + grid_levels + max_value + outer_radius
        .child(chart_card(
          cx,
          "自定义项 dot + grid_levels + max_value",
          ".dot().max_value(100.).grid_levels(5).outer_radius(110.)",
          RadarChart::new(data::radar_dimensions())
            .id("radar-custom")
            .label(|d| d.dimension.clone())
            .value(|d| d.alpha)
            .stroke(theme.chart_2)
            .fill(theme.chart_2.opacity(0.2))
            .dot()
            .max_value(100.0)
            .grid_levels(5)
            .outer_radius(110.0),
        ))
        // 4. 隐藏网格
        .child(chart_card(
          cx,
          "隐藏网格",
          ".grid(false)",
          RadarChart::new(data::radar_dimensions())
            .id("radar-nogrid")
            .label(|d| d.dimension.clone())
            .value(|d| d.beta)
            .stroke(theme.chart_3)
            .fill(theme.chart_3.opacity(0.25))
            .grid(false),
        )),
    )
    .child(
      grid()
        // 5. 元素标签：自定义 AnyElement
        .child(chart_card(
          cx,
          "元素标签 RadarLabel::Element",
          "label 闭包返回 .into_any_element()；自带样式，label_color 不生效，不提供 tooltip 标题",
          {
            let foreground = theme.foreground;
            let muted = theme.muted_foreground;
            RadarChart::new(data::radar_dimensions())
              .id("radar-element-label")
              .label(move |d| {
                RadarLabel::Element(
                  v_flex()
                    .items_center()
                    .child(
                      div()
                        .text_xs()
                        .text_color(foreground)
                        .child(d.dimension.clone()),
                    )
                    .child(
                      div()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("{:.0}", d.alpha)),
                    )
                    .into_any_element(),
                )
              })
              .value(|d| d.alpha)
              .outer_radius(95.0)
          },
        ))
        // 6. 标签颜色 + label_gap
        .child(chart_card(
          cx,
          "label_color + label_gap",
          ".label_color(..).label_gap(..)；字符串标签支持 label_color",
          RadarChart::new(data::radar_dimensions())
            .id("radar-label-color")
            .label(|d| d.dimension.clone())
            .value(|d| d.alpha)
            .label_color(theme.muted_foreground)
            .label_gap(14.0),
        )),
    )
    .into_any_element()
}

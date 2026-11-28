//! 饼图与环形图 PieChart：value 闭包 / outer_radius / inner_radius / 自定义配色 / pad_angle /
//! 引导线标签。饼图在容器内自动居中，图表尺寸由 `chart_card` 给定的容器决定。

use gpui_kit::{
  AnyElement, App, IntoElement, ParentElement, SharedString, Styled,
  component::{ActiveTheme, chart::PieChart, h_flex, v_flex},
};

use crate::{chart_card::chart_card, data, palette};

/// 饼图数据：市场份额。
fn market_share() -> Vec<data::CategoryValue> {
  [
    ("Desktop", 52.0),
    ("Mobile", 31.0),
    ("Tablet", 11.0),
    ("Other", 6.0),
  ]
  .into_iter()
  .map(|(category, value)| data::CategoryValue {
    category: category.to_string(),
    value,
  })
  .collect()
}

pub fn render(cx: &App) -> AnyElement {
  let theme = cx.theme();
  let muted_foreground = theme.muted_foreground;
  let foreground = theme.foreground;
  let grid = || h_flex().flex_wrap().gap_4().w_full();

  v_flex()
    .gap_4()
    .w_full()
    .child(
      grid()
        // 1. 基础饼图
        .child(chart_card(
          cx,
          "基础饼图",
          "PieChart::new(data).value(|d| ..f32).outer_radius(..)；本版本未设 color 时各片用 \
           chart_2，见下方自定义配色",
          PieChart::new(market_share())
            .id("pie-basic")
            .name("Market share")
            .value(|d| d.value as f32)
            .outer_radius(90.0),
        ))
        // 2. 环形图 donut
        .child(chart_card(
          cx,
          "环形图 donut",
          ".inner_radius(55.) 产生环形效果；另有 inner_radius_ratio / inner_radius_fn",
          PieChart::new(market_share())
            .id("pie-donut")
            .name("Market share")
            .value(|d| d.value as f32)
            .outer_radius(90.0)
            .inner_radius(55.0),
        ))
        // 3. 自定义配色 + pad_angle
        .child(chart_card(
          cx,
          "自定义配色 + pad_angle",
          ".color(|d| ..) 逐片取色；.pad_angle(4./100.) 4% 间距",
          {
            let colors = palette::chart_colors(cx);
            PieChart::new(market_share())
              .id("pie-color")
              .name("Market share")
              .value(|d| d.value as f32)
              .outer_radius(90.0)
              .inner_radius(45.0)
              .pad_angle(4.0 / 100.0)
              .color(move |d| {
                let idx = match d.category.as_str() {
                  "Desktop" => 0,
                  "Mobile" => 1,
                  "Tablet" => 2,
                  _ => 3,
                };
                colors[idx % colors.len()]
              })
          },
        ))
        // 4. 引导线 + 外部标签
        .child(chart_card(
          cx,
          "引导线 + 外部标签",
          ".label(..) 在环外绘制“引导线 + 文本”；.label_gap 默认 \
           15px；.label_color/.label_line_color",
          PieChart::new(market_share())
            .id("pie-label")
            .name("Market share")
            .value(|d| d.value as f32)
            .outer_radius(70.0)
            .label(|d| SharedString::from(format!("{} {:.0}%", d.category, d.value)))
            .label_color(foreground)
            .label_line_color(move |_| muted_foreground)
            .label_gap(12.0),
        )),
    )
    .child(
      grid()
        // 5. tooltip 自定义
        .child(chart_card(
          cx,
          "tooltip 自定义",
          ".tooltip_name(..).tooltip_value(..)；value 闭包收到 (datum, slice_value, total)",
          PieChart::new(market_share())
            .id("pie-tooltip")
            .name("Market share")
            .value(|d| d.value as f32)
            .outer_radius(90.0)
            .inner_radius(50.0)
            .tooltip_name(|d| SharedString::from(d.category.clone()))
            .tooltip_value(|d, value, total| {
              SharedString::from(format!(
                "{}: {:.0} ({:.1}%)",
                d.category,
                value,
                value / total * 100.0
              ))
            }),
        ))
        // 6. 逐片半径 outer_radius_fn
        .child(chart_card(
          cx,
          "逐片半径 outer_radius_fn",
          ".outer_radius_fn(|arc| ..) 按扇区决定半径；突出最大片",
          PieChart::new(market_share())
            .id("pie-radius-fn")
            .name("Market share")
            .value(|d| d.value as f32)
            .outer_radius(90.0)
            .outer_radius_fn(|arc| if arc.value >= 50.0 { 100.0 } else { 80.0 }),
        ))
        // 7. 默认外半径 = 高度 40%
        .child(chart_card(
          cx,
          "默认外半径（高度 40%）",
          "outer_radius 为 0 时用图表高度的 40%；显式设置更可控",
          PieChart::new(market_share())
            .id("pie-default-radius")
            .name("Market share")
            .value(|d| d.value as f32),
        )),
    )
    .into_any_element()
}

//! 柱状图 BarChart：分类轴/数值轴映射 / 自定义填充 / 渐变 / 对齐 / 圆角 / 负值 / 数值轴。
//!
//! 分类轴（band 轴）与数值轴（value 轴）分别由 `.band()` 和 `.value()` 映射；
//! `BarAlignment` 控制柱条方向与基线所在侧；负值自动从零线向另一侧延伸。

use gpui_kit::{
  AnyElement, App, Corners, IntoElement, ParentElement, Styled,
  component::{ActiveTheme, chart::BarChart, h_flex, plot::shape::BarAlignment, v_flex},
  linear_color_stop, px,
};

use crate::{chart_card::chart_card, data, palette};

pub fn render(cx: &App) -> AnyElement {
  let theme = cx.theme();
  let chart_3 = theme.chart_3;
  let chart_1 = theme.chart_1;
  let chart_4 = theme.chart_4;
  let danger = theme.danger;
  let grid = || h_flex().flex_wrap().gap_4().w_full();

  v_flex()
    .gap_4()
    .w_full()
    .child(
      grid()
        // 1. 基础垂直柱状图 + 数值标签 + 数值轴
        .child(chart_card(
          cx,
          "基础柱状图 + 数值轴",
          "BarChart::new(data).band(..).value(..)；.label() 显示数值；.value_axis(true)",
          BarChart::new(data::regional_performance())
            .id("bar-basic")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .label(|d| format!("{:.0}", d.value))
            .value_axis(true)
            .value_tick_count(4),
        ))
        // 2. 自定义填充（按类别取色）+ 圆角
        .child(chart_card(
          cx,
          "自定义填充 + 圆角柱条",
          ".fill(|d, bar_bounds, chart_bounds, alignment| ..)；.corner_radii(px(4.))",
          BarChart::new(data::regional_performance())
            .id("bar-fill")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .fill({
              let colors = palette::chart_colors(cx);
              move |d, _bar_bounds, _chart_bounds, _alignment| {
                let idx = match d.category.as_str() {
                  "North" => 0,
                  "South" => 1,
                  "East" => 2,
                  _ => 3,
                };
                colors[idx % colors.len()]
              }
            })
            .corner_radii(px(4.0)),
        ))
        // 3. 只圆化柱尖一端 + 自定义调色板
        .child(chart_card(
          cx,
          "Corners 精细圆角 + 自定义调色板",
          "Corners { top_left, top_right, bottom_left, bottom_right } 只圆化柱尖",
          BarChart::new(data::regional_performance())
            .id("bar-corners")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .fill({
              let colors = palette::custom_palette(cx);
              move |_, _, _, _| colors[1 % colors.len()]
            })
            .corner_radii(Corners {
              top_left: px(6.0),
              top_right: px(6.0),
              bottom_left: px(0.0),
              bottom_right: px(0.0),
            }),
        ))
        // 4. 逐柱渐变 fill_gradient
        .child(chart_card(
          cx,
          "逐柱渐变 fill_gradient",
          "渐变从柱底（stop-0）到柱尖（stop-1），与 BarAlignment 无关",
          BarChart::new(data::regional_performance())
            .id("bar-grad-bar")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .fill_gradient({
              let c = theme.chart_1;
              move |_, _, _| {
                [
                  linear_color_stop(c.opacity(0.3), 0.0),
                  linear_color_stop(c, 1.0),
                ]
              }
            }),
        )),
    )
    .child(
      grid()
        // 5. 全图渐变：每根柱显示单一渐变的对应切片
        .child(chart_card(
          cx,
          "全图渐变 chart_to_bar",
          "chart_to_bar 把图表数值坐标映射为柱条局部渐变位置；越界 stop 被裁剪并插值",
          BarChart::new(data::regional_performance())
            .id("bar-grad-chart")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .fill_gradient({
              let c = theme.chart_2;
              move |_, chart_range, chart_to_bar| {
                [
                  linear_color_stop(c.opacity(0.3), chart_to_bar(*chart_range.start())),
                  linear_color_stop(c, chart_to_bar(*chart_range.end())),
                ]
              }
            }),
        ))
        // 6. 水平柱状图 BarAlignment::Left
        .child(chart_card(
          cx,
          "水平柱状图 BarAlignment::Left",
          ".alignment(BarAlignment::Left)；水平时数值轴在下方",
          BarChart::new(data::regional_performance())
            .id("bar-left")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .alignment(BarAlignment::Left)
            .value_axis(true)
            .label(|d| format!("{:.0}", d.value))
            .fill(move |_, _, _, _| chart_3),
        ))
        // 7. 负值处理：从零线向两侧生长
        .child(chart_card(
          cx,
          "负值处理",
          "柱条从 0 基线生长，负值向另一侧延伸；无需额外配置",
          BarChart::new(data::quarterly_growth())
            .id("bar-negative")
            .band(|d| d.category.clone())
            .value(|d| d.value)
            .label(|d| format!("{:+.1}%", d.value))
            .fill(move |d, _, _, _| if d.value >= 0.0 { chart_1 } else { danger }),
        ))
        // 8. 其他对齐 + 刻度 + 标签控制
        .child(chart_card(
          cx,
          "BarAlignment::Top + 刻度控制",
          ".alignment(BarAlignment::Top).tick_margin(2).label_axis(true)",
          BarChart::new(data::quarterly_growth())
            .id("bar-top")
            .band(|d| d.category.clone())
            .value(|d| d.value.abs())
            .alignment(BarAlignment::Top)
            .tick_margin(2)
            .label_axis(true)
            .corner_radii(px(3.0))
            .fill(move |_, _, _, _| chart_4),
        )),
    )
    .into_any_element()
}

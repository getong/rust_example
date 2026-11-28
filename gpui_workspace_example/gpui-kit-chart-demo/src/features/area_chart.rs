//! 面积图 AreaChart：渐变填充 / 多序列 / 插值变体 / 交互。
//!
//! 与 LineChart 的关键差异：`y` 访问器可多次调用追加序列，
//! 每次 `.y()` 与紧随其后的 `.stroke()`/`.fill()` 配对；多序列 Y 值域自动合并。

use gpui_kit::{
  AnyElement, App, IntoElement, ParentElement, Styled,
  component::{ActiveTheme, chart::AreaChart, h_flex, v_flex},
  linear_color_stop, linear_gradient,
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
        // 1. 基础面积图
        .child(chart_card(
          cx,
          "基础面积图",
          "AreaChart::new(data).x(..).y(..)；线下区域填充",
          AreaChart::new(data::monthly_trend())
            .id("area-basic")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .stroke(theme.chart_1)
            .fill(theme.chart_1.opacity(0.4)),
        ))
        // 2. 渐变填充 linear_gradient
        .child(chart_card(
          cx,
          "渐变填充 linear_gradient",
          "linear_gradient(angle, from, to)；angle 0 自下而上",
          AreaChart::new(data::monthly_trend())
            .id("area-gradient")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .stroke(theme.chart_1)
            .fill(linear_gradient(
              0.0,
              linear_color_stop(theme.chart_1.opacity(0.4), 1.0),
              linear_color_stop(theme.background.opacity(0.3), 0.0),
            )),
        ))
        // 3. 多序列堆叠 + 逐序列配对
        .child(chart_card(
          cx,
          "多序列 + 逐序列 stroke/fill",
          "每次 .y() 追加一个序列，与紧随的 .stroke()/.fill() 配对；.name() 命名序列",
          AreaChart::new(data::device_usage())
            .id("area-multi")
            .x(|d| d.date.clone())
            .y(|d| d.desktop)
            .name("Desktop")
            .stroke(theme.chart_1)
            .fill(linear_gradient(
              0.0,
              linear_color_stop(theme.chart_1.opacity(0.4), 1.0),
              linear_color_stop(theme.background.opacity(0.3), 0.0),
            ))
            .y(|d| d.mobile)
            .name("Mobile")
            .stroke(theme.chart_2)
            .fill(linear_gradient(
              0.0,
              linear_color_stop(theme.chart_2.opacity(0.4), 1.0),
              linear_color_stop(theme.background.opacity(0.3), 0.0),
            ))
            .y(|d| d.tablet)
            .name("Tablet")
            .stroke(theme.chart_3)
            .fill(linear_gradient(
              0.0,
              linear_color_stop(theme.chart_3.opacity(0.4), 1.0),
              linear_color_stop(theme.background.opacity(0.3), 0.0),
            ))
            .tick_margin(3),
        ))
        // 4. linear 插值 + tick_margin
        .child(chart_card(
          cx,
          "linear 插值",
          ".linear() 或 .step_after()；逐序列 curve",
          AreaChart::new(data::monthly_trend())
            .id("area-linear")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .linear()
            .stroke(theme.chart_5)
            .fill(theme.chart_5.opacity(0.35))
            .tick_margin(2),
        )),
    )
    .child(
      grid()
        // 5. step_after
        .child(chart_card(
          cx,
          "step_after 阶梯",
          ".step_after() 阶梯式面积",
          AreaChart::new(data::monthly_trend())
            .id("area-step")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .step_after()
            .stroke(theme.chart_4)
            .fill(theme.chart_4.opacity(0.3)),
        ))
        // 6. 数值轴 + 参考线 + 定义域
        .child(chart_card(
          cx,
          "数值轴 + 参考线 + y_domain",
          ".y_axis(true).y_tick_count(..).reference_line(..).y_domain(..)",
          AreaChart::new(data::monthly_trend())
            .id("area-axis")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .stroke(theme.success)
            .fill(theme.success.opacity(0.25))
            .y_axis(true)
            .y_tick_count(5)
            .reference_line(150.0)
            .y_tick_format(|v| format!("{v:.0}")),
        ))
        // 7. 多序列半透明
        .child(chart_card(
          cx,
          "多序列半透明",
          "逐序列 .stroke/.fill 配对；本版本未显式设置时默认 chart_2 半透明",
          AreaChart::new(data::device_usage())
            .id("area-multi-flat")
            .x(|d| d.date.clone())
            .y(|d| d.desktop)
            .name("Desktop")
            .stroke(theme.chart_1)
            .fill(theme.chart_1.opacity(0.25))
            .y(|d| d.mobile)
            .name("Mobile")
            .stroke(theme.chart_2)
            .fill(theme.chart_2.opacity(0.25))
            .tick_margin(4),
        )),
    )
    .into_any_element()
}

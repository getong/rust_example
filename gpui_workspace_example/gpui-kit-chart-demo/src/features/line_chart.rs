//! 折线图 LineChart：插值变体 / 数据点 / 刻度密度 / 交互 tooltip / 主题色。
//!
//! 核心是 `x`/`y` 两个闭包，分别提取横轴类别与纵轴数值；`.name()` 设置 tooltip 中的数据系列名。
//! 本版本交互默认开启（`.interactive(true)`），`id` 默认取构造位置；
//! `.id()` 用于在兄弟元素中保证唯一，`.interactive(false)` 关闭交互做静态绘制。

use gpui_kit::{
  AnyElement, App, IntoElement, ParentElement, Styled,
  component::{ActiveTheme, chart::LineChart, h_flex, v_flex},
};

use crate::{chart_card::chart_card, data, palette};

pub fn render(cx: &App) -> AnyElement {
  let theme = cx.theme();

  let grid = || h_flex().flex_wrap().gap_4().w_full();

  v_flex()
    .gap_4()
    .w_full()
    .child(
      grid()
        // 1. 基础折线 + 交互 tooltip：natural 插值（默认）
        .child(chart_card(
          cx,
          "基础折线 + tooltip",
          "LineChart::new(data).id(..).name(..).x(..).y(..)；natural 自然曲线（默认）",
          LineChart::new(data::monthly_trend())
            .id("line-basic")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .stroke(theme.chart_1),
        ))
        // 2. linear 线性插值
        .child(chart_card(
          cx,
          "linear 线性插值",
          ".linear() 切换为直线段连接数据点",
          LineChart::new(data::monthly_trend())
            .id("line-linear")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .linear()
            .stroke(theme.chart_2),
        ))
        // 3. step_after 阶梯后置
        .child(chart_card(
          cx,
          "step_after 阶梯后置",
          ".step_after() 阶梯式连接，适合离散状态变化",
          LineChart::new(data::monthly_trend())
            .id("line-step")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .step_after()
            .stroke(theme.chart_3),
        ))
        // 4. dot 数据点
        .child(chart_card(
          cx,
          "dot 数据点",
          ".dot() 在每个数据点绘制圆点",
          LineChart::new(data::monthly_trend())
            .id("line-dot")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .dot()
            .stroke(theme.chart_4),
        )),
    )
    .child(
      grid()
        // 5. 刻度密度 tick_margin
        .child(chart_card(
          cx,
          "tick_margin 刻度密度",
          ".tick_margin(3) 每 3 个类别显示一个横轴标签",
          LineChart::new(data::monthly_trend())
            .id("line-ticks")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .tick_margin(3)
            .stroke(theme.chart_5),
        ))
        // 6. 自定义颜色 + 数值轴 + 参考线 + 定义域
        .child(chart_card(
          cx,
          "数值轴 + 参考线 + y_domain",
          ".y_axis(true).y_tick_count(5).reference_line(..).y_domain(..).y_tick_format(..)",
          LineChart::new(data::monthly_trend())
            .id("line-axis")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .stroke(theme.success)
            .y_axis(true)
            .y_tick_count(5)
            .y_domain(0.0, 240.0)
            .reference_line(150.0)
            .y_tick_format(|v| format!("{v:.0}"))
            .tick_margin(2),
        ))
        // 7. 隐藏网格 / 横轴 + 自定义色板
        .child(chart_card(
          cx,
          "隐藏网格与横轴 + 自定义主题色",
          ".grid(false).x_axis(false)；stroke 用 cx.theme() 任意色",
          LineChart::new(data::monthly_trend())
            .id("line-bare")
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .grid(false)
            .x_axis(false)
            .stroke(palette::chart_color(cx, 0)),
        ))
        // 8. 静态图表：.interactive(false)
        .child(chart_card(
          cx,
          "静态图表 interactive(false)",
          "交互默认开启；.interactive(false) 关闭 hitbox，不响应悬停；id 默认取构造位置",
          LineChart::new(data::monthly_trend())
            .name("Trend")
            .x(|d| d.label.clone())
            .y(|d| d.value)
            .interactive(false)
            .stroke(theme.muted_foreground),
        )),
    )
    .into_any_element()
}

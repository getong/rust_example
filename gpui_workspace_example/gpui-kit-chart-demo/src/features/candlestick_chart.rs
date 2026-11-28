//! K 线图 CandlestickChart：OHLC 四值 / body_width_ratio / tick_margin / 阳线阴线配色。
//!
//! 金融数据展示：open/high/low/close 四个闭包提取字段；
//! 阳线（close > open）用 `chart_bullish`，阴线用 `chart_bearish`，可 `.bullish()`/`.bearish()`
//! 覆盖。

use gpui_kit::{
  AnyElement, App, IntoElement, ParentElement, Styled,
  component::{
    ActiveTheme,
    chart::{BarChart, CandlestickChart},
    h_flex, v_flex,
  },
};

use crate::{chart_card::chart_card, data};

pub fn render(cx: &App) -> AnyElement {
  let theme = cx.theme();
  let grid = || h_flex().flex_wrap().gap_4().w_full();
  let bullish = theme.chart_bullish;
  let bearish = theme.chart_bearish;

  v_flex()
    .gap_4()
    .w_full()
    .child(
      grid()
        // 1. 基础 K 线
        .child(chart_card(
          cx,
          "基础 K 线",
          "CandlestickChart::new(data).x(..).open(..).high(..).low(..).close(..)",
          CandlestickChart::new(data::stock_prices())
            .id("k-basic")
            .x(|d| d.date.clone())
            .open(|d| d.open)
            .high(|d| d.high)
            .low(|d| d.low)
            .close(|d| d.close)
            .tick_margin(4),
        ))
        // 2. body_width_ratio 实体宽度
        .child(chart_card(
          cx,
          "body_width_ratio 实体宽度",
          "控制实体占 band 宽度的比例（源码默认 0.8）；.body_width_ratio(0.4) 更窄",
          CandlestickChart::new(data::stock_prices())
            .id("k-narrow")
            .x(|d| d.date.clone())
            .open(|d| d.open)
            .high(|d| d.high)
            .low(|d| d.low)
            .close(|d| d.close)
            .body_width_ratio(0.4)
            .tick_margin(4),
        ))
        // 3. 自定义阳线/阴线配色
        .child(chart_card(
          cx,
          "自定义阳线/阴线配色",
          ".bullish(..).bearish(..)；默认主题 chart_bullish / chart_bearish",
          CandlestickChart::new(data::stock_prices())
            .id("k-colors")
            .x(|d| d.date.clone())
            .open(|d| d.open)
            .high(|d| d.high)
            .low(|d| d.low)
            .close(|d| d.close)
            .bullish(theme.chart_1)
            .bearish(theme.chart_4)
            .tick_margin(4),
        ))
        // 4. 刻度 + 网格 + 轴控制
        .child(chart_card(
          cx,
          "tick_margin + 网格/轴控制",
          ".tick_margin(8).x_axis(true).grid(true)；.max_band_width(px(..)) 限制柱宽",
          CandlestickChart::new(data::stock_prices())
            .id("k-ticks")
            .x(|d| d.date.clone())
            .open(|d| d.open)
            .high(|d| d.high)
            .low(|d| d.low)
            .close(|d| d.close)
            .tick_margin(8)
            .x_axis(true)
            .grid(true)
            .max_band_width(gpui_kit::px(14.0)),
        ))
        // 5. 金融行情页：成交量柱状图（K 线 + 折线 + 成交量组合见各图表模块）
        .child(chart_card(
          cx,
          "Trading Volume 成交量",
          "K 线 + 成交量柱状图组合是典型行情页；BarChart .value(|d| d.volume as f64)",
          BarChart::new(data::stock_prices())
            .id("k-volume")
            .band(|d| d.date.clone())
            .value(|d| d.volume as f64)
            .fill(move |d, _, _, _| if d.close >= d.open { bullish } else { bearish })
            .tick_margin(4),
        )),
    )
    .into_any_element()
}

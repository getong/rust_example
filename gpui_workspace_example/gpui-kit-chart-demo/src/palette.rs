//! 主题色板助手 / Theme palette helpers.
//!
//! 图表默认自动使用主题色（`chart_1`..`chart_5`），深色/浅色模式自动适配；
//! 这里封装常用取色逻辑。推荐优先使用主题色而非硬编码颜色。

use gpui_kit::{App, Hsla, component::ActiveTheme};

/// 主题图表 5 色板。
pub fn chart_colors(cx: &App) -> [Hsla; 5] {
  let theme = cx.theme();
  [
    theme.chart_1,
    theme.chart_2,
    theme.chart_3,
    theme.chart_4,
    theme.chart_5,
  ]
}

/// 按下标循环取主题图表色。
pub fn chart_color(cx: &App, index: usize) -> Hsla {
  chart_colors(cx)[index % 5]
}

/// 自定义调色板示例：用主题语义色组合。
pub fn custom_palette(cx: &App) -> Vec<Hsla> {
  let theme = cx.theme();
  vec![
    theme.success,
    theme.warning,
    theme.danger,
    theme.info,
    theme.chart_1,
  ]
}

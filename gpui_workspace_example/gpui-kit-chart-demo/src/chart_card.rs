//! 统一风格的图表容器 / Uniformly-styled chart container.
//!
//! 把图表放入带边框、标题、副标题的卡片中，配合 `v_flex`/`h_flex` 布局原语。
//!
//! 渲染要点：图表尺寸由容器决定，因此给图表区一个**明确的高度**（`h(px(300.))`）
//! 与 `w_full()`；卡片 `flex_1 + min_w` 在 `flex_wrap` 网格里自适应、可换行。

use gpui_kit::{
  App, FontWeight, IntoElement, ParentElement, Styled,
  component::{ActiveTheme, v_flex},
  div, px,
};

/// 图表区高度。
pub const CHART_HEIGHT: f32 = 300.0;
/// 卡片最小宽度（小于此宽度时换行）。
pub const CARD_MIN_WIDTH: f32 = 420.0;

pub fn chart_card(
  cx: &App,
  title: impl Into<String>,
  description: impl Into<String>,
  chart: impl IntoElement,
) -> impl IntoElement {
  let theme = cx.theme();

  v_flex()
    .flex_1()
    .min_w(px(CARD_MIN_WIDTH))
    .border_1()
    .border_color(theme.border)
    .rounded(theme.radius_lg)
    .overflow_hidden()
    .bg(theme.background)
    .p_4()
    .gap_3()
    .child(div().font_weight(FontWeight::SEMIBOLD).child(title.into()))
    .child(
      div()
        .text_sm()
        .text_color(theme.muted_foreground)
        .child(description.into()),
    )
    // 图表区：明确高度 + 满宽，图表尺寸由容器决定。
    .child(div().w_full().h(px(CHART_HEIGHT)).overflow_hidden().child(chart))
}

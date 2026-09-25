use gpui_kit::*;

use super::{common::*, icon::icon};
use crate::travel::{app::TravelApp, theme::*};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  row()
    .p_4()
    .gap_4()
    .bg(rgb(0xffffff))
    .border_1()
    .border_color(rgb(BORDER))
    .rounded_xl()
    .child(
      row()
        .w(px(72.))
        .h(px(88.))
        .justify_center()
        .bg(rgb(0xf3ece0))
        .rounded_lg()
        .child(icon("town", ORANGE)),
    )
    .child(
      column()
        .flex_1()
        .gap_2()
        .child(label("沙溪古镇", 17., GREEN).font_weight(FontWeight::SEMIBOLD))
        .child(label("茶马古道上的时光驿站", 11., MUTED))
        .child(label("★ 4.9   ·   人均 ¥120", 12., ORANGE)),
    )
    .child(
      column()
        .id("save-place")
        .cursor_pointer()
        .items_center()
        .gap_2()
        .on_click(cx.listener(|this, _, _, cx| {
          this.state.saved = !this.state.saved;
          cx.notify();
        }))
        .child(icon("heart", if app.state.saved { ORANGE } else { MUTED }))
        .child(label(
          if app.state.saved {
            "已收藏"
          } else {
            "收藏"
          },
          10.,
          MUTED,
        )),
    )
}

use gpui_kit::*;

use super::{common::*, icon::icon, landscape::landscape};
use crate::travel::{app::TravelApp, state::Page, theme::*};

pub(in crate::travel) fn render(cx: &mut Context<TravelApp>) -> Div {
  column()
    .gap_4()
    .child(
      row()
        .justify_between()
        .child(
          label("云南 · 大理", 11., GREEN)
            .px_3()
            .py_1()
            .rounded_full()
            .bg(rgb(PALE)),
        )
        .child(label("距出发还有 12 天", 12., ORANGE)),
    )
    .child(
      column()
        .gap_2()
        .child(label("大理 · 去有风的地方", 27., GREEN).font_weight(FontWeight::BOLD))
        .child(label("晨雾苍山、洱海日出、慢生活节奏", 13., MUTED)),
    )
    .child(landscape())
    .child(
      row()
        .justify_between()
        .child(label("2026.10.07 — 10.10", 13., INK))
        .child(label("4 天 3 晚 · 2 人同行", 12., MUTED)),
    )
    .child(
      row()
        .justify_between()
        .child(
          row()
            .gap_2()
            .children([("A", PALE), ("B", 0xf4e7d9)].map(|(name, color)| {
              row()
                .size_8()
                .justify_center()
                .rounded_full()
                .bg(rgb(color))
                .child(label(name, 12., GREEN))
            }))
            .child(label("一起出发", 11., MUTED)),
        )
        .child(
          row()
            .id("view-trip")
            .cursor_pointer()
            .gap_3()
            .px_4()
            .py_3()
            .rounded_full()
            .bg(rgb(GREEN))
            .hover(|s| s.bg(rgb(0x38634e)))
            .on_click(cx.listener(|this, _, _, cx| {
              this.state.page = Page::Trips;
              cx.notify();
            }))
            .child(label("查看完整行程", 12., 0xffffff))
            .child(icon("arrow", 0xffffff)),
        ),
    )
}

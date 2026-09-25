use gpui_kit::*;

use super::{common::*, icon::icon};
use crate::travel::{app::TravelApp, state::Page, theme::*};

pub(in crate::travel) fn render(cx: &mut Context<TravelApp>) -> Div {
  row()
    .justify_between()
    .px_6()
    .py_4()
    .border_b_1()
    .border_color(rgb(BORDER))
    .child(
      row()
        .gap_2()
        .child(icon("map", GREEN))
        .child(heading("远行")),
    )
    .child(label("早安，旅行者", 13., MUTED))
    .child(
      row()
        .gap_3()
        .child(
          div()
            .id("avatar")
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
              this.state.page = Page::Profile;
              cx.notify();
            }))
            .child(
              row()
                .justify_center()
                .size_8()
                .rounded_full()
                .bg(rgb(PALE))
                .child(label("A", 13., GREEN)),
            ),
        )
        .child(
          div()
            .id("notifications")
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
              this.state.notifications = !this.state.notifications;
              cx.notify();
            }))
            .child(icon("bell", ORANGE)),
        ),
    )
}

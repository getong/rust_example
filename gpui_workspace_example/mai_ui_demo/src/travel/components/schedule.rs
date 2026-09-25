use gpui_kit::*;

use super::{common::*, icon::icon};
use crate::travel::{app::TravelApp, data::DAYS, theme::*};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  column()
    .gap_4()
    .child(
      row()
        .justify_between()
        .child(heading("每日安排"))
        .child(label(format!("10 月 {} 日", 7 + app.state.day), 12., MUTED)),
    )
    .child(row().gap_2().children((0 .. DAYS.len()).map(|day| {
      row()
        .id(("day", day))
        .flex_1()
        .justify_center()
        .py_2()
        .rounded_lg()
        .cursor_pointer()
        .bg(rgb(if app.state.day == day { GREEN } else { PALE }))
        .on_click(cx.listener(move |this, _, _, cx| {
          this.state.day = day;
          cx.notify();
        }))
        .child(label(
          format!("Day {}", day + 1),
          12.,
          if app.state.day == day {
            0xffffff
          } else {
            GREEN
          },
        ))
    })))
    .children(DAYS[app.state.day].items.iter().map(|item| {
      row()
        .gap_3()
        .p_4()
        .bg(rgb(0xffffff))
        .border_1()
        .border_l_3()
        .border_color(rgb(0xccdbd1))
        .rounded_lg()
        .child(
          row()
            .size_10()
            .justify_center()
            .rounded_lg()
            .bg(rgb(PALE))
            .child(icon(item.icon, GREEN)),
        )
        .child(
          column()
            .flex_1()
            .gap_1()
            .child(label(item.name, 14., INK).font_weight(FontWeight::SEMIBOLD))
            .child(label(item.detail, 11., MUTED)),
        )
        .child(label(item.time, 12., GREEN))
    }))
    .child(label(DAYS[app.state.day].route, 11., MUTED))
}

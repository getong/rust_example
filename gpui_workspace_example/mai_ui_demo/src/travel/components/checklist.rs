use gpui_kit::{prelude::FluentBuilder, *};

use super::{common::*, icon::icon};
use crate::travel::{app::TravelApp, theme::*};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  let done = app.state.ready.iter().filter(|ready| **ready).count();
  column()
    .gap_4()
    .child(
      row()
        .justify_between()
        .child(heading("出发前准备"))
        .child(label(format!("{done}/3 已完成"), 12., MUTED)),
    )
    .child(
      div()
        .h(px(6.))
        .w_full()
        .rounded_full()
        .bg(rgb(BORDER))
        .child(
          div()
            .h_full()
            .w(relative(done as f32 / 3.))
            .rounded_full()
            .bg(rgb(GREEN)),
        ),
    )
    .children(
      ["交通方式", "住宿安排", "行李整理"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
          let ready = app.state.ready[index];
          row()
            .id(("check", index))
            .cursor_pointer()
            .justify_between()
            .py_1()
            .on_click(cx.listener(move |this, _, _, cx| {
              this.state.ready[index] = !this.state.ready[index];
              cx.notify();
            }))
            .child(
              row()
                .gap_3()
                .child(
                  row()
                    .size_5()
                    .justify_center()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(if ready { GREEN } else { ORANGE }))
                    .bg(rgb(if ready { GREEN } else { 0xffffff }))
                    .when(ready, |s| s.child(icon("check", 0xffffff))),
                )
                .child(label(name, 13., INK)),
            )
            .child(label(
              if ready { "已完成" } else { "待整理" },
              11.,
              if ready { MUTED } else { ORANGE },
            ))
        }),
    )
}

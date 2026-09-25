use gpui_kit::{prelude::FluentBuilder, *};

use super::{common::*, icon::icon};
use crate::travel::{app::TravelApp, state::Page, theme::*};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  row()
    .flex_shrink_0()
    .px_3()
    .py_3()
    .border_t_1()
    .border_color(rgb(BORDER))
    .bg(rgb(0xffffff))
    .children(
      [
        (Page::Home, "home", "首页"),
        (Page::Trips, "map", "行程"),
        (Page::Saved, "heart", "收藏"),
        (Page::Profile, "user", "我的"),
      ]
      .into_iter()
      .enumerate()
      .map(|(index, (page, image, title))| {
        let active = app.state.page == page;
        column()
          .id(("nav", index))
          .flex_1()
          .items_center()
          .gap_1()
          .py_2()
          .rounded_lg()
          .cursor_pointer()
          .when(active, |s| s.bg(rgb(PALE)))
          .on_click(cx.listener(move |this, _, _, cx| {
            this.state.page = page;
            cx.notify();
          }))
          .child(icon(image, if active { GREEN } else { MUTED }))
          .child(label(title, 11., if active { GREEN } else { MUTED }))
      }),
    )
}

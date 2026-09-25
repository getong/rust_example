//! Root view: owns shared state and composes the shell around the selected tab.
use gpui_kit::{prelude::FluentBuilder, *};

use super::{
  components::{common::*, header, navigation},
  state::{Page, TravelState},
  tabs,
  theme::*,
};

#[derive(Default)]
pub(crate) struct TravelApp {
  pub(super) state: TravelState,
}

impl Render for TravelApp {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let content = match self.state.page {
      Page::Home => tabs::home::render(self, cx),
      Page::Trips => tabs::trips::render(self, cx),
      Page::Saved => tabs::saved::render(self, cx),
      Page::Profile => tabs::profile::render(self, cx),
    };
    row()
      .size_full()
      .justify_center()
      .bg(rgb(0xeef1eb))
      .font_family(".AppleSystemUIFont")
      .text_color(rgb(INK))
      .child(
        column()
          .size_full()
          .max_w(px(480.))
          .bg(rgb(0xfafbf7))
          .child(header::render(cx))
          .when(self.state.notifications, |s| {
            s.child(
              label("出发提醒：记得整理行李，并检查身份证件。", 12., GREEN)
                .px_6()
                .py_3()
                .bg(rgb(PALE)),
            )
          })
          .child(
            div()
              .id(match self.state.page {
                Page::Home => "home-scroll",
                Page::Trips => "trip-scroll",
                Page::Saved => "saved-scroll",
                Page::Profile => "profile-scroll",
              })
              .flex_1()
              .min_h_0()
              .overflow_y_scroll()
              .px_6()
              .py_6()
              .child(
                content.child(
                  label("远行 YUANXING  ·  让每一次出发都有期待", 10., MUTED)
                    .text_center()
                    .py_3(),
                ),
              ),
          )
          .child(navigation::render(self, cx)),
      )
  }
}

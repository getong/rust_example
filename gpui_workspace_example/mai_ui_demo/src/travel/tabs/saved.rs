use gpui_kit::{prelude::FluentBuilder, *};

use crate::travel::{
  app::TravelApp,
  components::{common::*, recommendation},
  theme::*,
};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  column()
    .gap_6()
    .child(heading("收藏目的地"))
    .child(label("把心动的地方，留给下一次出发。", 13., MUTED))
    .when(app.state.saved, |s| {
      s.child(recommendation::render(app, cx))
    })
    .when(!app.state.saved, |s| {
      s.child(card().child(heading("还没有收藏的目的地")).child(label(
        "点击下方爱心，收藏沙溪古镇。",
        13.,
        MUTED,
      )))
      .child(recommendation::render(app, cx))
    })
}

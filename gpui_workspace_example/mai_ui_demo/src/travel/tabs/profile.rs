use gpui_kit::*;

use crate::travel::{
  app::TravelApp,
  components::{checklist, common::*, icon::icon, overview},
  theme::*,
};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  column()
    .gap_6()
    .child(heading("我的资料"))
    .child(
      card()
        .child(icon("user", GREEN))
        .child(heading("旅行者小A"))
        .child(label("和小B一起，去看更远的风景。", 13., MUTED)),
    )
    .child(overview::render())
    .child(
      card()
        .child(heading("旅行足迹"))
        .child(label("已创建行程  2 个     ·     总天数  20 天", 14., INK))
        .child(label(
          format!("收藏目的地  {} 个", usize::from(app.state.saved)),
          14.,
          INK,
        )),
    )
    .child(checklist::render(app, cx))
}

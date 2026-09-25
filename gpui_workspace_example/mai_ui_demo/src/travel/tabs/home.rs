use gpui_kit::*;

use crate::travel::{
  app::TravelApp,
  components::{checklist, common::*, hero, overview, recommendation, schedule},
};

pub(in crate::travel) fn render(app: &TravelApp, cx: &mut Context<TravelApp>) -> Div {
  column()
    .gap_6()
    .child(hero::render(cx))
    .child(overview::render())
    .child(schedule::render(app, cx))
    .child(recommendation::render(app, cx))
    .child(checklist::render(app, cx))
}

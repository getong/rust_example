use gpui_kit::*;

use super::common::*;
use crate::travel::theme::*;

pub(in crate::travel) fn render() -> Div {
  row()
    .justify_between()
    .p_4()
    .bg(rgb(PALE))
    .rounded_lg()
    .children(
      [
        ("预算总额", "¥3,600"),
        ("已预订", "2 / 3 项"),
        ("目的地天气", "晴天 22°C"),
      ]
      .map(|(title, value)| {
        column()
          .gap_2()
          .child(label(title, 11., MUTED))
          .child(label(value, 15., GREEN).font_weight(FontWeight::SEMIBOLD))
      }),
    )
}

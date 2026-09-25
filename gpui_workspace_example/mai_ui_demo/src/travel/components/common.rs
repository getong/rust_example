use gpui_kit::*;

use crate::travel::theme::{BORDER, GREEN};

pub(in crate::travel) fn row() -> Div {
  div().flex().items_center()
}
pub(in crate::travel) fn column() -> Div {
  div().flex().flex_col()
}
pub(in crate::travel) fn label(text: impl Into<SharedString>, size: f32, color: u32) -> Div {
  div()
    .text_size(px(size))
    .text_color(rgb(color))
    .child(text.into())
}
pub(in crate::travel) fn heading(text: &'static str) -> Div {
  label(text, 19., GREEN).font_weight(FontWeight::BOLD)
}
pub(in crate::travel) fn card() -> Div {
  column()
    .gap_4()
    .p_5()
    .bg(rgb(0xffffff))
    .border_1()
    .border_color(rgb(BORDER))
    .rounded_xl()
}

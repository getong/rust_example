use gpui_kit::{
  component::{TitleBar, h_flex},
  *,
};

pub fn view() -> impl IntoElement {
  TitleBar::new().child(h_flex().h_full().items_center().child("GPUI Kit"))
}

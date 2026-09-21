//! Slot 注入：容器只负责布局，具体视图由调用方创建并转成 AnyView。
use gpui_kit::*;

pub(crate) struct SlotContainer {
  children: Vec<AnyView>,
}
impl SlotContainer {
  pub(crate) fn new(children: Vec<AnyView>) -> Self {
    Self { children }
  }
}
impl Render for SlotContainer {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .items_center()
      .gap_3()
      .children(self.children.iter().cloned())
  }
}

pub(crate) struct Profile;
impl Render for Profile {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div().child("Profile: Guest")
  }
}

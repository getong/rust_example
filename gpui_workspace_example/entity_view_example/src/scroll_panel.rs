use gpui_kit::{
  component::{
    ActiveTheme,
    scroll::{Scrollbar, ScrollbarMode},
  },
  *,
};

/// 保留调用方的滚动句柄，统一视口、边框和常驻滚动条。
#[derive(IntoElement)]
pub(crate) struct ScrollPanel {
  id: ElementId,
  handle: ScrollHandle,
  children: Vec<AnyElement>,
}

impl ScrollPanel {
  pub(crate) fn new(id: impl Into<ElementId>, handle: &ScrollHandle) -> Self {
    Self {
      id: id.into(),
      handle: handle.clone(),
      children: Vec::new(),
    }
  }
}

impl ParentElement for ScrollPanel {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for ScrollPanel {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    div()
      .relative()
      .flex_1()
      .min_h_0()
      .overflow_hidden()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .border_1()
      .border_color(cx.theme().border)
      .rounded_lg()
      .child(
        div()
          .id(self.id)
          .size_full()
          .overflow_y_scroll()
          .track_scroll(&self.handle)
          .pr_4()
          .children(self.children),
      )
      .child(Scrollbar::vertical(&self.handle).mode(ScrollbarMode::Always))
  }
}

//! 直接组合的基础计数器，也可由调用方注入 Slot 容器。
use gpui_kit::{component::button::*, *};

#[derive(Default)]
pub(crate) struct Counter {
  count: i32,
}

impl Render for Counter {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .gap_3()
      .p_4()
      .child(format!("count:{}", self.count))
      .child(
        Button::new("increment")
          .label("+1")
          .on_click(cx.listener(|view, _, _, cx| {
            view.count += 1;

            cx.notify(); //标记需要重绘
          })),
      )
  }
}

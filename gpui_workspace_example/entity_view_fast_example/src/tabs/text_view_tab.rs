use gpui_kit::{
  component::{
    button::Button,
    text::{html, markdown},
    v_flex,
  },
  *,
};
pub struct TextViewTab {
  markdown: bool,
}
impl Render for TextViewTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .gap_4()
      .child(
        Button::new("text-format")
          .label(if self.markdown {
            "Show HTML"
          } else {
            "Show Markdown"
          })
          .on_click(cx.listener(|this, _, _, cx| {
            this.markdown = !this.markdown;
            cx.notify();
          })),
      )
      .child(if self.markdown {
        markdown(
          "# TextView\n\n**Bold**, *italic*, `inline code` and selectable text.\n\n- Markdown \
           item one\n- Markdown item two\n\n> A quoted paragraph.\n\n```rust\nlet count = 42;\n```"
            .to_string(),
        )
        .into_any_element()
      } else {
        html(
          "<h1>HTML TextView</h1><p><strong>Rich text</strong> with \
           <em>formatting</em>.</p><ul><li>First item</li><li>Second item</li></ul>"
            .to_string(),
        )
        .into_any_element()
      })
  }
}
impl super::ComponentPage for TextViewTab {
  fn title() -> &'static str {
    "TextView"
  }
  fn new_view(_window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self { markdown: true })
  }
}

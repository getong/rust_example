use gpui_kit::{
  component::{FocusTrapElement, button::Button, h_flex, v_flex},
  *,
};
pub struct FocusTrapTab {
  enabled: bool,
  focus: FocusHandle,
}
impl Render for FocusTrapTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let content = h_flex()
      .gap_3()
      .child(Button::new("trap-first").label("First"))
      .child(Button::new("trap-second").label("Second"))
      .child(
        Button::new("trap-exit")
          .label("Release focus")
          .on_click(cx.listener(|this, _, _, cx| {
            this.enabled = false;
            cx.notify();
          })),
      );
    v_flex()
      .gap_4()
      .child(
        "Enable, then click First. Tab / Shift-Tab stays inside the three controls. Release focus \
         exits.",
      )
      .child(
        Button::new("enable-trap")
          .label("Enable focus trap")
          .on_click(cx.listener(|this, _, _, cx| {
            this.enabled = true;
            cx.notify();
          })),
      )
      .child(if self.enabled {
        content
          .focus_trap("demo-trap", &self.focus)
          .into_any_element()
      } else {
        content.into_any_element()
      })
      .child(Button::new("outside-trap").label("Outside the trap"))
  }
}
impl super::ComponentPage for FocusTrapTab {
  fn title() -> &'static str {
    "Focus Trap"
  }
  fn new_view(_window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|cx| Self {
      enabled: true,
      focus: cx.focus_handle(),
    })
  }
}

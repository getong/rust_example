use gpui_kit::{
  component::{WindowExt, button::Button, h_flex, v_flex},
  *,
};
pub struct RootDemoTab;
impl Render for RootDemoTab {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .gap_4()
      .child(
        "This window has one Root. Its layers host dialogs, sheets and notifications across tabs.",
      )
      .child(
        h_flex()
          .gap_3()
          .child(
            Button::new("root-dialog")
              .label("Open dialog")
              .on_click(|_, window, cx| {
                window.open_dialog(cx, |dialog, _, _| {
                  dialog
                    .title("Root dialog layer")
                    .child("The dialog belongs to this window.")
                })
              }),
          )
          .child(
            Button::new("root-sheet")
              .label("Open sheet")
              .on_click(|_, window, cx| {
                window.open_sheet(cx, |sheet, _, _| {
                  sheet
                    .title("Root sheet layer")
                    .child("Click outside to dismiss.")
                })
              }),
          )
          .child(
            Button::new("root-toast")
              .label("Show notification")
              .on_click(|_, window, cx| window.push_notification("Root notification layer", cx)),
          ),
      )
  }
}
impl super::ComponentPage for RootDemoTab {
  fn title() -> &'static str {
    "Root View"
  }
  fn new_view(_window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self)
  }
}

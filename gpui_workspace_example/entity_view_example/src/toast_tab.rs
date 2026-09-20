use gpui_kit::{
  component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    notification::Notification,
  },
  *,
};

pub(crate) struct ToastTab;

// Notification 继承文字颜色；这里使用与弹窗背景配对的主题前景色。
fn show_toast(note: Notification, window: &mut Window, cx: &mut App) {
  window.push_notification(
    note
      .text_color(cx.theme().popover_foreground)
      .action(|_, _, cx| {
        Button::new("dismiss-toast")
          .label("Close")
          .on_click(cx.listener(|note, _, window, cx| {
            cx.stop_propagation();
            note.dismiss(window, cx);
          }))
      })
      // action 默认关闭自动消失，必须在 action 之后重新开启。
      .autohide(true),
    cx,
  );
}

impl Render for ToastTab {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .flex()
      .flex_col()
      .items_center()
      .justify_center()
      .gap_4()
      .child(div().text_2xl().child("Toast playground"))
      .child("Click a button to show a toast in this window.")
      .child(
        div()
          .flex()
          .gap_2()
          .child(
            Button::new("toast-success")
              .primary()
              .label("Success")
              .on_click(|_, window, cx| {
                show_toast(
                  Notification::success("Your changes have been saved.").title("Saved"),
                  window,
                  cx,
                );
              }),
          )
          .child(
            Button::new("toast-info")
              .label("Info")
              .on_click(|_, window, cx| {
                show_toast(
                  Notification::info("This is an informational toast."),
                  window,
                  cx,
                );
              }),
          )
          .child(
            Button::new("toast-warning")
              .label("Warning")
              .on_click(|_, window, cx| {
                show_toast(
                  Notification::warning("Please review your changes."),
                  window,
                  cx,
                );
              }),
          )
          .child(
            Button::new("toast-error")
              .label("Error")
              .on_click(|_, window, cx| {
                show_toast(
                  Notification::error("Something went wrong. Please try again."),
                  window,
                  cx,
                );
              }),
          ),
      )
      .child("Toasts close after 5 seconds; hover to pause or click Close.")
      .child("Toasts belong to this window and remain visible when switching tabs.")
  }
}

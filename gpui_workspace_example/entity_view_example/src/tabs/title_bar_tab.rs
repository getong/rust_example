use gpui_kit::{
  component::{ActiveTheme, Root, TitleBar, button::Button, v_flex},
  *,
};
pub struct TitleBarTab;
struct TitleBarWindow;
impl Render for TitleBarWindow {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(TitleBar::new().child("TitleBar native preview"))
      .child("Drag the title bar, double-click to zoom, or use the window controls.")
  }
}
impl Render for TitleBarTab {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .gap_4()
      .child(TitleBar::new().child("Custom TitleBar"))
      .child("Open a dedicated window to try native title-bar dragging and window controls.")
      .child(
        Button::new("titlebar-window")
          .label("Open native title-bar window")
          .on_click(|_, _, cx| {
            if let Err(error) = cx.open_window(TitleBar::window_options(), |window, cx| {
              let view = cx.new(|_| TitleBarWindow);
              window.activate_window();
              cx.new(|cx| Root::new(view, window, cx))
            }) {
              eprintln!("Unable to open title-bar preview: {error}");
            }
          }),
      )
  }
}
impl super::ComponentPage for TitleBarTab {
  fn title() -> &'static str {
    "TitleBar"
  }
  fn new_view(_window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self)
  }
}

use gpui::{
  App, Bounds, Context, KeyBinding, TitlebarOptions, Window, WindowBounds, WindowOptions, actions,
  div, prelude::*, px, rgb, size,
};

actions!(gpui_fast_example, [Quit]);

#[derive(Default)]
struct CounterApp {
  count: u64,
}

impl Render for CounterApp {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .flex()
      .flex_col()
      .items_center()
      .justify_center()
      .gap_6()
      .bg(rgb(0x111827))
      .text_color(rgb(0xf3f4f6))
      .child(div().text_3xl().child("Hello, GPUI Fast!"))
      .child(
        div()
          .text_sm()
          .text_color(rgb(0x9ca3af))
          .child("A minimal Rust desktop application"),
      )
      .child(div().text_3xl().child(self.count.to_string()))
      .child(
        div()
          .flex()
          .gap_3()
          .child(
            div()
              .id("increment")
              .px_6()
              .py_3()
              .rounded_lg()
              .bg(rgb(0x2563eb))
              .cursor_pointer()
              .hover(|style| style.bg(rgb(0x1d4ed8)))
              .on_click(cx.listener(|this, _, _, cx| {
                this.count = this.count.saturating_add(1);
                // Notify GPUI Fast so the retained view is rendered again.
                cx.notify();
              }))
              .child("Increment"),
          )
          .child(
            div()
              .id("reset")
              .px_6()
              .py_3()
              .rounded_lg()
              .bg(rgb(0x374151))
              .cursor_pointer()
              .hover(|style| style.bg(rgb(0x4b5563)))
              .on_click(cx.listener(|this, _, _, cx| {
                this.count = 0;
                cx.notify();
              }))
              .child("Reset"),
          ),
      )
  }
}

fn main() {
  gpui_platform::application().run(|cx: &mut App| {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
    cx.on_window_closed(|cx, _window_id| {
      if cx.windows().is_empty() {
        cx.quit();
      }
    })
    .detach();

    let bounds = Bounds::centered(None, size(px(720.0), px(480.0)), cx);
    cx.open_window(
      WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(400.0), px(320.0))),
        titlebar: Some(TitlebarOptions {
          title: Some("GPUI Fast Example".into()),
          ..Default::default()
        }),
        ..Default::default()
      },
      |_, cx| cx.new(|_| CounterApp::default()),
    )
    .expect("failed to open the application window");

    cx.activate(true);
  });
}

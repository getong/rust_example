use gpui_kit::{
  component::{ActiveTheme, Root, button::Button},
  *,
};

#[derive(Default)]
struct ExampleWindow {
  clicks: usize,
}

impl Render for ExampleWindow {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .flex()
      .flex_col()
      .gap_4()
      .p_6()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(div().text_xl().child("独立 GPUI Window"))
      .child("这是新的原生窗口。每个窗口拥有自己的 Entity 和计数，关闭此窗口不会关闭主窗口。")
      .child(
        Button::new("new-window-counter")
          .label(format!("独立计数：{}", self.clicks))
          .on_click(cx.listener(|this, _, _, cx| {
            this.clicks += 1;
            cx.notify();
          })),
      )
      .child(
        Button::new("new-window-close")
          .label("关闭此窗口")
          .on_click(|_, window, _| window.remove_window()),
      )
  }
}

fn open_example_window(cx: &mut App) -> anyhow::Result<WindowHandle<Root>> {
  let bounds = Bounds::centered(None, size(px(520.), px(320.)), cx);
  cx.open_window(
    WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      window_min_size: Some(size(px(360.), px(240.))),
      titlebar: Some(TitlebarOptions {
        title: Some("GPUI · 新建 Window".into()),
        ..Default::default()
      }),
      ..Default::default()
    },
    |window, cx| {
      window.set_window_title("GPUI · 新建 Window");
      window.activate_window();
      let content = cx.new(|_| ExampleWindow::default());
      // 每个原生窗口各自包装 Root，以支持 Kit 主题、焦点和组件层。
      cx.new(|cx| Root::new(content, window, cx))
    },
  )
}

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  // 应用已在 main 中执行 gpui_kit::init；这里不再创建另一个 Application。
  let status = window.use_keyed_state("new-window-status", cx, |_, _| {
    "点击按钮创建窗口；再次点击可再建一个，计数互不影响。".to_owned()
  });
  let message = status.read(cx).clone();
  div()
    .flex()
    .flex_col()
    .gap_3()
    .child(
      Button::new("element-open-window")
        .label("新建 Window")
        .on_click(move |_, _, cx| {
          let message = match open_example_window(cx) {
            Ok(_) => "已创建独立窗口，可拖动、缩放、计数或关闭。".to_owned(),
            Err(error) => format!("创建窗口失败：{error}"),
          };
          status.update(cx, |status, cx| {
            *status = message;
            cx.notify();
          });
        }),
    )
    .child(message)
    .into_any_element()
}

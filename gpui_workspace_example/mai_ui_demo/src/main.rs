mod travel;
use gpui_kit::{component::Root, *};
fn main() {
  gpui_kit::application().run(|cx| {
    gpui_kit::init(cx);
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(480.), px(900.)),
        cx,
      ))),
      window_min_size: Some(size(px(390.), px(560.))),
      titlebar: Some(TitlebarOptions {
        title: Some("远行 · 去有风的地方".into()),
        ..Default::default()
      }),
      ..Default::default()
    };
    if let Err(error) = cx.open_window(options, |window, cx| {
      let view = cx.new(|_| travel::TravelApp::default());
      cx.new(|cx| Root::new(view, window, cx))
    }) {
      eprintln!("failed to open window: {error}");
      cx.quit();
    }
    cx.activate(true);
  });
}

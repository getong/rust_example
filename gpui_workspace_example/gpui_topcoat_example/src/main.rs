mod counter;
mod demo_page;
mod tabs;
mod studio;

use gpui_kit::{component::Root, *};
use topcoat_gpui_protocol::DEFAULT_SERVER_URL;

fn main() {
  let server = std::env::var("TOPCOAT_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.into());
  gpui_kit::application().run(move |cx| {
    gpui_kit::init(cx);
    gpui_router::init(cx);
    // 关闭最后一个窗口时退出，避免留下无窗口的桌面进程。
    cx.on_window_closed(|cx, _| {
      if cx.windows().is_empty() {
        cx.quit();
      }
    })
    .detach();
    cx.open_window(WindowOptions::default(), |window, cx| {
      let view = cx.new(|cx| tabs::Workspace::new(server, window, cx));
      cx.new(|cx| Root::new(view, window, cx))
    })
    .expect("Failed to open desktop window");
    cx.activate(true);
  });
}

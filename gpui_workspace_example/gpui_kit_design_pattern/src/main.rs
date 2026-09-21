mod app;
mod composition;
mod events;
mod global_state;
mod observe;
mod slots;

use gpui_kit::{component::Root, *};

fn main() {
  gpui_kit::application().run(|cx| {
    gpui_kit::init(cx);
    global_state::init(cx);
    cx.spawn(async move |cx| {
      if let Err(error) = cx.open_window(WindowOptions::default(), |window, cx| {
        let view = cx.new(app::PatternGallery::new);
        cx.new(|cx| Root::new(view, window, cx))
      }) {
        eprintln!("Failed to open window: {error}");
      }
    })
    .detach();
  });
}

mod model;
mod view;

use gpui_kit::{
  component::{Root, TitleBar},
  *,
};
use model::AppModel;
use view::AppView;

fn main() {
  let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

  app.run(move |cx| {
    gpui_kit::init(cx);

    cx.spawn(async move |cx| {
      cx.open_window(TitleBar::window_options(), |window, cx| {
        let model = cx.new(|_| AppModel::default());
        let view = cx.new(|cx| AppView::new(model, window, cx));
        cx.new(|cx| Root::new(view, window, cx))
      })
      .expect("failed to open window");
    })
    .detach();
  });
}

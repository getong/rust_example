use gpui_kit::{
  component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
  },
  prelude::FluentBuilder as _,
  *,
};

use super::{
  bottom_nav::BottomNavView, content_view::ContentView, drawer_view::DrawerView, title_bar,
};
use crate::model::AppModel;

pub struct AppView {
  model: Entity<AppModel>,
  drawer: Entity<DrawerView>,
  content: Entity<ContentView>,
  bottom_nav: Entity<BottomNavView>,
  _model_subscription: Subscription,
}

impl AppView {
  pub fn new(model: Entity<AppModel>, _: &mut Window, cx: &mut Context<Self>) -> Self {
    let drawer = cx.new(|cx| DrawerView::new(model.clone(), cx));
    let content = cx.new(|cx| ContentView::new(model.clone(), cx));
    let bottom_nav = cx.new(|cx| BottomNavView::new(model.clone(), cx));
    let _model_subscription = cx.observe(&model, |_, _, cx| cx.notify());
    Self {
      model,
      drawer,
      content,
      bottom_nav,
      _model_subscription,
    }
  }

  fn open_drawer(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.model.update(cx, |model, cx| model.open_drawer(cx));
  }
}

impl Render for AppView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let drawer_open = self.model.read(cx).drawer_open();

    v_flex()
      .size_full()
      .relative()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(title_bar::view())
      .child(
        v_flex()
          .flex_1()
          .p_8()
          .gap_4()
          .child(
            h_flex().w_full().items_center().child(
              Button::new("open-drawer")
                .ghost()
                .small()
                .label("☰")
                .on_click(cx.listener(Self::open_drawer)),
            ),
          )
          .child(self.content.clone()),
      )
      .child(self.bottom_nav.clone())
      .when(drawer_open, |this| this.child(self.drawer.clone()))
  }
}

use gpui_kit::{
  component::{
    ActiveTheme as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
  },
  *,
};

use crate::model::{AppModel, Tab};

pub struct BottomNavView {
  model: Entity<AppModel>,
  _model_subscription: Subscription,
}

impl BottomNavView {
  pub fn new(model: Entity<AppModel>, cx: &mut Context<Self>) -> Self {
    let _model_subscription = cx.observe(&model, |_, _, cx| cx.notify());
    Self {
      model,
      _model_subscription,
    }
  }

  fn select_tab(&mut self, tab: Tab, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.model.update(cx, |model, cx| model.select_tab(tab, cx));
  }

  fn tab_button(&self, tab: Tab, id: &'static str, cx: &mut Context<Self>) -> Button {
    let selected = self.model.read(cx).selected_tab() == tab;
    Button::new(id)
      .ghost()
      .small()
      .flex_1()
      .label(tab_label(tab))
      .selected(selected)
      .on_click(cx.listener(move |this, event, window, cx| {
        this.select_tab(tab, event, window, cx);
      }))
  }
}

impl Render for BottomNavView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    h_flex()
      .w_full()
      .h(px(72.))
      .px_4()
      .gap_2()
      .border_t_1()
      .border_color(cx.theme().border)
      .bg(cx.theme().secondary)
      .child(self.tab_button(Tab::Home, "home-tab", cx))
      .child(self.tab_button(Tab::Explore, "explore-tab", cx))
      .child(self.tab_button(Tab::Profile, "profile-tab", cx))
  }
}

fn tab_label(tab: Tab) -> &'static str {
  match tab {
    Tab::Home => "Home",
    Tab::Explore => "Explore",
    Tab::Profile => "Profile",
  }
}

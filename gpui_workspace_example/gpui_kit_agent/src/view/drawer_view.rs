use gpui_kit::{
  component::{
    ActiveTheme as _, Selectable as _, Sizable as _, StyledExt as _, TITLE_BAR_HEIGHT,
    accordion::Accordion,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
  },
  *,
};

use crate::model::{AppModel, DrawerAction, Tab};

pub struct DrawerView {
  model: Entity<AppModel>,
  _model_subscription: Subscription,
}

impl DrawerView {
  pub fn new(model: Entity<AppModel>, cx: &mut Context<Self>) -> Self {
    let _model_subscription = cx.observe(&model, |_, _, cx| cx.notify());
    Self {
      model,
      _model_subscription,
    }
  }

  fn close(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.model.update(cx, |model, cx| model.close_drawer(cx));
  }

  fn select_tab(&mut self, tab: Tab, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self
      .model
      .update(cx, |model, cx| model.select_tab_from_drawer(tab, cx));
  }

  fn select_action(
    &mut self,
    action: DrawerAction,
    _: &ClickEvent,
    _: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self
      .model
      .update(cx, |model, cx| model.select_drawer_action(action, cx));
  }

  fn navigation_button(&self, tab: Tab, id: &'static str, cx: &mut Context<Self>) -> Button {
    let selected = self.model.read(cx).selected_tab() == tab;
    Button::new(id)
      .ghost()
      .small()
      .w_full()
      .label(tab_label(tab))
      .selected(selected)
      .on_click(cx.listener(move |this, event, window, cx| {
        this.select_tab(tab, event, window, cx);
      }))
  }

  fn action_button(
    &self,
    action: DrawerAction,
    id: &'static str,
    cx: &mut Context<Self>,
  ) -> Button {
    let selected = self.model.read(cx).selected_action() == action;
    Button::new(id)
      .ghost()
      .small()
      .w_full()
      .label(action_title(action))
      .selected(selected)
      .on_click(cx.listener(move |this, event, window, cx| {
        this.select_action(action, event, window, cx);
      }))
  }
}

impl Render for DrawerView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .absolute()
      .top(TITLE_BAR_HEIGHT)
      .left_0()
      .bottom_0()
      .w(px(320.))
      .bg(cx.theme().background)
      .border_r_1()
      .border_color(cx.theme().border)
      .shadow_xl()
      .child(
        v_flex()
          .size_full()
          .gap_4()
          .p_4()
          .child(
            h_flex()
              .w_full()
              .items_center()
              .justify_between()
              .child(div().font_semibold().child("Navigation"))
              .child(
                Button::new("close-drawer")
                  .ghost()
                  .small()
                  .label("Close")
                  .on_click(cx.listener(Self::close)),
              ),
          )
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child("Quickly switch pages, browse settings, and view app information here."),
          )
          .child(
            v_flex()
              .gap_2()
              .child(div().text_sm().font_semibold().child("Quick Navigation"))
              .child(self.navigation_button(Tab::Home, "drawer-home", cx))
              .child(self.navigation_button(Tab::Explore, "drawer-explore", cx))
              .child(self.navigation_button(Tab::Profile, "drawer-profile", cx)),
          )
          .child(
            Accordion::new("drawer-sections")
              .multiple(true)
              .item(|item| {
                item.title("Settings").open(true).child(
                  v_flex()
                    .gap_2()
                    .child(self.action_button(
                      DrawerAction::Notifications,
                      "drawer-notifications",
                      cx,
                    ))
                    .child(self.action_button(DrawerAction::Appearance, "drawer-appearance", cx))
                    .child(self.action_button(DrawerAction::Security, "drawer-security", cx)),
                )
              })
              .item(|item| {
                item.title("Information").open(true).child(
                  v_flex()
                    .gap_2()
                    .child(self.action_button(DrawerAction::About, "drawer-about", cx))
                    .child(self.action_button(DrawerAction::Version, "drawer-version", cx))
                    .child(self.action_button(DrawerAction::Help, "drawer-help", cx)),
                )
              }),
          ),
      )
  }
}

fn tab_label(tab: Tab) -> &'static str {
  match tab {
    Tab::Home => "Home",
    Tab::Explore => "Explore",
    Tab::Profile => "Profile",
  }
}

fn action_title(action: DrawerAction) -> &'static str {
  match action {
    DrawerAction::Notifications => "Notification Settings",
    DrawerAction::Appearance => "Appearance Preferences",
    DrawerAction::Security => "Security & Privacy",
    DrawerAction::About => "About the App",
    DrawerAction::Version => "Version Information",
    DrawerAction::Help => "Help",
    DrawerAction::Overview => "Ready",
  }
}

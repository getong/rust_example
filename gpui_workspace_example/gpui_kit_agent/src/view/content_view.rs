use gpui_kit::{
  component::{ActiveTheme as _, StyledExt as _, v_flex},
  *,
};

use crate::model::{AppModel, DrawerAction, Tab};

pub struct ContentView {
  model: Entity<AppModel>,
  _model_subscription: Subscription,
}

impl ContentView {
  pub fn new(model: Entity<AppModel>, cx: &mut Context<Self>) -> Self {
    let _model_subscription = cx.observe(&model, |_, _, cx| cx.notify());
    Self {
      model,
      _model_subscription,
    }
  }
}

impl Render for ContentView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let model = self.model.read(cx);
    let tab = model.selected_tab();
    let action = model.selected_action();
    let tab_title = tab_title(tab);

    v_flex()
      .gap_4()
      .child(div().text_2xl().font_semibold().child(tab_title))
      .child(
        div()
          .text_color(cx.theme().muted_foreground)
          .child(tab_description(tab)),
      )
      .child(
        div()
          .mt_4()
          .p_6()
          .rounded(cx.theme().radius)
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().secondary)
          .child(format!("Current page: {tab_title}")),
      )
      .child(
        div()
          .p_6()
          .rounded(cx.theme().radius)
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().secondary)
          .child(
            v_flex()
              .gap_2()
              .child(div().font_semibold().child(action_title(action)))
              .child(
                div()
                  .text_color(cx.theme().muted_foreground)
                  .child(action_description(action)),
              ),
          ),
      )
  }
}

fn tab_title(tab: Tab) -> &'static str {
  match tab {
    Tab::Home => "Home",
    Tab::Explore => "Explore",
    Tab::Profile => "Profile",
  }
}

fn tab_description(tab: Tab) -> &'static str {
  match tab {
    Tab::Home => "This is your home page, where you can quickly catch up on recent activity.",
    Tab::Explore => "Browse new content and discover more interesting things.",
    Tab::Profile => "Manage your profile and app settings.",
  }
}

fn action_title(action: DrawerAction) -> &'static str {
  match action {
    DrawerAction::Overview => "Ready",
    DrawerAction::Notifications => "Notification Settings",
    DrawerAction::Appearance => "Appearance Preferences",
    DrawerAction::Security => "Security & Privacy",
    DrawerAction::About => "About the App",
    DrawerAction::Version => "Version Information",
    DrawerAction::Help => "Help",
  }
}

fn action_description(action: DrawerAction) -> &'static str {
  match action {
    DrawerAction::Overview => {
      "Quickly switch navigation, browse setting groups, or open app information from the drawer."
    }
    DrawerAction::Notifications => {
      "Manage reminder frequency, message delivery, and banner notifications."
    }
    DrawerAction::Appearance => {
      "Adjust the theme, interface density, and reading area preferences."
    }
    DrawerAction::Security => {
      "Find options for signed-in devices, privacy permissions, and account protection."
    }
    DrawerAction::About => {
      "Learn about the app, its core capabilities, and the purpose of this example page."
    }
    DrawerAction::Version => {
      "The current version is 0.1.0. This is a good place to show the build number and changelog."
    }
    DrawerAction::Help => {
      "Find frequently asked questions, usage instructions, and feedback channels."
    }
  }
}

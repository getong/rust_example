use gpui_kit::Context;

use super::{DrawerAction, Tab};

/// The shared reactive application model.
pub struct AppModel {
  drawer_open: bool,
  selected_tab: Tab,
  selected_action: DrawerAction,
}

impl Default for AppModel {
  fn default() -> Self {
    Self {
      drawer_open: false,
      selected_tab: Tab::default(),
      selected_action: DrawerAction::default(),
    }
  }
}

impl AppModel {
  pub fn drawer_open(&self) -> bool {
    self.drawer_open
  }

  pub fn selected_tab(&self) -> Tab {
    self.selected_tab
  }

  pub fn selected_action(&self) -> DrawerAction {
    self.selected_action
  }

  pub fn open_drawer(&mut self, cx: &mut Context<Self>) {
    self.drawer_open = true;
    cx.notify();
  }

  pub fn close_drawer(&mut self, cx: &mut Context<Self>) {
    self.drawer_open = false;
    cx.notify();
  }

  pub fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
    self.selected_tab = tab;
    cx.notify();
  }

  pub fn select_tab_from_drawer(&mut self, tab: Tab, cx: &mut Context<Self>) {
    self.selected_tab = tab;
    self.drawer_open = false;
    cx.notify();
  }

  pub fn select_drawer_action(&mut self, action: DrawerAction, cx: &mut Context<Self>) {
    self.selected_action = action;
    self.drawer_open = false;
    cx.notify();
  }
}

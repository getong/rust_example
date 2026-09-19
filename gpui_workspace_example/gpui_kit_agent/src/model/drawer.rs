#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerAction {
  Overview,
  Notifications,
  Appearance,
  Security,
  About,
  Version,
  Help,
}

impl Default for DrawerAction {
  fn default() -> Self {
    Self::Overview
  }
}

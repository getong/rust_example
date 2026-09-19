#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
  Home,
  Explore,
  Profile,
}

impl Default for Tab {
  fn default() -> Self {
    Self::Home
  }
}

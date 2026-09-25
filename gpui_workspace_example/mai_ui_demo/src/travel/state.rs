//! State shared across tabs; switching tabs preserves user selections.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Page {
  #[default]
  Home,
  Trips,
  Saved,
  Profile,
}

pub(super) struct TravelState {
  pub(super) page: Page,
  pub(super) day: usize,
  pub(super) saved: bool,
  pub(super) ready: [bool; 3],
  pub(super) notifications: bool,
}
impl Default for TravelState {
  fn default() -> Self {
    Self {
      page: Page::Home,
      day: 0,
      saved: false,
      ready: [true, true, false],
      notifications: false,
    }
  }
}

use gpui_kit::{AnyView, Entity, EntityId, Render, SharedString};
use gpui_router::Route;

/// Logical identity is independent of tab order and page state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabId {
  Baidu,
  Component(&'static str),
  Counter(usize),
  Toast(EntityId),
  Scrollbar(EntityId),
  Directory(EntityId),
}

impl TabId {
  fn path(self) -> SharedString {
    match self {
      Self::Baidu => "/baidu/top".to_owned(),
      Self::Component(slug) => format!("/component/{slug}"),
      Self::Counter(number) => format!("/counter/{number}"),
      Self::Toast(id) => format!("/toast/{id}"),
      Self::Scrollbar(id) => format!("/scrollbar/{id}"),
      Self::Directory(id) => format!("/tabs/{id}"),
    }
    .into()
  }
}

/// Owns an open page. Route factories reuse this Entity rather than rebuilding it.
pub(crate) struct PanelTab {
  pub(crate) id: TabId,
  path: SharedString,
  label: SharedString,
  view: AnyView,
}

impl PanelTab {
  pub(crate) fn new<T: Render>(id: TabId, label: impl Into<SharedString>, view: Entity<T>) -> Self {
    Self {
      id,
      path: id.path(),
      label: label.into(),
      view: view.into(),
    }
  }

  pub(crate) fn path(&self) -> SharedString {
    self.path.clone()
  }

  pub(crate) fn label(&self) -> SharedString {
    self.label.clone()
  }

  pub(crate) fn route(&self) -> Route {
    let view = self.view.clone();
    Route::new()
      .path(self.path.trim_start_matches('/').to_owned())
      .element(move |_, _| view.clone())
  }

  #[cfg(test)]
  pub(crate) fn entity<T: 'static>(&self) -> Entity<T> {
    self
      .view
      .clone()
      .downcast()
      .expect("unexpected tab view type")
  }

  #[cfg(test)]
  pub(crate) fn counter(&self) -> Entity<crate::CounterTab> {
    self.entity()
  }
}

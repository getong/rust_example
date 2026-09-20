use gpui_kit::{
  App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement as _,
  Render, Styled, Window,
  component::{ActiveTheme as _, dock::PanelControl, v_flex},
  div, img, px,
};

use crate::tabs::section;

pub struct ImageTab {
  focus_handle: gpui_kit::FocusHandle,
}

impl super::ComponentPage for ImageTab {
  fn title() -> &'static str {
    "Image"
  }

  fn description() -> &'static str {
    "Image and SVG image supported."
  }

  fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    Self::view(window, cx)
  }

  fn zoomable() -> Option<PanelControl> {
    Some(PanelControl::Toolbar)
  }
}

impl ImageTab {
  pub fn new(_: &mut Window, cx: &mut App) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
    }
  }

  pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    cx.new(|cx| Self::new(window, cx))
  }
}

impl Focusable for ImageTab {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ImageTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex().size_full().items_center().gap_6().child(
      section("Remote SVG")
        .description("Loads and renders an SVG from a remote URL.")
        .w(px(480.))
        .child(
          div()
            .w_full()
            .h(px(180.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .child(img("https://pub.lbkrs.com/files/202503/vEnnmgUM6bo362ya/sdk.svg").h_24()),
        ),
    )
  }
}

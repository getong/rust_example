//! Components 总览标签；单个组件的实现位于同级 *_tab.rs。
use gpui_kit::{
  component::{button::Button, scroll::ScrollableElement, v_flex},
  *,
};

use super::catalog::DEMOS;
use crate::TabbedPanel;
pub(crate) fn render(panel: &WeakEntity<TabbedPanel>) -> AnyElement {
  v_flex()
    .id("component-catalog")
    .size_full()
    .overflow_y_scrollbar()
    .gap_2()
    .p_4()
    .children(DEMOS.iter().enumerate().map(|(index, demo)| {
      let panel = panel.clone();
      Button::new(("open-component", index))
        .w_full()
        .label(format!("{} · {}", demo.title, demo.description))
        .on_click(move |_, _, cx| {
          if let Some(panel) = panel.upgrade() {
            panel.update(cx, |panel, cx| panel.open_component(Some(index), cx));
          }
        })
    }))
    .into_any_element()
}

use gpui_kit::{
  component::{
    ActiveTheme, button::Button, h_flex, label::Label, scroll::ScrollableElement, v_flex,
  },
  *,
};

use crate::{TabbedPanel, tabs::catalog::DEMOS};

pub(crate) struct ComponentTab {
  pub(crate) index: Option<usize>,
  panel: WeakEntity<TabbedPanel>,
  content: Option<AnyView>,
}
impl ComponentTab {
  pub(crate) fn new(index: Option<usize>, panel: WeakEntity<TabbedPanel>) -> Self {
    Self {
      index,
      panel,
      content: None,
    }
  }
  pub(crate) fn title(&self) -> &'static str {
    self.index.map_or("Components", |i| DEMOS[i].title)
  }
}
impl Render for ComponentTab {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let title = self.title();
    let description = self.index.map_or(
      "点击组件名称查看真实交互示例。每个组件有独立标签，关闭后可从此处重新打开。",
      |i| DEMOS[i].description,
    );
    // 首次显示时创建一次，后续 render 复用 Entity，切换标签保留控件状态。
    if let Some(index) = self.index
      && self.content.is_none()
    {
      self.content = Some((DEMOS[index].build)(window, cx));
    }
    let body = if let Some(content) = self.content.clone() {
      v_flex()
        .id("component-demo-content")
        .size_full()
        .overflow_y_scrollbar()
        .child(v_flex().size_full().p_4().child(content))
        .into_any_element()
    } else {
      crate::tabs::components_tab::render(&self.panel)
    };
    let panel = self.panel.clone();
    v_flex()
      .size_full()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        h_flex()
          .gap_3()
          .p_4()
          .child(Label::new(title).text_2xl())
          .child(
            Button::new("component-overview")
              .label("All components")
              .on_click(move |_, _, cx| {
                if let Some(panel) = panel.upgrade() {
                  panel.update(cx, |panel, cx| panel.open_component(None, cx));
                }
              }),
          ),
      )
      .child(
        Label::new(description)
          .px_4()
          .pb_3()
          .text_color(cx.theme().muted_foreground),
      )
      .child(v_flex().flex_1().min_h_0().child(body))
  }
}

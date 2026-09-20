use gpui_kit::{
  component::{
    ActiveTheme,
    button::Button,
    scroll::{Scrollbar, ScrollbarMode},
  },
  *,
};

use crate::TabbedPanel;

pub(crate) struct TabDirectory {
  panel: WeakEntity<TabbedPanel>,
  pub(crate) scroll_handle: ScrollHandle,
  _subscription: Subscription,
}

impl TabDirectory {
  pub(crate) fn new(panel: &Entity<TabbedPanel>, cx: &mut Context<Self>) -> Self {
    Self {
      panel: panel.downgrade(),
      scroll_handle: ScrollHandle::new(),
      _subscription: cx.observe(panel, |_, _, cx| cx.notify()),
    }
  }
}

impl Render for TabDirectory {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // 从面板实时派生目录，只复制文字，不持有目标页面的 Entity。
    let entries = self.panel.upgrade().map_or_else(Vec::new, |panel| {
      panel
        .read(cx)
        .tabs
        .iter()
        .map(|tab| (tab.path(cx), tab.label(cx)))
        .collect::<Vec<_>>()
    });
    div()
      .size_full()
      .flex()
      .flex_col()
      .gap_3()
      .p_4()
      .child(div().text_2xl().child("Tab directory"))
      .child(format!(
        "{} open tabs · Click a tab to switch",
        entries.len()
      ))
      .child(
        div()
          .relative()
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .bg(cx.theme().background)
          .text_color(cx.theme().foreground)
          .border_1()
          .border_color(cx.theme().border)
          .rounded_lg()
          .child(
            div()
              .id("tab-directory-content")
              .size_full()
              .overflow_y_scroll()
              .track_scroll(&self.scroll_handle)
              .pr_4()
              .children(entries.into_iter().map(|(path, label)| {
                div()
                  .h(px(56.))
                  .px_3()
                  .flex()
                  .items_center()
                  .border_b_1()
                  .border_color(cx.theme().border)
                  .child(
                    Button::new(path.clone())
                      .w_full()
                      .label(format!("{label} · {path}"))
                      .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(panel) = view.panel.upgrade() {
                          panel.update(cx, |panel, cx| {
                            // 使用稳定路径，列表增删不会令旧索引跳到其他页面。
                            if let Some(index) =
                              panel.tabs.iter().position(|tab| tab.path(cx) == path)
                            {
                              panel.select_tab(index, cx);
                            }
                          });
                        }
                      })),
                  )
              })),
          )
          .child(Scrollbar::vertical(&self.scroll_handle).mode(ScrollbarMode::Always)),
      )
  }
}

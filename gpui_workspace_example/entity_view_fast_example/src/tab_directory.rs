use gpui_kit::{
  component::{label::Label, list::ListItem, v_flex},
  *,
};

use crate::{TabbedPanel, palette::AppPalette, scroll_panel::ScrollPanel};

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
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // 从面板实时派生目录，只复制文字，不持有目标页面的 Entity。
    let entries = self.panel.upgrade().map_or_else(Vec::new, |panel| {
      panel
        .read(cx)
        .tabs
        .iter()
        .map(|tab| (tab.path(), tab.label()))
        .collect::<Vec<_>>()
    });
    v_flex()
      .size_full()
      .gap_3()
      .p_4()
      .child(
        Label::new("Tab directory")
          .text_2xl()
          .text_color(AppPalette::default().foreground),
      )
      .child(format!(
        "{} open tabs · Click a tab to switch",
        entries.len()
      ))
      .child(
        ScrollPanel::new("tab-directory-content", &self.scroll_handle).children(
          entries.into_iter().map(|(path, label)| {
            let focus = window
              .use_keyed_state(path.clone(), cx, |_, cx| cx.focus_handle())
              .read(cx)
              .clone();
            let text = format!("{label} · {path}");
            ListItem::new(path.clone())
              .h(rems(3.5))
              .role(accesskit::Role::Button)
              .aria_label(text.clone())
              .track_focus(&focus)
              .child(Label::new(text))
              .on_click(cx.listener(move |view, _, _, cx| {
                if let Some(panel) = view.panel.upgrade() {
                  panel.update(cx, |panel, cx| {
                    // 稳定路径不会因列表插入或删除而指向其他标签。
                    panel.navigate(&path, cx);
                  });
                }
              }))
          }),
        ),
      )
  }
}

use gpui_kit::{
  base::Disableable,
  component::{
    button::Button, h_flex, label::Label, list::ListItem, notification::Notification, v_flex,
  },
  *,
};

use crate::{palette::AppPalette, scroll_panel::ScrollPanel, toast_tab::show_toast};

#[derive(Default)]
pub(crate) struct ScrollbarTab {
  // 每个标签保存自己的滚动位置，切换标签时不重建。
  pub(crate) scroll_handle: ScrollHandle,
  selected_row: Option<usize>,
  saved_offset: Option<Point<Pixels>>,
}

impl Render for ScrollbarTab {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .gap_3()
      .p_4()
      .child(
        Label::new("Scrollbar playground")
          .text_2xl()
          .text_color(AppPalette::default().foreground),
      )
      .child("Scroll with the mouse wheel / trackpad, or drag the scrollbar.")
      .child(
        h_flex()
          .flex_wrap()
          .gap_2()
          .child(
            Button::new("scroll-top")
              .label("Back to top")
              .on_click(cx.listener(|view, _, _, cx| {
                view.scroll_handle.set_offset(point(px(0.), px(0.)));
                cx.notify();
              })),
          )
          .child(
            Button::new("scroll-save")
              .label("Remember position")
              .on_click(cx.listener(|view, _, window, cx| {
                view.saved_offset = Some(view.scroll_handle.offset());
                show_toast(
                  Notification::success("Scroll position remembered."),
                  window,
                  cx,
                );
                cx.notify();
              })),
          )
          .child(
            Button::new("scroll-restore")
              .label("Restore position")
              .disabled(self.saved_offset.is_none())
              .on_click(cx.listener(|view, _, window, cx| {
                if let Some(offset) = view.saved_offset {
                  view.scroll_handle.set_offset(offset);
                  show_toast(
                    Notification::info("Returned to your remembered position."),
                    window,
                    cx,
                  );
                  cx.notify();
                }
              })),
          ),
      )
      .child(
        ScrollPanel::new("scroll-content", &self.scroll_handle).children((1 ..= 80).map(
          |number| {
            let focus = window
              .use_keyed_state(("scroll-row", number), cx, |_, cx| cx.focus_handle())
              .read(cx)
              .clone();
            let text = if self.selected_row == Some(number) {
              format!("Row {number:02} — Selected")
            } else {
              format!("Row {number:02} — Click to select")
            };
            ListItem::new(("scroll-row", number))
              .h(rems(3.25))
              .selected(self.selected_row == Some(number))
              .role(accesskit::Role::Button)
              .aria_label(text.clone())
              .track_focus(&focus)
              .child(Label::new(text))
              .on_click(cx.listener(move |view, _, window, cx| {
                view.selected_row = Some(number);
                show_toast(
                  Notification::success(format!("You selected row {number:02}."))
                    .title("Row selected"),
                  window,
                  cx,
                );
                cx.notify();
              }))
          },
        )),
      )
      .child(match self.selected_row {
        Some(number) => format!("Selected: Row {number:02} · 80 clickable rows"),
        None => "80 clickable rows · Select a row to show a toast.".into(),
      })
      .child("Each tab remembers its scroll position and selection while open.")
  }
}

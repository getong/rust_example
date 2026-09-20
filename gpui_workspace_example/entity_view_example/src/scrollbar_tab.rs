use gpui_kit::{
  base::Disableable,
  component::{
    ActiveTheme,
    button::Button,
    notification::Notification,
    scroll::{Scrollbar, ScrollbarMode},
  },
  *,
};

use crate::toast_tab::show_toast;

#[derive(Default)]
pub(crate) struct ScrollbarTab {
  // 每个标签保存自己的滚动位置，切换标签时不重建。
  pub(crate) scroll_handle: ScrollHandle,
  selected_row: Option<usize>,
  saved_offset: Option<Point<Pixels>>,
}

impl Render for ScrollbarTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .flex()
      .flex_col()
      .gap_3()
      .p_4()
      .child(div().text_2xl().child("Scrollbar playground"))
      .child("Scroll with the mouse wheel / trackpad, or drag the scrollbar.")
      .child(
        div()
          .flex()
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
              .id("scroll-content")
              .size_full()
              .overflow_y_scroll()
              .track_scroll(&self.scroll_handle)
              .pr_4()
              .children((1 ..= 80).map(|number| {
                div()
                  .h(px(52.))
                  .px_4()
                  .flex()
                  .items_center()
                  .border_b_1()
                  .border_color(cx.theme().border)
                  .bg(if self.selected_row == Some(number) {
                    cx.theme().secondary
                  } else {
                    cx.theme().background
                  })
                  .child(
                    Button::new(("scroll-row", number))
                      .w_full()
                      .label(if self.selected_row == Some(number) {
                        format!("Row {number:02} — Selected")
                      } else {
                        format!("Row {number:02} — Click to select")
                      })
                      .on_click(cx.listener(move |view, _, window, cx| {
                        view.selected_row = Some(number);
                        show_toast(
                          Notification::success(format!("You selected row {number:02}."))
                            .title("Row selected"),
                          window,
                          cx,
                        );
                        cx.notify();
                      })),
                  )
              })),
          )
          .child(Scrollbar::vertical(&self.scroll_handle).mode(ScrollbarMode::Always)),
      )
      .child(match self.selected_row {
        Some(number) => format!("Selected: Row {number:02} · 80 clickable rows"),
        None => "80 clickable rows · Select a row to show a toast.".into(),
      })
      .child("Each tab remembers its scroll position and selection while open.")
  }
}

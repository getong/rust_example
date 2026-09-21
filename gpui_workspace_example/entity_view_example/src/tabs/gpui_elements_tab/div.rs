use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let count = window.use_keyed_state("element-div-count", cx, |_, _| 0usize);
    let clicks = *count.read(cx);
    div()
      .id("element-div-button")
      .test_support()
      .role(accesskit::Role::Button)
      .aria_label(format!("div 点击次数：{clicks}"))
      .flex()
      .items_center()
      .justify_center()
      .w(px(240.))
      .h(px(56.))
      .rounded_lg()
      .bg(rgb(0x2563eb))
      .text_color(rgb(0xffffff))
      .cursor_pointer()
      .hover(|s| s.bg(rgb(0x1d4ed8)))
      .on_click(move |_, _, cx| {
        count.update(cx, |value, cx| {
          *value += 1;
          cx.notify();
        })
      })
      .child(format!("div 点击次数：{clicks}"))
  };
  preview.into_any_element()
}

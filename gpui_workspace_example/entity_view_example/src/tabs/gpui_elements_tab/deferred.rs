use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let enabled = window.use_keyed_state("deferred-enabled", cx, |_, _| true);
    let use_deferred = *enabled.read(cx);
    let overlay = div()
      .absolute()
      .top(px(8.))
      .left(px(8.))
      .w(px(200.))
      .h(px(64.))
      .bg(rgb(0x2563eb))
      .text_color(rgb(0xffffff))
      .child("蓝色层：先声明");
    let overlay = if use_deferred {
      deferred(overlay).with_priority(1).into_any_element()
    } else {
      overlay.into_any_element()
    };
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .id("element-deferred-toggle")
          .test_support()
          .cursor_pointer()
          .child(format!("deferred = {use_deferred}，点击切换"))
          .on_click(move |_, _, cx| {
            enabled.update(cx, |enabled, cx| {
              *enabled = !*enabled;
              cx.notify();
            })
          }),
      )
      .child(
        div().relative().h(px(110.)).w_full().child(overlay).child(
          div()
            .absolute()
            .top(px(32.))
            .left(px(72.))
            .w(px(200.))
            .h(px(64.))
            .bg(rgb(0xd97706))
            .text_color(rgb(0xffffff))
            .child("橙色层：后声明"),
        ),
      )
  };
  preview.into_any_element()
}

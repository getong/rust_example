use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let open = window.use_keyed_state("anchored-open", cx, |_, _| false);
    let visible = *open.read(cx);
    let mut preview = div().relative().h(px(130.)).w_full().child(
      div()
        .id("element-anchor-toggle")
        .test_support()
        .w(px(180.))
        .h(px(36.))
        .cursor_pointer()
        .child("点击显示 / 隐藏锚定层")
        .on_click(move |_, _, cx| {
          open.update(cx, |open, cx| {
            *open = !*open;
            cx.notify();
          })
        }),
    );
    if visible {
      preview = preview.child(
        anchored()
          .anchor(Anchor::TopLeft)
          .position_mode(AnchoredPositionMode::Local)
          .position(point(px(12.), px(44.)))
          .snap_to_window()
          .child(
            div()
              .p_3()
              .rounded_lg()
              .bg(rgb(0x334155))
              .text_color(rgb(0xffffff))
              .child("按锚点定位，并避让窗口边界"),
          ),
      );
    }
    preview
  };
  preview.into_any_element()
}

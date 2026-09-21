use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let wide = window.use_keyed_state("query-wide", cx, |_, _| false);
    let width = if *wide.read(cx) { 400. } else { 220. };
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .id("element-query-toggle")
          .test_support()
          .cursor_pointer()
          .child("点击切换 220px / 400px")
          .on_click(move |_, _, cx| {
            wide.update(cx, |wide, cx| {
              *wide = !*wide;
              cx.notify();
            })
          }),
      )
      .child(
        container_query(|size, _, _| {
          let narrow = size.width < px(300.);
          div()
            .size_full()
            .p_3()
            .rounded_lg()
            .bg(if narrow { rgb(0x334155) } else { rgb(0x047857) })
            .text_color(rgb(0xffffff))
            .child(if narrow {
              "窄容器：单列内容"
            } else {
              "宽容器：更多内容空间"
            })
        })
        .w(px(width))
        .h(px(80.)),
      )
  };
  preview.into_any_element()
}

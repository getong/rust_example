use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let expanded = window.use_keyed_state("spring-expanded", cx, |_, _| false);
    let target = if *expanded.read(cx) {
      px(240.)
    } else {
      px(60.)
    };
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .id("element-spring-toggle")
          .test_support()
          .cursor_pointer()
          .child("切换弹簧目标宽度 60 / 240px")
          .on_click(move |_, _, cx| {
            expanded.update(cx, |value, cx| {
              *value = !*value;
              cx.notify();
            })
          }),
      )
      .child(div().h(px(48.)).rounded_lg().bg(rgb(0x059669)).with_spring(
        "element-spring",
        SpringAnimation::new(SpringConfig::new(170., 15., 1.)).to(target),
        |this, width| this.w(width),
      ))
  };
  preview.into_any_element()
}

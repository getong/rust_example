use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let visible = window.use_keyed_state("empty-visible", cx, |_, _| false);
    let show = *visible.read(cx);
    let content = if show {
      div()
        .p_3()
        .bg(rgb(0x2563eb))
        .text_color(rgb(0xffffff))
        .child("可选内容")
        .into_any_element()
    } else {
      Empty.into_any_element()
    };
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .id("element-empty-toggle")
          .test_support()
          .cursor_pointer()
          .child("切换 Empty / 内容")
          .on_click(move |_, _, cx| {
            visible.update(cx, |visible, cx| {
              *visible = !*visible;
              cx.notify();
            })
          }),
      )
      .child(content)
      .child("Empty 不占布局空间；需要固定占位时使用带尺寸的 div。")
  };
  preview.into_any_element()
}

use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let count = window.use_keyed_state("interactive-text-count", cx, |_, _| 0usize);
    let clicks = *count.read(cx);
    let label = "Click this link";
    let text = StyledText::new(label).with_highlights([(
      0 .. label.len(),
      HighlightStyle {
        color: Some(rgb(0x2563eb).into()),
        ..Default::default()
      },
    )]);
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .id("element-text-link")
          .test_support()
          .w(px(160.))
          .child(InteractiveText::new("interactive-link", text).on_click(
            vec![0 .. label.len()],
            move |_, _, cx| {
              count.update(cx, |count, cx| {
                *count += 1;
                cx.notify();
              });
            },
          )),
      )
      .child(format!("链接点击次数：{clicks}；范围使用 UTF-8 字节索引"))
  };
  preview.into_any_element()
}

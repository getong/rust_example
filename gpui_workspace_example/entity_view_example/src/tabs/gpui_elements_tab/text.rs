use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(gpui_kit::text!(
        id = "element-text",
        "Text：带稳定 ID 的普通文本"
      ))
      .child(StyledText::new("GPUI: highlighted text").with_highlights([(
        6 .. 17,
        HighlightStyle {
          color: Some(rgb(0x2563eb).into()),
          font_weight: Some(FontWeight::BOLD),
          ..Default::default()
        },
      )]))
  };
  preview.into_any_element()
}

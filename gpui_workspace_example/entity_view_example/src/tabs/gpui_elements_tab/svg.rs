use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    let data = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><path d="M24 4 L44 40 H4 Z M24 16 V28 M24 32 V35" fill="none" stroke="black" stroke-width="3" stroke-linejoin="round"/></svg>"##;
    div()
      .flex()
      .items_center()
      .gap_4()
      .child(svg().data(data).size(px(40.)).text_color(rgb(0x2563eb)))
      .child(svg().data(data).size(px(80.)).text_color(rgb(0x059669)))
  };
  preview.into_any_element()
}

use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    // Drawable::new 是内部 API；AnyElement 内部持有 Drawable。
    // 手动绘制需按 layout → prepaint → paint 顺序调用，不能在 render 中直接 paint。
    canvas(
      |bounds, window, cx| {
        let mut element = div()
          .size_full()
          .p_3()
          .rounded_lg()
          .bg(rgb(0x334155))
          .text_color(rgb(0xffffff))
          .child("通过 AnyElement 手动布局和绘制")
          .into_any_element();
        element.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
        element.prepaint_at(bounds.origin, window, cx);
        element
      },
      |_, mut element, window, cx| element.paint(window, cx),
    )
    .w_full()
    .h(px(80.))
  };
  preview.into_any_element()
}

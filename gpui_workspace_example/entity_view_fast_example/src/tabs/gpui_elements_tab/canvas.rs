use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    canvas(
      |bounds, _, _| Bounds {
        origin: bounds.origin + point(px(12.), px(12.)),
        size: size(bounds.size.width * 0.65, bounds.size.height - px(24.)),
      },
      |bounds, inner, window, _| {
        window.paint_quad(fill(bounds, rgb(0x1e293b)));
        window.paint_quad(fill(inner, rgb(0x38bdf8)));
      },
    )
    .w_full()
    .h(px(100.))
  };
  preview.into_any_element()
}

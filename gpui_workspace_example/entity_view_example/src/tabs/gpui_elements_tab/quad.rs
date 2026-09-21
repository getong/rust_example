use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    canvas(
      |_, _, _| (),
      |bounds, _, window, _| {
        // quad() 返回 PaintQuad；交给 window.paint_quad，而不是 parent.child。
        let shape: PaintQuad = quad(
          bounds,
          px(12.),
          rgb(0x2563eb),
          px(3.),
          rgb(0x38bdf8),
          BorderStyle::default(),
        );
        window.paint_quad(shape);
      },
    )
    .w(px(240.))
    .h(px(80.))
  };
  preview.into_any_element()
}

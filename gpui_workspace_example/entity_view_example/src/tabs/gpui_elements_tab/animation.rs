use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    use std::time::Duration;
    let revision = window.use_keyed_state("animation-revision", cx, |_, _| 0usize);
    let generation = *revision.read(cx);
    div()
      .flex()
      .flex_col()
      .gap_3()
      .child(
        div()
          .id("element-animation-replay")
          .test_support()
          .cursor_pointer()
          .child("点击重播 2 秒动画")
          .on_click(move |_, _, cx| {
            revision.update(cx, |value, cx| {
              *value += 1;
              cx.notify();
            })
          }),
      )
      .child(
        div()
          .h(px(48.))
          .rounded_lg()
          .bg(rgb(0x2563eb))
          .with_animation(
            ("element-animation", generation),
            Animation::new(Duration::from_secs(2)).with_easing(ease_in_out),
            |this, delta| this.w(px(60. + 180. * delta)).opacity(0.3 + 0.7 * delta),
          ),
      )
  };
  preview.into_any_element()
}

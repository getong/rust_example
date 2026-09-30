{
  use std::time::Duration;

  use gpui_kit::{Animation, AnimationExt as _};
  let primary = cx.theme().primary;
  let muted = cx.theme().muted;
  div()
    .id("div-extension-animation-preview")
    .test_support()
    .flex()
    .flex_col()
    .gap_3()
    .w_full()
    .child(
      Button::new("div-replay-animation")
        .label("重播宽度与透明度动画")
        .on_click(cx.listener(|this, _, _, cx| {
          this.animation_runs += 1;
          cx.notify();
        })),
    )
    .child("2 秒内：宽度 48 → 240px，透明度 0.4 → 1.0；文字和间距保持不变")
    .child(
      div()
        .h(px(64.))
        .w_full()
        .flex()
        .items_center()
        .px_3()
        .bg(muted)
        .rounded_lg()
        .child(div().h(px(40.)).rounded_md().bg(primary).with_animation(
          ("div-extension-animation", self.animation_runs),
          Animation::new(Duration::from_secs(2)),
          |this, delta| this.w(px(48. + 192. * delta)).opacity(0.4 + 0.6 * delta),
        )),
    )
}

{
  use std::time::Duration;

  use gpui_kit::{Animation, AnimationExt as _};
  let primary = cx.theme().primary;
  let muted = cx.theme().muted;
  let border = cx.theme().border;
  div()
    .id("div-extension-animation-preview")
    .test_support()
    .flex()
    .flex_col()
    .gap_3()
    .w_full()
    .child(
      Button::new("div-replay-animation")
        .label("重播组合动画")
        .on_click(cx.listener(|this, _, _, cx| {
          this.animation_runs = this.animation_runs.wrapping_add(1);
          cx.notify();
        })),
    )
    .child("卡片错峰滑入、逐渐显现，进度条随后填充；固定槽位避免挤动相邻内容。")
    .child(
      div().flex().flex_wrap().gap_3().children(
        ["准备", "处理中", "完成"].into_iter().enumerate().map(|(index, label)| {
          div().relative().w(px(144.)).h(px(100.)).child(
            div().absolute().w_full().h(px(84.)).p_3().rounded_lg()
              .bg(muted).border_1().border_color(border)
              .flex().flex_col().gap_3().child(label)
              .child(
                div().h(px(6.)).rounded_full().bg(primary).with_animation(
                  (gpui_kit::SharedString::from(format!("div-progress-{index}")), self.animation_runs),
                  Animation::new(Duration::from_millis(1800)),
                  move |el, t| {
                    let progress = ((t - 0.3 - index as f32 * 0.12) / 0.4).clamp(0., 1.);
                    el.w(relative(progress))
                  },
                ),
              )
              .with_animation(
                (gpui_kit::SharedString::from(format!("div-enter-{index}")), self.animation_runs),
                Animation::new(Duration::from_millis(1800)),
                move |el, t| {
                  let progress = ((t - index as f32 * 0.15) / 0.45).clamp(0., 1.);
                  let eased = 1. - (1. - progress).powi(3);
                  el.top(px(16. * (1. - eased))).opacity(eased)
                },
              ),
          )
        }),
      ),
    )
    .child("更多效果：在 GPUI 元素页选择 AnimationElement 或 SpringAnimationElement，可对比缓动、轨道与不同阻尼。")
}

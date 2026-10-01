use std::{f32::consts::TAU, time::Duration};

use gpui_kit::{
  component::{ActiveTheme, Selectable, button::Button},
  *,
};

#[derive(Default)]
struct Playback {
  generation: usize,
  slow: bool,
  looping: bool,
}

// Keep a short hold after the entrance so the finished composition can be read.
fn entrance(t: f32, index: usize) -> f32 {
  let progress = ((t - index as f32 * 0.12) / 0.45).clamp(0., 1.);
  1. - (1. - progress).powi(3)
}

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let playback = window.use_keyed_state("animation-playback", cx, |_, _| Playback::default());
  let state = playback.read(cx);
  let (generation, slow, looping) = (state.generation, state.slow, state.looping);
  let duration = Duration::from_millis(if slow { 4800 } else { 2400 });
  // Repeating animations use their start state under reduced motion. Use a
  // one-shot instead so entrance cards remain visible in their final state.
  let timeline = if looping && !cx.reduce_motion() {
    Animation::new(duration).repeat()
  } else {
    Animation::new(duration)
  };
  let primary = cx.theme().primary;
  let muted = cx.theme().muted;
  let border = cx.theme().border;
  let foreground = cx.theme().foreground;
  let secondary = cx.theme().muted_foreground;
  let replay = playback.clone();
  let speed = playback.clone();

  div()
    .flex()
    .flex_col()
    .gap_4()
    .w_full()
    .child(
      div()
        .flex()
        .flex_wrap()
        .gap_2()
        .child(
          Button::new("element-animation-replay")
            .label("重播全部")
            .on_click(move |_, _, cx| {
              replay.update(cx, |state, cx| {
                state.generation = state.generation.wrapping_add(1);
                cx.notify();
              })
            }),
        )
        .child(
          Button::new("element-animation-speed")
            .label(if slow { "速度 ×0.5" } else { "速度 ×1" })
            .selected(slow)
            .on_click(move |_, _, cx| {
              speed.update(cx, |state, cx| {
                state.slow = !state.slow;
                state.generation = state.generation.wrapping_add(1);
                cx.notify();
              })
            }),
        )
        .child(
          Button::new("element-animation-loop")
            .label(if looping {
              "停止循环"
            } else {
              "循环播放"
            })
            .selected(looping)
            .on_click(move |_, _, cx| {
              playback.update(cx, |state, cx| {
                state.looping = !state.looping;
                state.generation = state.generation.wrapping_add(1);
                cx.notify();
              })
            }),
        ),
    )
    .child(
      div()
        .text_sm()
        .text_color(secondary)
        .child("默认播放一次；慢速观察细节，循环可随时关闭。动效遵循系统减少动态效果设置。"),
    )
    .child(div().text_lg().child("01 · 错峰入场"))
    .child(
      div().flex().flex_wrap().gap_3().children(
        ["准备就绪", "连接服务", "同步完成"]
          .into_iter()
          .enumerate()
          .map(|(index, label)| {
            // Fixed slots isolate the animated offset from neighboring layout.
            div().relative().w(px(156.)).h(px(92.)).child(
              div()
                .absolute()
                .left_0()
                .top_0()
                .w_full()
                .h(px(76.))
                .p_3()
                .rounded_lg()
                .bg(muted)
                .border_1()
                .border_color(border)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                  div()
                    .text_sm()
                    .text_color(primary)
                    .child(format!("0{}", index + 1)),
                )
                .child(label)
                .with_animation(
                  (SharedString::from(format!("entrance-{index}")), generation),
                  timeline.clone(),
                  move |el, t| {
                    let progress = entrance(t, index);
                    el.top(px(16. * (1. - progress))).opacity(progress)
                  },
                ),
            )
          }),
      ),
    )
    .child(div().text_lg().child("02 · 同程缓动对比"))
    .child(
      div().flex().flex_col().gap_2().children(
        ["匀速", "先快后慢", "先慢后快"]
          .into_iter()
          .enumerate()
          .map(|(index, label)| {
            div()
              .flex()
              .items_center()
              .gap_3()
              .child(div().w(px(90.)).flex_shrink_0().text_sm().child(label))
              .child(
                div()
                  .relative()
                  .flex_1()
                  .h(px(32.))
                  .rounded_md()
                  .bg(muted)
                  .overflow_hidden()
                  .child(
                    div()
                      .absolute()
                      .top(px(6.))
                      .size(px(20.))
                      .rounded_full()
                      .bg(primary)
                      .with_animation(
                        (SharedString::from(format!("easing-{index}")), generation),
                        timeline.clone(),
                        move |el, t| {
                          // Out and back: both endpoints match for a seamless repeat.
                          let phase = 1. - (2. * t - 1.).abs();
                          let progress = match index {
                            1 => 1. - (1. - phase).powi(3),
                            2 => phase.powi(3),
                            _ => phase,
                          };
                          el.left(relative(0.85 * progress))
                        },
                      ),
                  ),
              )
          }),
      ),
    )
    .child(div().text_lg().child("03 · 轨道与节奏"))
    .child(
      div()
        .flex()
        .flex_wrap()
        .gap_4()
        .child(
          div()
            .relative()
            .size(px(144.))
            .rounded_lg()
            .bg(muted)
            .child(
              div()
                .absolute()
                .left(px(26.))
                .top(px(26.))
                .size(px(92.))
                .rounded_full()
                .border_1()
                .border_color(border),
            )
            .child(
              div()
                .absolute()
                .left(px(58.))
                .top(px(58.))
                .size(px(28.))
                .rounded_full()
                .bg(primary)
                .with_animation(
                  ("motion-heartbeat", generation),
                  timeline.clone(),
                  |el, t| {
                    let pulse = (t * TAU * 2.).cos() * 0.5 + 0.5;
                    el.opacity(0.45 + 0.55 * pulse)
                  },
                ),
            )
            .children((0 .. 3).map(|index| {
              div()
                .absolute()
                .size(px(12.))
                .rounded_full()
                .bg(primary)
                .with_animation(
                  (SharedString::from(format!("orbit-{index}")), generation),
                  timeline.clone(),
                  move |el, t| {
                    let angle = TAU * (t + index as f32 / 3.);
                    el.left(px(66. + angle.cos() * 46.))
                      .top(px(66. + angle.sin() * 46.))
                      .opacity(1. - index as f32 * 0.25)
                  },
                )
            })),
        )
        .child(
          div()
            .flex()
            .flex_col()
            .justify_center()
            .gap_3()
            .child(
              div()
                .text_sm()
                .text_color(secondary)
                .child("相位错开的跳跃 · 柔和脉冲"),
            )
            .child(div().flex().gap_3().children((0 .. 5).map(|index| {
              div().relative().w(px(20.)).h(px(52.)).child(
                div()
                  .absolute()
                  .size(px(14.))
                  .rounded_full()
                  .bg(primary)
                  .with_animation(
                    (SharedString::from(format!("rhythm-{index}")), generation),
                    timeline.clone(),
                    move |el, t| {
                      // The envelope settles all dots at rest for a one-shot animation.
                      let envelope = (std::f32::consts::PI * t).sin().max(0.);
                      let bounce = (TAU * (t * 2. - index as f32 * 0.12)).sin().max(0.);
                      el.top(px(28. - 24. * envelope * bounce))
                        .opacity(0.5 + 0.5 * bounce * envelope)
                    },
                  ),
              )
            })))
            .child(
              div()
                .text_sm()
                .text_color(foreground)
                .child("位移、透明度与节奏组合"),
            ),
        ),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
  use gpui_kit::{
    AppContext, Context, IntoElement, Render, TestAppContext, Window, component::Root, px, size,
    test::TestWindowExt,
  };

  use super::entrance;

  struct Preview;

  impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
      super::render(window, cx)
    }
  }

  #[gpui_kit::test]
  fn playback_controls_work_with_reduced_motion(cx: &mut TestAppContext) {
    cx.update(|cx| {
      gpui_kit::init(cx);
      cx.set_reduce_motion(true);
    });
    let view = cx.new(|_| Preview);
    let window = cx.open_window(size(px(1000.), px(1000.)), |window, cx| {
      Root::new(view, window, cx)
    });
    cx.update_window(window.into(), |_, window, cx| {
      window.render_frame(cx);
      window.click("element-animation-replay", cx);
      window.click("element-animation-speed", cx);
      assert_eq!(
        window.find("element-animation-speed").label(),
        Some("速度 ×0.5")
      );
      window.click("element-animation-loop", cx);
      assert_eq!(
        window.find("element-animation-loop").label(),
        Some("停止循环")
      );
      window.click("element-animation-loop", cx);
      assert_eq!(
        window.find("element-animation-loop").label(),
        Some("循环播放")
      );
      window.click("element-animation-speed", cx);
      assert_eq!(
        window.find("element-animation-speed").label(),
        Some("速度 ×1")
      );
    })
    .unwrap();
  }

  #[test]
  fn staggered_entrances_finish_and_remain_bounded() {
    for index in 0 .. 3 {
      assert_eq!(entrance(0., index), 0.);
      assert_eq!(entrance(1., index), 1.);
      let mut previous = 0.;
      for frame in 0 ..= 100 {
        let value = entrance(frame as f32 / 100., index);
        assert!((previous ..= 1.).contains(&value));
        previous = value;
      }
    }
    assert!(entrance(0.2, 0) > entrance(0.2, 1));
    assert_eq!(entrance(0.2, 2), 0.);
  }
}

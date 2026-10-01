use gpui_kit::{
  component::{ActiveTheme, button::Button},
  *,
};

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let expanded = window.use_keyed_state("spring-expanded", cx, |_, _| false);
  let active = *expanded.read(cx);
  let target = if active { 1.0_f32 } else { 0.0_f32 };
  let primary = cx.theme().primary;
  let muted = cx.theme().muted;
  let secondary = cx.theme().muted_foreground;
  div()
    .flex()
    .flex_col()
    .gap_4()
    .w_full()
    .child(
      Button::new("element-spring-toggle")
        .label(if active {
          "收回 · 再次点击可中途反向"
        } else {
          "展开 · 比较三种弹簧"
        })
        .on_click(move |_, _, cx| {
          expanded.update(cx, |value, cx| {
            *value = !*value;
            cx.notify();
          })
        }),
    )
    .child(
      div()
        .text_sm()
        .text_color(secondary)
        .child("三条轨道同时改变目标；运动中反向会保留速度。轨道上的竖线标记起点和终点。"),
    )
    .children(
      [
        (
          "轻快回弹",
          "低阻尼 · 明显越过终点",
          SpringConfig::new(170., 10., 1.),
        ),
        (
          "平稳到达",
          "临界阻尼 · 快速稳定",
          SpringConfig::new(170., 2. * 170_f32.sqrt(), 1.),
        ),
        (
          "缓慢跟随",
          "高阻尼 · 柔和靠近",
          SpringConfig::new(170., 40., 1.),
        ),
      ]
      .into_iter()
      .enumerate()
      .map(|(index, (label, hint, config))| {
        div()
          .flex()
          .flex_col()
          .gap_2()
          .child(
            div()
              .flex()
              .flex_wrap()
              .gap_3()
              .child(label)
              .child(div().text_sm().text_color(secondary).child(hint)),
          )
          .child(
            div()
              .relative()
              .w_full()
              .h(px(52.))
              .rounded_lg()
              .bg(muted)
              .overflow_hidden()
              .children([0.15, 0.7].into_iter().map(|position| {
                div()
                  .absolute()
                  .left(relative(position))
                  .top(px(6.))
                  .h(px(40.))
                  .w(px(2.))
                  .bg(secondary)
                  .opacity(0.3)
              }))
              .child(
                div()
                  .absolute()
                  .top(px(12.))
                  .size(px(28.))
                  .rounded_full()
                  .bg(primary)
                  .with_spring(
                    ("spring-comparison", index),
                    SpringAnimation::new(config).to(target),
                    |el, value| el.left(relative(0.15 + 0.55 * value)),
                  ),
              ),
          )
      }),
    )
    .child(div().text_lg().child("组合应用 · 弹性展开卡片"))
    .child(
      div()
        .overflow_hidden()
        .rounded_lg()
        .bg(muted)
        .w_full()
        .child(
          div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child("项目概览")
            .child(
              div()
                .text_sm()
                .text_color(secondary)
                .child("高度、内容透明度与进度同步过渡"),
            )
            .child(div().h(px(8.)).rounded_full().bg(primary).with_spring(
              "spring-card-progress",
              SpringAnimation::new(SpringConfig::new(170., 22., 1.)).to(target),
              |el, value| el.w(relative(0.15 + 0.7 * value.clamp(0., 1.))),
            ))
            .child(
              div()
                .text_sm()
                .child("所有更改已同步，可以开始下一步。")
                .with_spring(
                  "spring-card-opacity",
                  SpringAnimation::new(SpringConfig::new(170., 26., 1.)).to(target),
                  |el, value| el.opacity(value.clamp(0., 1.)),
                ),
            ),
        )
        .with_spring(
          "element-spring",
          SpringAnimation::new(SpringConfig::new(170., 16., 1.)).to(target),
          |el, value| el.h(px(54. + 114. * value)),
        ),
    )
    .into_any_element()
}

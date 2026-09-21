{
  let primary = cx.theme().primary;
  let border = cx.theme().border;
  div()
    .id("div-extension-layers")
    .test_support()
    .flex()
    .flex_wrap()
    .gap_3()
    .children(
      [
        ("100% 不透明", 1.0),
        ("65% 不透明", 0.65),
        ("35% 不透明", 0.35),
      ]
      .into_iter()
      .map(|(label, opacity)| {
        div()
          .flex()
          .flex_col()
          .gap_2()
          .p_3()
          .border_1()
          .border_color(border)
          .rounded_lg()
          .shadow_md()
          .child(
            div()
              .w(px(72.))
              .h(px(40.))
              .rounded_md()
              .bg(primary)
              .opacity(opacity),
          )
          .child(label)
      }),
    )
}

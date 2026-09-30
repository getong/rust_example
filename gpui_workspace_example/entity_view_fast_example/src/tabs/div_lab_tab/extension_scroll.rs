{
  let muted = cx.theme().muted;
  let border = cx.theme().border;
  div()
    .id("div-extension-list")
    .test_support()
    .h(px(120.))
    .w_full()
    .overflow_y_scroll()
    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
    .rounded_lg()
    .border_1()
    .border_color(border)
    .children((1 ..= 12).map(|index| {
      div()
        .h(px(36.))
        .px_3()
        .flex()
        .items_center()
        .bg(muted)
        .border_b_1()
        .border_color(border)
        .child(format!("第 {index:02} 行 · 12 行内容放在 120px 高的视口中"))
    }))
}

{
  let primary = cx.theme().primary;
  let foreground = cx.theme().primary_foreground;
  let muted = cx.theme().muted;
  div()
    .id("div-extension-layout")
    .test_support()
    .flex()
    .flex_col()
    .gap_3()
    .w_full()
    .child("外层 flex_col 纵向排列，内层 flex + flex_wrap 横向排列并自动换行")
    .child(
      div()
        .flex()
        .flex_wrap()
        .gap_3()
        .p_3()
        .rounded_lg()
        .bg(muted)
        .children(["导航", "内容", "操作"].into_iter().map(|label| {
          div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(120.))
            .h(px(56.))
            .rounded_md()
            .bg(primary)
            .text_color(foreground)
            .child(label)
        })),
    )
    .child("gap_3 统一设置子元素间距；缩小窗口可观察换行")
}

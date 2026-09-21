div()
  .flex()
  .flex_col()
  .gap_3()
  .p_4()
  .rounded_lg()
  .border_1()
  .border_color(cx.theme().border)
  .child(
    div()
      .flex()
      .flex_wrap()
      .gap_3()
      .items_center()
      .justify_between()
      .child("由 div 组合的任务卡片")
      .child(
        div()
          .px_3()
          .py_1()
          .rounded_full()
          .bg(muted)
          .child(if self.enabled {
            "运行中"
          } else {
            "待开始"
          }),
      ),
  )
  .child(format!(
    "进度 {}% · 来自按钮的点击状态",
    self.clicks % 11 * 10
  ))
  .child(
    div()
      .h(px(12.))
      .w_full()
      .rounded_full()
      .overflow_hidden()
      .bg(muted)
      .child(
        div()
          .h_full()
          .w(relative((self.clicks % 11) as f32 / 10.))
          .bg(primary),
      ),
  )

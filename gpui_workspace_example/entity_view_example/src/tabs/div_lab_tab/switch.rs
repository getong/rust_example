div()
  .flex()
  .items_center()
  .gap_3()
  .child(
    div()
      .id("div-switch")
      .test_support()
      .cursor_pointer()
      .relative()
      .w(px(56.))
      .h(px(32.))
      .rounded_full()
      .bg(if self.enabled {
        primary
      } else {
        cx.theme().border
      })
      .on_click(cx.listener(|this, _, _, cx| {
        this.enabled = !this.enabled;
        cx.notify();
      }))
      .child(
        div()
          .absolute()
          .top(px(4.))
          .left(px(if self.enabled { 28. } else { 4. }))
          .size(px(24.))
          .rounded_full()
          .bg(rgb(0xffffff)),
      ),
  )
  .child(if self.enabled {
    "已开启"
  } else {
    "已关闭"
  })

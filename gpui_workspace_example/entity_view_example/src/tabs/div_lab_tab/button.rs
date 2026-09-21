div()
  .id("div-counter")
  .test_support()
  .cursor_pointer()
  .px_4()
  .py_3()
  .rounded_lg()
  .bg(primary)
  .text_color(foreground)
  .hover(|style| style.opacity(0.85))
  .active(|style| style.opacity(0.65))
  .on_click(cx.listener(|this, _, _, cx| {
    this.clicks += 1;
    cx.notify();
  }))
  .child(format!("点击这个 div · 已点击 {} 次", self.clicks))

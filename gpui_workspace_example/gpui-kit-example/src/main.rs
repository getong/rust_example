use gpui_kit::{
  component::{button::*, *},
  *,
};

pub struct HelloWorld {
  counter: Entity<Counter>,
}
impl Render for HelloWorld {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .v_flex()
      .gap_2()
      .size_full()
      .items_center()
      .justify_center()
      .child("Hello, World!")
      .child(
        Button::new("ok")
          .primary()
          .label("Let's Go!")
          .on_click(|_, _, _| println!("Clicked!")),
      )
      .child(self.counter.clone())
  }
}

struct Counter {
  count: i32,
}

impl Render for Counter {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .gap_3()
      .p_4()
      .bg(rgb(0x131315))
      .child(format!("count:{}", self.count))
      .child(
        Button::new("increment")
          .label("+1")
          .on_click(cx.listener(|view, _, _, cx| {
            view.count += 1;

            cx.notify(); //标记需要重绘
          })),
      )
  }
}

fn main() {
  gpui_kit::application().run(move |cx| {
    // This must be called before using any GPUI Component features.
    gpui_kit::init(cx);

    cx.spawn(async move |cx| {
      cx.open_window(WindowOptions::default(), |window, cx| {
        let counter = cx.new(|_| Counter { count: 0 });
        let view = cx.new(|_| HelloWorld { counter });
        // This first level on the window, should be a Root.
        cx.new(|cx| Root::new(view, window, cx))
      })
      .expect("Failed to open window");
    })
    .detach();
  });
}

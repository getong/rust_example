//! 事件订阅：子组件发出业务事件，父组件只处理事件载荷。
use gpui_kit::{component::button::*, *};

struct CountUpdated(u32);

#[derive(Default)]
struct Counter {
  count: u32,
}

impl EventEmitter<CountUpdated> for Counter {}

impl Render for Counter {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    Button::new("increment")
      .label(format!("Child: {} (+1)", self.count))
      .on_click(cx.listener(|this, _, _, cx| {
        this.count += 1;
        cx.notify();
        // notify 请求重绘；emit 才会触发业务事件订阅。
        cx.emit(CountUpdated(this.count));
      }))
  }
}

pub(crate) struct EventDemo {
  counter: Entity<Counter>,
  received_count: u32,
  // 随父组件释放订阅，避免在 render 中重复注册。
  _subscription: Subscription,
}

impl EventDemo {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    let counter = cx.new(|_| Counter::default());
    let subscription = cx.subscribe(&counter, |this, _, event: &CountUpdated, cx| {
      this.received_count = event.0;
      cx.notify();
    });
    Self {
      counter,
      received_count: 0,
      _subscription: subscription,
    }
  }
}

impl Render for EventDemo {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .items_center()
      .gap_3()
      .child(self.counter.clone())
      .child(format!("Parent received: {}", self.received_count))
  }
}

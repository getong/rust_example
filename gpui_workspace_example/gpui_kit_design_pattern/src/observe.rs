//! 状态观察：源实体 notify 后，派生视图重新读取源数据，无业务事件。
use gpui_kit::{component::button::*, *};

#[derive(Default)]
struct Counter {
  count: u32,
}
impl Render for Counter {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    Button::new("observed-increment")
      .label(format!("Source: {} (+1)", self.count))
      .on_click(cx.listener(|this, _, _, cx| {
        this.count += 1;
        cx.notify();
      }))
  }
}

struct DoubledCount {
  source: Entity<Counter>,
  _subscription: Subscription,
}
impl DoubledCount {
  fn new(source: Entity<Counter>, cx: &mut Context<Self>) -> Self {
    let subscription = cx.observe(&source, |_, _, cx| cx.notify());
    Self {
      source,
      _subscription: subscription,
    }
  }
}
impl Render for DoubledCount {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div().child(format!(
      "Observed double: {}",
      u64::from(self.source.read(cx).count) * 2
    ))
  }
}

pub(crate) struct ObserveDemo {
  source: Entity<Counter>,
  mirror: Entity<DoubledCount>,
}
impl ObserveDemo {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    let source = cx.new(|_| Counter::default());
    let mirror = cx.new(|cx| DoubledCount::new(source.clone(), cx));
    Self { source, mirror }
  }
}
impl Render for ObserveDemo {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .items_center()
      .gap_3()
      .child(self.source.clone())
      .child(self.mirror.clone())
  }
}

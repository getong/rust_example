mod state;
#[cfg(test)]
mod tests;

use gpui_kit::{
  component::{
    Root,
    button::{Button, ButtonVariants},
  },
  *,
};
use state::{AppSettings, CounterId, CounterState};

struct CounterPanel {
  id: CounterId,
  model: Entity<CounterState>,
  // Subscription 必须保存；丢弃即取消观察，随视图一起释放。
  _subscriptions: Vec<Subscription>,
}
impl CounterPanel {
  fn new(id: CounterId, model: Entity<CounterState>, cx: &mut Context<Self>) -> Self {
    let subscriptions = vec![
      // Model 的 notify 不会自动通知任意持有句柄的视图，显式建立观察关系。
      cx.observe(&model, |_, _, cx| cx.notify()),
      cx.observe_global::<AppSettings>(|_, cx| cx.notify()),
    ];
    Self {
      id,
      model,
      _subscriptions: subscriptions,
    }
  }
}
impl Render for CounterPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // read 是只读借用；clone Entity 只复制强句柄，不复制 Model。
    let count = self.model.read(cx).count(self.id);
    let step = cx.global::<AppSettings>().step();
    div()
      .flex()
      .flex_col()
      .items_center()
      .gap_3()
      .p_4()
      .child(
        div()
          .text_xl()
          .child(format!("Counter {}: {}", self.id.name(), count)),
      )
      .child(
        Button::new("increment")
          .primary()
          .label(format!("+{step}"))
          .on_click(cx.listener(|view, _, _, cx| {
            // 外层 cx 属于 CounterPanel，update 闭包的 cx 属于 CounterState。
            // update 同步修改 Model；notify 的观察回调由 GPUI 随后调度。
            view
              .model
              .update(cx, |model, cx| model.increment(view.id, cx));
          })),
      )
  }
}

struct SummaryPanel {
  model: Entity<CounterState>,
  _subscription: Subscription,
}

impl SummaryPanel {
  fn new(model: Entity<CounterState>, cx: &mut Context<Self>) -> Self {
    let subscription = cx.observe(&model, |_, _, cx| cx.notify());
    Self {
      model,
      _subscription: subscription,
    }
  }
}

impl Render for SummaryPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let model = self.model.read(cx);
    div()
      .flex()
      .flex_col()
      .items_center()
      .gap_2()
      // 汇总直接从 Model 派生，不维护第二份状态。
      .child(div().text_xl().child(format!("Total: {}", model.total())))
      .child(format!(
        "A = {}, B = {}",
        model.count(CounterId::A),
        model.count(CounterId::B)
      ))
      .child(model.last_change().to_owned())
      .child(
        Button::new("reset-all")
          .label("Reset both counters")
          .on_click(cx.listener(|view, _, _, cx| {
            view.model.update(cx, CounterState::reset);
          })),
      )
  }
}

struct CounterApp {
  counters: [Entity<CounterPanel>; 2],
  summary: Entity<SummaryPanel>,
  _settings_subscription: Subscription,
}

impl CounterApp {
  fn new(cx: &mut Context<Self>) -> Self {
    // 当前版本使用 cx.new；旧教程中的 new_model / new_view 已统一为此 API。
    let model = cx.new(|_| CounterState::default());
    let counters = [
      cx.new(|cx| CounterPanel::new(CounterId::A, model.clone(), cx)),
      cx.new(|cx| CounterPanel::new(CounterId::B, model.clone(), cx)),
    ];
    let summary = cx.new(|cx| SummaryPanel::new(model, cx));
    let subscription = cx.observe_global::<AppSettings>(|_, cx| cx.notify());
    Self {
      counters,
      summary,
      _settings_subscription: subscription,
    }
  }
}

impl Render for CounterApp {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let step = cx.global::<AppSettings>().step();
    div()
      .flex()
      .flex_col()
      .items_center()
      .justify_center()
      .gap_4()
      .size_full()
      .bg(rgb(0x1e1e1e))
      .text_color(rgb(0xffffff))
      .child(div().text_2xl().child("Shared counters"))
      .child(format!("Increment step: {step}"))
      .child(
        Button::new("toggle-step")
          .label("Switch step (1 / 5)")
          .on_click(|_, _, cx| {
            // Global 更新通知 observe_global 订阅者；不必手动 emit。
            // 只影响步长显示和后续递增，不改已有计数。
            cx.update_global::<AppSettings, _>(|settings, _| settings.toggle_step());
          }),
      )
      .child(
        div()
          .flex()
          .gap_4()
          .child(self.counters[0].clone())
          .child(self.counters[1].clone()),
      )
      .child(self.summary.clone())
  }
}

fn main() {
  // 启动 GPUI 应用程序
  gpui_kit::application().run(|cx| {
    // 使用组件前，先初始化 GPUI Kit。
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());

    cx.spawn(async move |cx| {
      if let Err(error) = cx.open_window(WindowOptions::default(), |window, cx| {
        // cx.new 同时用于创建 Model Entity 和实现了 Render 的 View Entity。
        let view = cx.new(CounterApp::new);
        cx.new(|cx| Root::new(view, window, cx))
      }) {
        eprintln!("Failed to open counter window: {error}");
        cx.update(|cx| cx.quit());
      }
    })
    .detach();
  });
}

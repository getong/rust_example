mod state;
#[cfg(test)]
mod tests;

use gpui_kit::{
  base::Disableable,
  component::{
    Root,
    button::{Button, ButtonVariants},
    tab::{Tab, TabBar},
  },
  *,
};
use state::{AppSettings, CounterId, CounterState};

// Global 持有应用级模型；关闭标签不会丢失计数。内部模型仍通过 observe 订阅。
struct AppServices {
  counters: Entity<CounterState>,
}
impl Global for AppServices {}

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
        Button::new(match self.id {
          CounterId::A => "increment-a",
          CounterId::B => "increment-b",
        })
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

struct CounterTab {
  tab_number: usize,
  local_clicks: usize,
  counters: [Entity<CounterPanel>; 2],
  summary: Entity<SummaryPanel>,
  _settings_subscription: Subscription,
}

impl CounterTab {
  fn new(tab_number: usize, model: Entity<CounterState>, cx: &mut Context<Self>) -> Self {
    // 每个标签独立创建视图，只共享传入的 Model Entity。
    let counters = [
      cx.new(|cx| CounterPanel::new(CounterId::A, model.clone(), cx)),
      cx.new(|cx| CounterPanel::new(CounterId::B, model.clone(), cx)),
    ];
    let summary = cx.new(|cx| SummaryPanel::new(model, cx));
    let subscription = cx.observe_global::<AppSettings>(|_, cx| cx.notify());
    Self {
      tab_number,
      local_clicks: 0,
      counters,
      summary,
      _settings_subscription: subscription,
    }
  }
}

impl Render for CounterTab {
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
      .child(
        div()
          .text_2xl()
          .child(format!("Shared counters - Tab {}", self.tab_number)),
      )
      .child("Change a counter, then switch tabs to see the shared values.")
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
      .child(
        Button::new("local-click")
          .label(format!("Only this tab: {}", self.local_clicks))
          .on_click(cx.listener(|view, _, _, cx| {
            view.local_clicks += 1;
            cx.notify();
          })),
      )
      .child("The button above is local; counters and step are shared.")
  }
}

// 同一个 panel 持有所有标签实体，切换只改变 active_tab，不重建标签。
struct TabbedPanel {
  model: Entity<CounterState>,
  tabs: Vec<Entity<CounterTab>>,
  active_tab: usize,
  next_tab: usize,
}

impl TabbedPanel {
  fn new(model: Entity<CounterState>, cx: &mut Context<Self>) -> Self {
    let tabs = (1 ..= 2)
      .map(|number| cx.new(|cx| CounterTab::new(number, model.clone(), cx)))
      .collect();
    Self {
      model,
      tabs,
      active_tab: 0,
      next_tab: 3,
    }
  }

  fn add_tab(&mut self, cx: &mut Context<Self>) {
    let number = self.next_tab;
    self.next_tab += 1;
    self
      .tabs
      .push(cx.new(|cx| CounterTab::new(number, self.model.clone(), cx)));
    self.active_tab = self.tabs.len() - 1;
    cx.notify();
  }

  fn close_active_tab(&mut self, cx: &mut Context<Self>) {
    if self.tabs.is_empty() {
      return;
    }
    // 释放此标签的 Entity 和 Subscription；共享模型由 panel / Global 保活。
    self.tabs.remove(self.active_tab);
    self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
    cx.notify();
  }
}

impl Render for TabbedPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .flex_col()
      .size_full()
      .bg(rgb(0x1e1e1e))
      .text_color(rgb(0xffffff))
      .child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .p_2()
          .child(
            Button::new("new-tab")
              .label("New tab")
              .on_click(cx.listener(|panel, _, _, cx| panel.add_tab(cx))),
          )
          .child(
            Button::new("close-tab")
              .label("Close current tab")
              .disabled(self.tabs.is_empty())
              .on_click(cx.listener(|panel, _, _, cx| panel.close_active_tab(cx))),
          ),
      )
      .child(
        TabBar::new("counter-tabs")
          .w_full()
          .selected_index(self.active_tab)
          .on_click(cx.listener(|panel, index: &usize, _, cx| {
            if *index < panel.tabs.len() {
              panel.active_tab = *index;
              cx.notify();
            }
          }))
          .children(
            self
              .tabs
              .iter()
              .map(|tab| Tab::new().label(format!("Tab {}", tab.read(cx).tab_number))),
          ),
      )
      .child(
        div()
          .flex_1()
          .min_h_0()
          .child(match self.tabs.get(self.active_tab) {
            Some(tab) => tab.clone().into_any_element(),
            None => div()
              .p_4()
              .child("No tabs. Click New tab to resume the shared counters.")
              .into_any_element(),
          }),
      )
  }
}

fn main() {
  gpui_kit::application().run(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
    let counters = cx.new(|_| CounterState::default());
    cx.set_global(AppServices { counters });
    cx.spawn(async move |cx| {
      let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
          origin: point(px(60.), px(60.)),
          size: size(px(760.), px(700.)),
        })),
        window_min_size: Some(size(px(640.), px(640.))),
        ..Default::default()
      };
      if let Err(error) = cx.open_window(options, |window, cx| {
        window.set_window_title("Shared counters - Tab panel");
        let model = cx.global::<AppServices>().counters.clone();
        let panel = cx.new(|cx| TabbedPanel::new(model, cx));
        cx.new(|cx| Root::new(panel, window, cx))
      }) {
        eprintln!("Failed to open tab demo: {error}");
        cx.update(|cx| cx.quit());
      }
    })
    .detach();
  });
}

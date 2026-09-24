// Turso's nested async query/transaction futures require deeper Send evaluation.
#![recursion_limit = "256"]

mod baidu_cache;
mod baidu_tab;
mod component_tab;
mod tabs;
rust_i18n::i18n!("locales/component_gallery", fallback = "en");
mod palette;
mod panel_tab;
mod raised_button;
mod scroll_panel;
mod scrollbar_tab;
mod state;
mod tab_directory;
mod tabbed_panel;
use tabbed_panel::TabbedPanel;
#[cfg(test)]
mod tests;
mod toast_tab;

use gpui_kit::{
  component::{
    Root,
    button::Button,
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    label::Label,
    v_flex,
  },
  *,
};
use palette::AppPalette;
use raised_button::RaisedButton;
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
    GroupBox::new()
      .outline()
      .title(format!("Counter {}", self.id.name()))
      .title_style(StyleRefinement::default().text_color(AppPalette::default().foreground))
      .content_style(
        StyleRefinement::default()
          .items_center()
          .gap_3()
          .bg(AppPalette::default().background)
          .text_color(AppPalette::default().foreground),
      )
      .child(
        Label::new(format!("Counter {}: {}", self.id.name(), count))
          .text_xl()
          .text_color(AppPalette::default().foreground),
      )
      .child(
        RaisedButton::new(match self.id {
          CounterId::A => "increment-a",
          CounterId::B => "increment-b",
        })
        .label(format!("+{step}"))
        .on_click(cx.listener(|view: &mut Self, _, _, cx| {
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
    GroupBox::new()
      .fill()
      .title("Summary")
      .title_style(StyleRefinement::default().text_color(AppPalette::default().foreground))
      .content_style(
        StyleRefinement::default()
          .items_center()
          .gap_2()
          .bg(AppPalette::default().background)
          .text_color(AppPalette::default().foreground),
      )
      // 汇总直接从 Model 派生，不维护第二份状态。
      .child(
        Label::new(format!("Total: {}", model.total()))
          .text_xl()
          .text_color(AppPalette::default().foreground),
      )
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
    v_flex()
      .items_center()
      .justify_center()
      .gap_4()
      .size_full()
      .bg(AppPalette::default().background)
      .text_color(AppPalette::default().foreground)
      .child(
        Label::new(format!("Shared counters - Tab {}", self.tab_number))
          .text_2xl()
          .text_color(AppPalette::default().foreground),
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
        h_flex()
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

fn main() {
  gpui_kit::application()
    .with_assets(gpui_kit::assets::Assets)
    .run(|cx| {
      gpui_kit::init(cx);
      crate::tabs::init(cx);
      crate::tabs::init_http(cx);
      // 关闭最后一个窗口时退出应用，避免留下无窗口的后台进程。
      cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
          cx.quit();
        }
      })
      .detach();
      cx.set_global(AppSettings::default());
      let counters = cx.new(|_| CounterState::default());
      cx.set_global(AppServices { counters });
      cx.activate(true);
      cx.spawn(async move |cx| {
        let options = WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(60.), px(60.)),
            size: size(px(1200.), px(860.)),
          })),
          window_min_size: Some(size(px(640.), px(640.))),
          ..Default::default()
        };
        if let Err(error) = cx.open_window(options, |window, cx| {
          window.activate_window();
          window.set_window_title("Shared counters - Tab panel");
          let model = cx.global::<AppServices>().counters.clone();
          let panel = cx.new(|cx| TabbedPanel::new(model, cx));
          panel.update(cx, |panel, cx| {
            panel.add_component_gallery(cx);
            panel.open_baidu(cx);
          });
          cx.new(|cx| Root::new(panel, window, cx))
        }) {
          eprintln!("Failed to open tab demo: {error}");
          cx.update(|cx| cx.quit());
        }
      })
      .detach();
    });
}

mod baidu_tab;
mod component_tab;
mod tabs;
rust_i18n::i18n!("locales/component_gallery", fallback = "en");
mod palette;
mod raised_button;
mod router;
mod scroll_panel;
mod scrollbar_tab;
mod state;
mod tab_directory;
#[cfg(test)]
mod tests;
mod toast_tab;

use baidu_tab::BaiduTab;
use component_tab::ComponentTab;
use gpui_kit::{
  base::{Disableable, NavStack},
  component::{
    ActiveTheme, Root,
    button::Button,
    empty::{Empty as EmptyState, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    label::Label,
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    v_flex,
  },
  prelude::FluentBuilder as _,
  *,
};
use palette::AppPalette;
use raised_button::RaisedButton;
use router::TabRouter;
use scrollbar_tab::ScrollbarTab;
use state::{AppSettings, CounterId, CounterState};
use tab_directory::TabDirectory;
use toast_tab::ToastTab;

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

enum PanelTab {
  Baidu(Entity<BaiduTab>),
  Component(Entity<ComponentTab>),
  Counter(Entity<CounterTab>),
  Toast(Entity<ToastTab>),
  Scrollbar(Entity<ScrollbarTab>),
  Directory(Entity<TabDirectory>),
}

impl PanelTab {
  fn path(&self, cx: &App) -> SharedString {
    match self {
      Self::Baidu(_) => "/baidu/top".to_string(),
      Self::Component(tab) => format!("/component/{}", tab.read(cx).slug()),
      Self::Counter(tab) => format!("/counter/{}", tab.read(cx).tab_number),
      Self::Toast(tab) => format!("/toast/{}", tab.entity_id()),
      Self::Scrollbar(tab) => format!("/scrollbar/{}", tab.entity_id()),
      Self::Directory(tab) => format!("/tabs/{}", tab.entity_id()),
    }
    .into()
  }

  fn label(&self, cx: &App) -> String {
    match self {
      Self::Baidu(_) => "百度热榜".into(),
      Self::Component(tab) => tab.read(cx).title().into(),
      Self::Counter(tab) => format!("Tab {}", tab.read(cx).tab_number),
      Self::Toast(_) => "Toast".into(),
      Self::Scrollbar(_) => "Scrollbar".into(),
      Self::Directory(_) => "Tab directory".into(),
    }
  }

  fn view(&self) -> AnyView {
    match self {
      Self::Baidu(tab) => tab.clone().into(),
      Self::Component(tab) => tab.clone().into(),
      Self::Counter(tab) => tab.clone().into(),
      Self::Toast(tab) => tab.clone().into(),
      Self::Scrollbar(tab) => tab.clone().into(),
      Self::Directory(tab) => tab.clone().into(),
    }
  }

  #[cfg(test)]
  fn counter(&self) -> &Entity<CounterTab> {
    match self {
      Self::Counter(tab) => tab,
      Self::Baidu(_)
      | Self::Component(_)
      | Self::Toast(_)
      | Self::Scrollbar(_)
      | Self::Directory(_) => {
        panic!("expected a counter tab")
      }
    }
  }
}

// 路由是选中状态的唯一来源；panel 保留标签 Entity 及其局部状态。
struct TabbedPanel {
  model: Entity<CounterState>,
  tabs: Vec<PanelTab>,
  router: Entity<TabRouter>,
  _router_subscription: Subscription,
  next_tab: usize,
  tab_scroll: ScrollHandle,
}

impl TabbedPanel {
  fn new(model: Entity<CounterState>, cx: &mut Context<Self>) -> Self {
    let mut tabs: Vec<_> = (1 ..= 2)
      .map(|number| PanelTab::Counter(cx.new(|cx| CounterTab::new(number, model.clone(), cx))))
      .collect();
    tabs.push(PanelTab::Toast(cx.new(|_| ToastTab)));
    tabs.push(PanelTab::Scrollbar(cx.new(|_| ScrollbarTab::default())));
    let router = cx.new(TabRouter::new);
    router.update(cx, |router, cx| {
      for pattern in [
        "/counter/{id}",
        "/toast/{id}",
        "/scrollbar/{id}",
        "/tabs/{id}",
        "/component/{id}",
        "/baidu/{id}",
      ] {
        router
          .register_route(pattern)
          .expect("valid tab route templates");
      }
      for tab in &tabs {
        router
          .register(&tab.path(cx), tab.view())
          .expect("tab paths are unique and match their route templates");
      }
      router
        .navigate(&tabs[0].path(cx), cx)
        .expect("initial tab is registered");
    });
    let subscription = cx.observe(&router, |_, _, cx| cx.notify());
    Self {
      model,
      tabs,
      router,
      _router_subscription: subscription,
      next_tab: 3,
      tab_scroll: ScrollHandle::new(),
    }
  }

  fn active_tab(&self, cx: &App) -> Option<usize> {
    let pathname = self.router.read(cx).pathname()?;
    self
      .tabs
      .iter()
      .position(|tab| tab.path(cx).as_ref() == pathname)
  }

  fn select_tab(&self, index: usize, cx: &mut Context<Self>) {
    if let Some(tab) = self.tabs.get(index) {
      self.tab_scroll.scroll_to_item(index);
      let path = tab.path(cx);
      self.router.update(cx, |router, cx| {
        router.navigate(&path, cx).expect("open tab is registered");
      });
      cx.notify();
    }
  }

  fn open_tab(&mut self, tab: PanelTab, cx: &mut Context<Self>) {
    let path = tab.path(cx);
    self.router.update(cx, |router, _| {
      router
        .register(&path, tab.view())
        .expect("new tab has a unique path");
    });
    self.tabs.push(tab);
    self.select_tab(self.tabs.len() - 1, cx);
  }

  fn add_tab(&mut self, cx: &mut Context<Self>) {
    let number = self.next_tab;
    self.next_tab += 1;
    let tab = cx.new(|cx| CounterTab::new(number, self.model.clone(), cx));
    self.open_tab(PanelTab::Counter(tab), cx);
  }

  fn add_toast_tab(&mut self, cx: &mut Context<Self>) {
    let tab = cx.new(|_| ToastTab);
    self.open_tab(PanelTab::Toast(tab), cx);
  }

  fn add_scrollbar_tab(&mut self, cx: &mut Context<Self>) {
    let tab = cx.new(|_| ScrollbarTab::default());
    self.open_tab(PanelTab::Scrollbar(tab), cx);
  }

  fn open_directory(&mut self, cx: &mut Context<Self>) {
    if let Some(index) = self
      .tabs
      .iter()
      .position(|tab| matches!(tab, PanelTab::Directory(_)))
    {
      self.select_tab(index, cx);
      return;
    }
    let panel = cx.entity();
    let tab = cx.new(|cx| TabDirectory::new(&panel, cx));
    self.open_tab(PanelTab::Directory(tab), cx);
  }

  fn open_baidu(&mut self, cx: &mut Context<Self>) {
    if let Some(index) = self
      .tabs
      .iter()
      .position(|tab| matches!(tab, PanelTab::Baidu(_)))
    {
      self.select_tab(index, cx);
      return;
    }
    let tab = cx.new(BaiduTab::new);
    self.open_tab(PanelTab::Baidu(tab), cx);
  }

  fn open_component(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
    if let Some(position) = self
      .tabs
      .iter()
      .position(|tab| matches!(tab, PanelTab::Component(view) if view.read(cx).index == index))
    {
      self.select_tab(position, cx);
      return;
    }
    let panel = cx.entity().downgrade();
    let view = cx.new(|_| ComponentTab::new(index, panel));
    self.open_tab(PanelTab::Component(view), cx);
  }

  fn add_component_gallery(&mut self, cx: &mut Context<Self>) {
    self.open_component(None, cx);
    for index in 0 .. crate::tabs::catalog::DEMOS.len() {
      self.open_component(Some(index), cx);
    }
    self.open_component(None, cx);
  }

  fn close_active_tab(&mut self, cx: &mut Context<Self>) {
    let Some(index) = self.active_tab(cx) else {
      return;
    };
    // 释放此标签的 Entity 和 Subscription；共享模型由 panel / Global 保活。
    let path = self.tabs[index].path(cx);
    self
      .router
      .update(cx, |router, cx| router.unregister(&path, cx));
    self.tabs.remove(index);
    if self.tabs.is_empty() {
      cx.notify();
    } else {
      self.select_tab(index.min(self.tabs.len() - 1), cx);
    }
  }
}

impl Render for TabbedPanel {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let notifications = Root::render_notification_layer(window, cx);
    let dialogs = Root::render_dialog_layer(window, cx);
    let sheets = Root::render_sheet_layer(window, cx);
    let active_tab = self.active_tab(cx);
    let router = self.router.read(cx);
    let pathname = router.pathname().unwrap_or("/").to_owned();
    let stack = router.stack().clone();
    v_flex()
      .relative()
      .size_full()
      .bg(AppPalette::default().background)
      .text_color(AppPalette::default().foreground)
      .child(
        h_flex()
          .flex_wrap()
          .flex_shrink_0()
          .items_center()
          .gap_2()
          .p_2()
          .child(
            Button::new("new-tab")
              .label("New tab")
              .on_click(cx.listener(|panel, _, _, cx| panel.add_tab(cx))),
          )
          .child(
            Button::new("new-toast-tab")
              .label("New toast tab")
              .on_click(cx.listener(|panel, _, _, cx| panel.add_toast_tab(cx))),
          )
          .child(
            Button::new("new-scrollbar-tab")
              .label("New scroll tab")
              .on_click(cx.listener(|panel, _, _, cx| panel.add_scrollbar_tab(cx))),
          )
          .child(
            Button::new("open-baidu")
              .label("百度热榜")
              .on_click(cx.listener(|panel, _, _, cx| panel.open_baidu(cx))),
          )
          .child(
            Button::new("open-components")
              .label("Components")
              .on_click(cx.listener(|panel, _, _, cx| panel.open_component(None, cx))),
          )
          .child(
            Button::new("open-tab-directory")
              .label("Tab directory")
              .on_click(cx.listener(|panel, _, _, cx| panel.open_directory(cx))),
          )
          .child(
            Button::new("close-tab")
              .label("Close current tab")
              .disabled(active_tab.is_none())
              .on_click(cx.listener(|panel, _, _, cx| panel.close_active_tab(cx))),
          ),
      )
      .child(
        TabBar::new("counter-tabs")
          .w_full()
          .menu(true)
          .track_scroll(&self.tab_scroll)
          .when_some(active_tab, |bar, index| bar.selected_index(index))
          .on_click(cx.listener(|panel, index: &usize, _, cx| {
            panel.select_tab(*index, cx);
          }))
          .children(self.tabs.iter().map(|tab| Tab::new().label(tab.label(cx)))),
      )
      .child(v_flex().flex_1().min_h_0().child(if active_tab.is_some() {
        NavStack::new(&stack).size_full().into_any_element()
      } else {
        EmptyState::new()
          .header(
            EmptyHeader::new()
              .title(
                EmptyTitle::new()
                  .text_color(AppPalette::default().foreground)
                  .child("No tabs"),
              )
              .description(
                EmptyDescription::new()
                  .text_color(AppPalette::default().foreground)
                  .child("Click New tab to resume the shared counters."),
              ),
          )
          .content(
            EmptyContent::new().child(
              Button::new("empty-new-tab")
                .label("New tab")
                .on_click(cx.listener(|panel, _, _, cx| panel.add_tab(cx))),
            ),
          )
          .into_any_element()
      }))
      .child(
        StatusBar::new()
          .bg(AppPalette::default().background)
          .text_color(AppPalette::default().foreground)
          .left(format!("Route: {pathname}"))
          .right(format!("id: {}", router.param("id").unwrap_or("—"))),
      )
      .children(sheets)
      .children(dialogs)
      // 通知背景由组件主题决定，文字不能继承深色应用画布的白色。
      .child(
        v_flex()
          .absolute()
          .inset_0()
          .text_color(cx.theme().popover_foreground)
          .children(notifications),
      )
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

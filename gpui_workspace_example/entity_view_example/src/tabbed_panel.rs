use gpui_kit::{
  base::Disableable,
  component::{
    ActiveTheme, Root,
    button::Button,
    empty::{Empty as EmptyState, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle},
    h_flex,
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    v_flex,
  },
  prelude::FluentBuilder as _,
  *,
};
use gpui_router::{RouterState, Routes, use_location, use_navigate};

use crate::{
  CounterTab,
  baidu_tab::BaiduTab,
  component_tab::ComponentTab,
  palette::AppPalette,
  panel_tab::{PanelTab, TabId},
  scrollbar_tab::ScrollbarTab,
  state::CounterState,
  tab_directory::TabDirectory,
  toast_tab::ToastTab,
};

// 路由是选中状态的唯一来源；panel 保留标签 Entity 及其局部状态。
pub(crate) struct TabbedPanel {
  model: Entity<CounterState>,
  pub(crate) tabs: Vec<PanelTab>,
  _router_subscription: Subscription,
  next_tab: usize,
  tab_scroll: ScrollHandle,
}

impl TabbedPanel {
  pub(crate) fn new(model: Entity<CounterState>, cx: &mut Context<Self>) -> Self {
    let mut tabs: Vec<_> = (1 ..= 2)
      .map(|number| {
        PanelTab::new(
          TabId::Counter(number),
          format!("Tab {number}"),
          cx.new(|cx| CounterTab::new(number, model.clone(), cx)),
        )
      })
      .collect();
    let toast = cx.new(|_| ToastTab);
    tabs.push(PanelTab::new(
      TabId::Toast(toast.entity_id()),
      "Toast",
      toast,
    ));
    let scrollbar = cx.new(|_| ScrollbarTab::default());
    tabs.push(PanelTab::new(
      TabId::Scrollbar(scrollbar.entity_id()),
      "Scrollbar",
      scrollbar,
    ));
    if !cx.has_global::<RouterState>() {
      gpui_router::init(cx);
    }
    let initial_path = tabs[0].path();
    use_navigate(cx)(initial_path.clone());
    let mut previous_path = initial_path;
    let subscription = cx.observe_global::<RouterState>(move |panel, cx| {
      let pathname = &use_location(cx).pathname;
      // Routes also updates match metadata during render; only navigation needs a redraw.
      if *pathname != previous_path {
        previous_path = pathname.clone();
        if let Some(index) = panel.active_tab(cx) {
          panel.tab_scroll.scroll_to_item(index);
        }
        cx.notify();
      }
    });
    Self {
      model,
      tabs,
      _router_subscription: subscription,
      next_tab: 3,
      tab_scroll: ScrollHandle::new(),
    }
  }

  pub(crate) fn active_tab(&self, cx: &App) -> Option<usize> {
    let pathname = use_location(cx).pathname.as_ref();
    self
      .tabs
      .iter()
      .position(|tab| tab.path().as_ref() == pathname)
  }

  /// Only open tabs are navigable; rejected paths leave the current page intact.
  pub(crate) fn navigate(&self, path: &str, cx: &mut Context<Self>) -> bool {
    let Some(tab) = self.tabs.iter().find(|tab| tab.path().as_ref() == path) else {
      return false;
    };
    use_navigate(cx)(tab.path());
    true
  }

  pub(crate) fn select_tab(&self, index: usize, cx: &mut Context<Self>) {
    if let Some(tab) = self.tabs.get(index) {
      use_navigate(cx)(tab.path());
    }
  }

  fn select_existing(&self, id: TabId, cx: &mut Context<Self>) -> bool {
    let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
      return false;
    };
    self.select_tab(index, cx);
    true
  }

  fn open_tab(&mut self, tab: PanelTab, cx: &mut Context<Self>) {
    self.tabs.push(tab);
    self.select_tab(self.tabs.len() - 1, cx);
    cx.notify();
  }

  pub(crate) fn add_tab(&mut self, cx: &mut Context<Self>) {
    let number = self.next_tab;
    self.next_tab += 1;
    let tab = cx.new(|cx| CounterTab::new(number, self.model.clone(), cx));
    self.open_tab(
      PanelTab::new(TabId::Counter(number), format!("Tab {number}"), tab),
      cx,
    );
  }

  pub(crate) fn add_toast_tab(&mut self, cx: &mut Context<Self>) {
    let tab = cx.new(|_| ToastTab);
    self.open_tab(
      PanelTab::new(TabId::Toast(tab.entity_id()), "Toast", tab),
      cx,
    );
  }

  pub(crate) fn add_scrollbar_tab(&mut self, cx: &mut Context<Self>) {
    let tab = cx.new(|_| ScrollbarTab::default());
    self.open_tab(
      PanelTab::new(TabId::Scrollbar(tab.entity_id()), "Scrollbar", tab),
      cx,
    );
  }

  pub(crate) fn open_directory(&mut self, cx: &mut Context<Self>) {
    if let Some(index) = self
      .tabs
      .iter()
      .position(|tab| matches!(tab.id, TabId::Directory(_)))
    {
      self.select_tab(index, cx);
      return;
    }
    let panel = cx.entity();
    let tab = cx.new(|cx| TabDirectory::new(&panel, cx));
    self.open_tab(
      PanelTab::new(TabId::Directory(tab.entity_id()), "Tab directory", tab),
      cx,
    );
  }

  pub(crate) fn open_baidu(&mut self, cx: &mut Context<Self>) {
    if self.select_existing(TabId::Baidu, cx) {
      return;
    }
    let tab = cx.new(BaiduTab::new);
    self.open_tab(PanelTab::new(TabId::Baidu, "百度热榜", tab), cx);
  }

  pub(crate) fn open_component(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
    let id = TabId::Component(index.map_or("components", |i| crate::tabs::catalog::DEMOS[i].slug));
    if self.select_existing(id, cx) {
      return;
    }
    let panel = cx.entity().downgrade();
    let view = cx.new(|_| ComponentTab::new(index, panel));
    let title = view.read(cx).title();
    self.open_tab(PanelTab::new(id, title, view), cx);
  }

  pub(crate) fn add_component_gallery(&mut self, cx: &mut Context<Self>) {
    self.open_component(None, cx);
    for index in 0 .. crate::tabs::catalog::DEMOS.len() {
      self.open_component(Some(index), cx);
    }
    self.open_component(None, cx);
  }

  pub(crate) fn close_active_tab(&mut self, cx: &mut Context<Self>) {
    let Some(index) = self.active_tab(cx) else {
      return;
    };
    // 释放此标签的 Entity 和 Subscription；共享模型由 panel / Global 保活。
    self.tabs.remove(index);
    if self.tabs.is_empty() {
      use_navigate(cx)("/".into());
    } else {
      self.select_tab(index.min(self.tabs.len() - 1), cx);
    }
    cx.notify();
  }
}

impl Render for TabbedPanel {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let notifications = Root::render_notification_layer(window, cx);
    let dialogs = Root::render_dialog_layer(window, cx);
    let sheets = Root::render_sheet_layer(window, cx);
    let active_tab = self.active_tab(cx);
    let pathname = use_location(cx).pathname.clone();
    let routes = Routes::new().children(self.tabs.iter().map(PanelTab::route));
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
          .children(self.tabs.iter().map(|tab| Tab::new().label(tab.label()))),
      )
      .child(v_flex().flex_1().min_h_0().child(if active_tab.is_some() {
        routes.into_any_element()
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
          .right(format!(
            "id: {}",
            pathname
              .rsplit_once('/')
              .map(|(_, id)| id)
              .filter(|id| !id.is_empty())
              .unwrap_or("—")
          )),
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

#[cfg(test)]
mod tests {
  use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};
  use gpui_router::use_navigate;

  use super::TabbedPanel;
  use crate::state::{AppSettings, CounterState};

  #[gpui_kit::test]
  fn external_navigation_scrolls_to_the_selected_tab(cx: &mut TestAppContext) {
    cx.update(|cx| {
      gpui_kit::init(cx);
      cx.set_global(AppSettings::default());
    });
    let model = cx.new(|_| CounterState::default());
    let panel = cx.new(|cx| TabbedPanel::new(model, cx));
    panel.update(cx, |panel, cx| {
      for _ in 0 .. 20 {
        panel.add_tab(cx);
      }
      panel.select_tab(0, cx);
    });
    let _window = cx.open_window(size(px(760.), px(700.)), |window, cx| {
      Root::new(panel.clone(), window, cx)
    });
    cx.run_until_parked();
    let scroll = panel.read_with(cx, |panel, _| panel.tab_scroll.clone());
    assert_eq!(scroll.offset().x, px(0.));

    cx.update(|cx| use_navigate(cx)("/counter/22".into()));
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
      assert_eq!(panel.active_tab(cx), Some(panel.tabs.len() - 1));
    });
    assert!(scroll.offset().x < px(0.));

    cx.update(|cx| use_navigate(cx)("/counter/1".into()));
    cx.run_until_parked();
    assert_eq!(scroll.offset().x, px(0.));
  }
}

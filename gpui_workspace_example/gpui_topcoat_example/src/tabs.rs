//! Adapted from entity_view_example::{tabbed_panel, panel_tab}.
//! The router owns selection; stable page entities preserve form drafts across navigation.
use gpui_kit::{
  component::{
    tab::{Tab, TabBar},
    *,
  },
  *,
};
use gpui_router::{Route, RouterState, Routes, use_location, use_navigate};

use crate::{
  counter::CounterView,
  demo_page::{DemoPage, PageKind},
};

struct PageTab {
  path: &'static str,
  label: &'static str,
  view: AnyView,
}
impl PageTab {
  fn route(&self) -> Route {
    let view = self.view.clone();
    Route::new()
      .path(self.path.trim_start_matches('/').to_owned())
      .element(move |_, _| view.clone())
  }
}

pub(crate) struct Workspace {
  tabs: Vec<PageTab>,
  server: String,
  _router_subscription: Subscription,
}
impl Workspace {
  pub(crate) fn new(server: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let mut tabs = vec![PageTab {
      path: "/counter",
      label: "计数器",
      view: cx.new(|cx| CounterView::new(server.clone(), cx)).into(),
    }];
    for kind in [PageKind::Todos, PageKind::Echo, PageKind::Profile] {
      tabs.push(PageTab {
        path: kind.path(),
        label: kind.label(),
        view: cx
          .new(|cx| DemoPage::new(kind, server.clone(), window, cx))
          .into(),
      });
    }
    use_navigate(cx)("/counter".into());
    let mut previous = use_location(cx).pathname.clone();
    let subscription = cx.observe_global::<RouterState>(move |_, cx| {
      let current = use_location(cx).pathname.clone();
      // Routes writes match metadata while rendering; redraw only when the path changes.
      if current != previous {
        previous = current;
        cx.notify();
      }
    });
    Self {
      tabs,
      server,
      _router_subscription: subscription,
    }
  }
}
impl Render for Workspace {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let path = use_location(cx).pathname.clone();
    let selected = self.tabs.iter().position(|tab| tab.path == path.as_ref());
    let routes = Routes::new().children(self.tabs.iter().map(PageTab::route));
    div()
      .v_flex()
      .size_full()
      .gap_2()
      .child(div().p_3().child("Topcoat × GPUI-kit · 协作工作台"))
      .child(
        TabBar::new("demo-tabs")
          .selected_index(selected.unwrap_or(0))
          .children(self.tabs.iter().map(|tab| Tab::new().label(tab.label)))
          .on_click(cx.listener(|view, index: &usize, _, cx| {
            if let Some(tab) = view.tabs.get(*index) {
              use_navigate(cx)(tab.path.into());
            }
          })),
      )
      .child(div().flex_1().min_h_0().child(if selected.is_some() {
        routes.into_any_element()
      } else {
        div()
          .child("页面不存在，请选择上方 Tab。")
          .into_any_element()
      }))
      .child(
        div()
          .p_2()
          .child(format!("路由：{path}  ·  服务：{}", self.server)),
      )
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{AppContext, TestAppContext, component::Root, px, size, test::TestWindowExt};
  use gpui_router::use_location;

  use super::Workspace;
  use crate::demo_page::DemoPage;

  #[gpui_kit::test]
  fn tab_clicks_route_and_preserve_drafts(cx: &mut TestAppContext) {
    cx.update(|cx| {
      gpui_kit::init(cx);
      gpui_router::init(cx);
    });
    let mut panel = None;
    let window = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
      let view = cx.new(|cx| Workspace::new("http://127.0.0.1:1".into(), window, cx));
      panel = Some(view.clone());
      Root::new(view, window, cx)
    });
    let panel = panel.unwrap();
    cx.run_until_parked();
    let page = panel.read_with(cx, |panel, _| {
      panel.tabs[1].view.clone().downcast::<DemoPage>().unwrap()
    });
    let draft = page.read_with(cx, |page, _| page.draft());
    cx.update_window(window.into(), |_, window, cx| {
      window.within("demo-tabs").click(1usize, cx);
      assert_eq!(use_location(cx).pathname.as_ref(), "/todos");
      draft.update(cx, |input, cx| {
        input.set_value("保留这个未提交的草稿", window, cx)
      });
      for (index, path) in [
        (2, "/echo"),
        (3, "/profile"),
        (0, "/counter"),
        (1, "/todos"),
      ] {
        window.within("demo-tabs").click(index, cx);
        assert_eq!(use_location(cx).pathname.as_ref(), path);
      }
      assert_eq!(draft.read(cx).value().as_ref(), "保留这个未提交的草稿");
      assert_eq!(panel.read(cx).tabs[1].view.entity_id(), page.entity_id());
    })
    .unwrap();
  }
}

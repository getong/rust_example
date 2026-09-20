use gpui_kit::{
  AppContext, BorrowAppContext, Entity, Subscription, TestAppContext, test::TestWindowExt,
};

use crate::{
  CounterTab,
  state::{AppSettings, CounterId, CounterState},
};

struct Probe {
  changes: [usize; 4],
  _subscriptions: Vec<Subscription>,
}

#[gpui_kit::test]
fn directory_lists_live_tabs_and_clicks_navigate(cx: &mut TestAppContext) {
  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  let window = cx.open_window(
    gpui_kit::size(gpui_kit::px(760.), gpui_kit::px(700.)),
    |window, cx| crate::Root::new(panel.clone(), window, cx),
  );
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    window.click("open-tab-directory", cx);
    window.click("open-tab-directory", cx);
  })
  .unwrap();
  let entries = panel.read_with(cx, |panel, cx| {
    assert_eq!(panel.tabs.len(), 5); // 重复打开只选中已有目录。
    panel
      .tabs
      .iter()
      .map(|tab| (tab.path(cx), tab.label(cx)))
      .collect::<Vec<_>>()
  });
  cx.update_window(window.into(), |_, window, cx| {
    for (path, label) in &entries {
      assert_eq!(
        window.find(path.clone()).label(),
        Some(format!("{label} · {path}").as_str())
      );
    }
    window.click(entries[4].0.clone(), cx); // 目录自身也可跳转。
    window.click(entries[0].0.clone(), cx);
    assert!(window.find("local-click").visible());
    window.click("new-tab", cx);
    window.click("open-tab-directory", cx);
    assert!(
      window
        .find(gpui_kit::SharedString::from("/counter/3"))
        .visible()
    );
    window.click(gpui_kit::SharedString::from("/counter/3"), cx);
    window.click("close-tab", cx);
    window.click("open-tab-directory", cx);
    assert!(
      window
        .try_find(gpui_kit::SharedString::from("/counter/3"))
        .is_none()
    );
    window.click(entries[2].0.clone(), cx);
    assert!(window.find("toast-success").visible());
    window.click("open-tab-directory", cx);
    window.click(entries[3].0.clone(), cx);
    assert!(window.find("scroll-top").visible());
    window.click("open-tab-directory", cx);
  })
  .unwrap();
  let directory = panel.read_with(cx, |panel, _| match &panel.tabs[4] {
    crate::PanelTab::Directory(tab) => tab.downgrade(),
    _ => panic!("expected directory"),
  });
  cx.update_window(window.into(), |_, window, cx| window.click("close-tab", cx))
    .unwrap();
  cx.run_until_parked();
  assert!(directory.upgrade().is_none());
  cx.update_window(window.into(), |_, window, cx| {
    window.click("open-tab-directory", cx);
    assert!(window.try_find(entries[4].0.clone()).is_none());
    assert!(window.find(entries[0].0.clone()).visible());
  })
  .unwrap();
}

#[gpui_kit::test]
fn directory_scrolls_and_does_not_keep_panel_alive(cx: &mut TestAppContext) {
  use gpui_kit::{ScrollDelta, point, px};

  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  panel.update(cx, |panel, cx| {
    for _ in 0 .. 20 {
      panel.add_tab(cx);
    }
    panel.open_directory(cx);
  });
  let scroll = panel.read_with(cx, |panel, cx| match panel.tabs.last().unwrap() {
    crate::PanelTab::Directory(tab) => tab.read(cx).scroll_handle.clone(),
    _ => panic!("expected directory"),
  });
  let window = cx.open_window(gpui_kit::size(px(760.), px(700.)), |window, cx| {
    crate::Root::new(panel.clone(), window, cx)
  });
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    window.scroll(
      gpui_kit::SharedString::from("/counter/1"),
      ScrollDelta::Pixels(point(px(0.), px(-400.))),
      cx,
    );
    let offset = scroll.offset();
    assert!(offset.y < px(0.));
    panel.update(cx, |panel, cx| panel.select_tab(0, cx));
    window.click("open-tab-directory", cx);
    assert_eq!(scroll.offset(), offset);
  })
  .unwrap();
  // 无窗口持有的面板也不会因为目录订阅或反向引用而泄漏。
  let model = cx.new(|_| CounterState::default());
  let standalone = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  standalone.update(cx, |panel, cx| panel.open_directory(cx));
  let weak = standalone.downgrade();
  cx.update(|_| drop(standalone));
  cx.run_until_parked();
  assert!(weak.upgrade().is_none());
}

#[gpui_kit::test]
fn dynamic_route_templates_keep_page_instances_separate(cx: &mut TestAppContext) {
  use crate::router::{RegisterError, TabRouter};

  cx.update(|cx| cx.set_global(AppSettings::default()));
  let model = cx.new(|_| CounterState::default());
  let first = cx.new(|cx| CounterTab::new(1, model.clone(), cx));
  let second = cx.new(|cx| CounterTab::new(2, model, cx));
  let router = cx.new(TabRouter::new);
  router.update(cx, |router, cx| {
    router.register_route("/counter/{id}").unwrap();
    router.register("/counter/1", first.clone().into()).unwrap();
    router.navigate("/counter/1", cx).unwrap();
    assert_eq!(router.param("id"), Some("1"));
    assert_eq!(router.param("missing"), None);
    // 模板匹配成功，但不存在的页面不能跳转。
    assert!(router.navigate("/counter/2", cx).is_err());
    assert_eq!(router.pathname(), Some("/counter/1"));
    assert_eq!(router.param("id"), Some("1"));
    assert_eq!(
      router.register("/counter/1", second.clone().into()),
      Err(RegisterError::DuplicatePath)
    );
    assert_eq!(
      router.register("/unknown/2", second.clone().into()),
      Err(RegisterError::UnknownRoute)
    );
    // 运行中加入同模板的新页面，注册本身不切换页面。
    router
      .register("/counter/2", second.clone().into())
      .unwrap();
    assert_eq!(router.pathname(), Some("/counter/1"));
    router.navigate("/counter/2", cx).unwrap();
    assert_eq!(router.param("id"), Some("2"));
    router.unregister("/counter/1", cx);
    assert_eq!(router.pathname(), Some("/counter/2"));
    assert!(!router.stack().read(cx).is_empty());
    assert!(router.navigate("/counter/1", cx).is_err());
    router.unregister("/counter/2", cx);
    assert_eq!(router.pathname(), None);
    assert_eq!(router.param("id"), None);
    assert!(router.stack().read(cx).is_empty());
    // 最后一个实例关闭后，模板仍可接收新页面。
    router.register("/counter/3", first.clone().into()).unwrap();
    router.navigate("/counter/3", cx).unwrap();
    assert_eq!(router.param("id"), Some("3"));
  });
}

#[gpui_kit::test]
fn route_templates_can_be_added_at_runtime(cx: &mut TestAppContext) {
  use crate::router::TabRouter;

  let page = cx.new(|_| crate::ToastTab);
  let router = cx.new(TabRouter::new);
  router.update(cx, |router, cx| {
    router.register_route("/home").unwrap();
    router.register("/home", page.clone().into()).unwrap();
    router.navigate("/home", cx).unwrap();
    router
      .register_route("/projects/{project}/files/{*file}")
      .unwrap();
    assert!(
      router
        .register_route("/projects/{project}/files/{*file}")
        .is_err()
    );
    assert!(router.register_route("/broken/{").is_err());
    assert_eq!(router.pathname(), Some("/home"));
    assert_eq!(router.param("project"), None);
    let path = "/projects/demo/files/src/main.rs";
    router.register(path, page.clone().into()).unwrap();
    router.navigate(path, cx).unwrap();
    assert_eq!(router.param("project"), Some("demo"));
    assert_eq!(router.param("file"), Some("src/main.rs"));
    router.unregister(path, cx);
    assert!(router.navigate(path, cx).is_err());
    router.navigate("/home", cx).unwrap();
    assert_eq!(router.param("file"), None);
  });
}

#[gpui_kit::test]
fn dynamically_added_tabs_navigate_preserve_state_and_release_pages(cx: &mut TestAppContext) {
  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  let router = panel.read_with(cx, |panel, _| panel.router.clone());
  let window = cx.open_window(
    gpui_kit::size(gpui_kit::px(760.), gpui_kit::px(700.)),
    |window, cx| crate::Root::new(panel.clone(), window, cx),
  );
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    window.click("new-tab", cx);
    window.click("local-click", cx);
    window.click("new-toast-tab", cx);
    window.click("new-scrollbar-tab", cx);
  })
  .unwrap();
  let paths = panel.read_with(cx, |panel, cx| {
    panel.tabs[4 ..]
      .iter()
      .map(|tab| tab.path(cx))
      .collect::<Vec<_>>()
  });
  let closed = panel.read_with(cx, |panel, _| panel.tabs[4].counter().downgrade());
  for (index, path) in paths.iter().enumerate() {
    router.update(cx, |router, cx| router.navigate(path, cx).unwrap());
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
      assert_eq!(panel.active_tab(cx), Some(4 + index))
    });
    cx.update_window(window.into(), |_, window, _| {
      let control = ["local-click", "toast-success", "scroll-top"][index];
      assert!(window.find(control).visible());
      if index == 0 {
        assert_eq!(window.find(control).label(), Some("Only this tab: 1"));
      }
    })
    .unwrap();
  }
  router.update(cx, |router, cx| router.navigate(&paths[0], cx).unwrap());
  panel.update(cx, |panel, cx| panel.close_active_tab(cx));
  cx.run_until_parked();
  assert!(closed.upgrade().is_none());
  router.update(cx, |router, cx| {
    assert!(router.navigate(&paths[0], cx).is_err());
    router.navigate("/counter/1", cx).unwrap();
    router.navigate(&paths[1], cx).unwrap();
    router.navigate(&paths[2], cx).unwrap();
  });
}

#[gpui_kit::test]
fn routes_drive_tabs_and_closed_paths_cannot_be_revisited(cx: &mut TestAppContext) {
  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  let router = panel.read_with(cx, |panel, _| panel.router.clone());
  let window = cx.open_window(
    gpui_kit::size(gpui_kit::px(760.), gpui_kit::px(700.)),
    |window, cx| crate::Root::new(panel.clone(), window, cx),
  );
  cx.run_until_parked();
  // 绕过 TabBar，直接操作 router；页面与选中状态也必须同步。
  router.update(cx, |router, cx| router.navigate("/counter/2", cx).unwrap());
  cx.run_until_parked();
  panel.read_with(cx, |panel, cx| assert_eq!(panel.active_tab(cx), Some(1)));
  cx.update_window(window.into(), |_, window, cx| {
    assert!(window.find("local-click").visible());
    window.click("local-click", cx);
    window.within("counter-tabs").click(0usize, cx);
  })
  .unwrap();
  router.read_with(cx, |router, _| {
    assert_eq!(router.pathname(), Some("/counter/1"))
  });
  router.update(cx, |router, cx| {
    assert!(router.navigate("/missing", cx).is_err());
    assert_eq!(router.pathname(), Some("/counter/1"));
    router.navigate("/counter/2", cx).unwrap();
  });
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    assert_eq!(window.find("local-click").label(), Some("Only this tab: 1"));
    window.click("close-tab", cx);
  })
  .unwrap();
  router.update(cx, |router, cx| {
    let current = router.pathname().unwrap().to_owned();
    assert!(router.navigate("/counter/2", cx).is_err());
    assert_eq!(router.pathname(), Some(current.as_str()));
  });
  // 同类新增标签也有独立路径；关闭前面的标签不改变其他标签路径。
  let original = panel.read_with(cx, |panel, cx| panel.tabs[1].path(cx));
  panel.update(cx, |panel, cx| panel.add_toast_tab(cx));
  let added = router.read_with(cx, |router, _| router.pathname().unwrap().to_owned());
  assert_ne!(original.as_ref(), added);
  router.update(cx, |router, cx| router.navigate(&original, cx).unwrap());
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, _| {
    assert!(window.find("toast-success").visible());
  })
  .unwrap();
}

#[gpui_kit::test]
fn panel_routers_are_independent_and_empty_panels_release_routes(cx: &mut TestAppContext) {
  cx.update(|cx| cx.set_global(AppSettings::default()));
  let model = cx.new(|_| CounterState::default());
  let first = cx.new(|cx| crate::TabbedPanel::new(model.clone(), cx));
  let second = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  let first_router = first.read_with(cx, |panel, _| panel.router.clone());
  let second_router = second.read_with(cx, |panel, _| panel.router.clone());
  first_router.update(cx, |router, cx| router.navigate("/counter/2", cx).unwrap());
  second_router.read_with(cx, |router, _| {
    assert_eq!(router.pathname(), Some("/counter/1"))
  });
  let paths = first.read_with(cx, |panel, cx| {
    panel
      .tabs
      .iter()
      .map(|tab| tab.path(cx))
      .collect::<Vec<_>>()
  });
  first.update(cx, |panel, cx| {
    while !panel.tabs.is_empty() {
      panel.close_active_tab(cx);
    }
  });
  first_router.update(cx, |router, cx| {
    assert_eq!(router.pathname(), None);
    assert!(router.stack().read(cx).is_empty());
    for path in paths {
      assert!(router.navigate(&path, cx).is_err());
    }
  });
  first.update(cx, |panel, cx| panel.add_tab(cx));
  first_router.read_with(cx, |router, _| {
    assert_eq!(router.pathname(), Some("/counter/3"))
  });
  second_router.read_with(cx, |router, _| {
    assert_eq!(router.pathname(), Some("/counter/1"))
  });
}

#[gpui_kit::test]
fn scrollbar_rows_show_toasts_and_restore_remembered_position(cx: &mut TestAppContext) {
  use gpui_kit::{ScrollDelta, component::WindowExt, point, px};

  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  let window = cx.open_window(gpui_kit::size(px(760.), px(700.)), |window, cx| {
    crate::Root::new(panel.clone(), window, cx)
  });
  cx.run_until_parked();
  let scroll = panel.read_with(cx, |panel, cx| match &panel.tabs[3] {
    crate::PanelTab::Scrollbar(tab) => tab.read(cx).scroll_handle.clone(),
    _ => panic!("expected scrollbar tab"),
  });
  cx.update_window(window.into(), |_, window, cx| {
    window.within("counter-tabs").click(3usize, cx);
    window.click(("scroll-row", 1usize), cx);
    assert_eq!(
      window.find(("scroll-row", 1usize)).label(),
      Some("Row 01 — Selected")
    );
    assert_eq!(window.notifications(cx).len(), 1);
    window.scroll(
      ("scroll-row", 1usize),
      ScrollDelta::Pixels(point(px(0.), px(-300.))),
      cx,
    );
    let remembered = scroll.offset();
    assert!(remembered.y < px(0.));
    window.click("scroll-save", cx);
    window.click("scroll-top", cx);
    assert_eq!(scroll.offset().y, px(0.));
    window.click("scroll-restore", cx);
    assert_eq!(scroll.offset(), remembered);
    window.within("counter-tabs").click(0usize, cx);
    window.within("counter-tabs").click(3usize, cx);
    assert_eq!(scroll.offset(), remembered);
    window.click("scroll-top", cx);
    assert_eq!(
      window.find(("scroll-row", 1usize)).label(),
      Some("Row 01 — Selected")
    );
    // 保存的位置不会被回到顶部或标签切换覆盖。
    window.click("scroll-restore", cx);
    assert_eq!(scroll.offset(), remembered);
  })
  .unwrap();
}

#[gpui_kit::test]
fn scrollbar_tab_scrolls_preserves_position_and_resets(cx: &mut TestAppContext) {
  use gpui_kit::{InputEvent, ScrollDelta, ScrollWheelEvent, point, px};

  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model, cx));
  let window = cx.open_window(gpui_kit::size(px(640.), px(640.)), |window, cx| {
    crate::Root::new(panel.clone(), window, cx)
  });
  cx.run_until_parked();
  let scroll = panel.read_with(cx, |panel, cx| match &panel.tabs[3] {
    crate::PanelTab::Scrollbar(tab) => tab.read(cx).scroll_handle.clone(),
    _ => panic!("expected scrollbar tab"),
  });
  cx.update_window(window.into(), |_, window, cx| {
    window.within("counter-tabs").click(3usize, cx);
    window.dispatch_event(
      ScrollWheelEvent {
        position: scroll.bounds().center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-300.))),
        ..Default::default()
      }
      .to_platform_input(),
      cx,
    );
    window.render_frame(cx);
    let offset = scroll.offset();
    assert!(offset.y < px(0.));
    window.within("counter-tabs").click(0usize, cx);
    window.within("counter-tabs").click(3usize, cx);
    assert_eq!(scroll.offset(), offset);
    window.click("scroll-top", cx);
    assert_eq!(scroll.offset().y, px(0.));
    window.click("close-tab", cx);
    window.click("new-scrollbar-tab", cx);
    assert!(window.find("scroll-top").visible());
  })
  .unwrap();
  panel.read_with(cx, |panel, cx| {
    assert_eq!(panel.tabs.len(), 4);
    assert_eq!(panel.active_tab(cx), Some(3));
    assert!(matches!(panel.tabs[3], crate::PanelTab::Scrollbar(_)));
  });
}

#[gpui_kit::test]
fn toast_tab_shows_notifications_and_can_be_closed_and_reopened(cx: &mut TestAppContext) {
  use gpui_kit::{
    Styled,
    component::{ActiveTheme, WindowExt},
  };

  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model.clone(), cx));
  let window = cx.open_window(
    gpui_kit::size(gpui_kit::px(760.), gpui_kit::px(700.)),
    |window, cx| crate::Root::new(panel.clone(), window, cx),
  );
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    window.within("counter-tabs").click(2usize, cx);
    for (index, button) in [
      "toast-success",
      "toast-info",
      "toast-warning",
      "toast-error",
    ]
    .into_iter()
    .enumerate()
    {
      window.click(button, cx);
      assert_eq!(window.notifications(cx).len(), index + 1);
      for note in window.notifications(cx).iter() {
        note.update(cx, |note, cx| {
          let foreground = note.text_style().color.expect("explicit toast text color");
          assert_eq!(foreground, cx.theme().popover_foreground);
          assert_ne!(foreground, cx.theme().popover);
        });
      }
    }
    window.within("counter-tabs").click(0usize, cx);
    assert_eq!(window.notifications(cx).len(), 4);
    window.click("increment-a", cx);
    window.within("counter-tabs").click(2usize, cx);
    window.clear_notifications(cx);
  })
  .unwrap();
  // 清除先播放退出动画，推进测试时钟后才真正移除通知。
  cx.background_executor
    .advance_clock(std::time::Duration::from_secs(1));
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    assert!(window.notifications(cx).is_empty());
    // 通知浮层退出后再点击工具栏，避免被浮层遮挡。
    window.click("close-tab", cx);
    window.click("new-toast-tab", cx);
    window.click("toast-success", cx);
    assert_eq!(window.notifications(cx).len(), 1);
    let close = window.find("dismiss-toast");
    assert_eq!(close.label(), Some("Close"));
    assert!(close.visible());
    window.click("dismiss-toast", cx);
  })
  .unwrap();
  cx.run_until_parked();
  cx.background_executor
    .advance_clock(std::time::Duration::from_secs(1));
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    assert!(window.notifications(cx).is_empty());
    window.click("toast-info", cx);
    window.hover("toast-info", cx);
  })
  .unwrap();
  // 逐秒推进，覆盖进入、5 秒超时和退出；无需点击清除按钮。
  for _ in 0 .. 7 {
    cx.run_until_parked();
    cx.background_executor
      .advance_clock(std::time::Duration::from_secs(1));
  }
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    assert!(window.notifications(cx).is_empty());
  })
  .unwrap();
  panel.read_with(cx, |panel, cx| {
    assert_eq!(panel.active_tab(cx), Some(3));
    assert!(matches!(panel.tabs[3], crate::PanelTab::Toast(_)));
  });
  model.read_with(cx, |model, _| assert_eq!(model.total(), 1));
}

#[gpui_kit::test]
fn tabs_share_state_preserve_local_state_and_release_closed_views(cx: &mut TestAppContext) {
  cx.update(|cx| {
    gpui_kit::init(cx);
    cx.set_global(AppSettings::default());
  });
  let model = cx.new(|_| CounterState::default());
  let panel = cx.new(|cx| crate::TabbedPanel::new(model.clone(), cx));
  let window = cx.open_window(
    gpui_kit::size(gpui_kit::px(760.), gpui_kit::px(700.)),
    |window, cx| crate::Root::new(panel.clone(), window, cx),
  );
  cx.run_until_parked();
  // Toast 和 Scrollbar 标签单独测试，这里保留原有两个计数标签的生命周期场景。
  panel.update(cx, |panel, cx| {
    panel.select_tab(3, cx);
    panel.close_active_tab(cx);
    panel.select_tab(2, cx);
    panel.close_active_tab(cx);
    panel.select_tab(0, cx);
    cx.notify();
  });
  let second = panel.read_with(cx, |panel, _| panel.tabs[1].counter().clone());
  let summary = second.read_with(cx, |tab, _| tab.summary.clone());
  let probe = cx.new(|cx| Probe {
    changes: [0; 4],
    _subscriptions: vec![cx.observe(&summary, |probe: &mut Probe, _, _| probe.changes[0] += 1)],
  });
  cx.run_until_parked();
  cx.update_window(window.into(), |_, window, cx| {
    window.click("increment-a", cx);
    window.click("local-click", cx);
  })
  .unwrap();
  cx.run_until_parked();
  // 隐藏标签也收到了模型通知；标签切换没有重建任何视图。
  probe.read_with(cx, |probe, _| assert!(probe.changes[0] > 0));
  second.read_with(cx, |tab, cx| {
    assert_eq!(tab.summary.read(cx).model.read(cx).total(), 1)
  });
  cx.update_window(window.into(), |_, window, cx| {
    window.within("counter-tabs").click(1usize, cx);
    assert_eq!(window.find("local-click").label(), Some("Only this tab: 0"));
    window.click("toggle-step", cx);
    window.click("increment-b", cx);
    window.within("counter-tabs").click(0usize, cx);
    assert_eq!(window.find("local-click").label(), Some("Only this tab: 1"));
    assert_eq!(window.find("increment-a").label(), Some("+5"));
  })
  .unwrap();
  model.read_with(cx, |model, _| assert_eq!(model.total(), 6));
  cx.update_window(window.into(), |_, window, cx| window.click("new-tab", cx))
    .unwrap();
  panel.read_with(cx, |panel, cx| {
    assert_eq!(panel.tabs.len(), 3);
    assert_eq!(panel.active_tab(cx), Some(2));
    let tab = panel.tabs[2].counter().read(cx);
    assert_eq!(tab.local_clicks, 0);
    assert_eq!(tab.summary.read(cx).model.read(cx).total(), 6);
  });
  let closed = panel.read_with(cx, |panel, _| panel.tabs[2].counter().downgrade());
  cx.update_window(window.into(), |_, window, cx| window.click("close-tab", cx))
    .unwrap();
  cx.run_until_parked();
  assert!(closed.upgrade().is_none());
  panel.read_with(cx, |panel, cx| assert_eq!(panel.active_tab(cx), Some(1)));
  cx.update_window(window.into(), |_, window, cx| {
    window.click("reset-all", cx);
    window.within("counter-tabs").click(0usize, cx);
    assert_eq!(window.find("local-click").label(), Some("Only this tab: 1"));
    window.click("increment-a", cx);
    // 关闭非末尾标签，再关闭最后一个标签；进入空面板。
    window.click("close-tab", cx);
    window.click("close-tab", cx);
  })
  .unwrap();
  panel.read_with(cx, |panel, _| assert!(panel.tabs.is_empty()));
  cx.update_window(window.into(), |_, window, cx| {
    window.click("new-tab", cx);
    assert_eq!(window.find("local-click").label(), Some("Only this tab: 0"));
    assert_eq!(window.find("increment-a").label(), Some("+5"));
  })
  .unwrap();
  panel.read_with(cx, |panel, cx| {
    assert_eq!(panel.active_tab(cx), Some(0));
    assert_eq!(panel.tabs[0].counter().read(cx).tab_number, 4);
    assert_eq!(
      panel.tabs[0]
        .counter()
        .read(cx)
        .summary
        .read(cx)
        .model
        .read(cx)
        .total(),
      5
    );
  });
}

fn fixture(cx: &mut TestAppContext) -> (Entity<CounterTab>, Entity<CounterState>, Entity<Probe>) {
  cx.update(|cx| cx.set_global(AppSettings::default()));
  let shared = cx.new(|_| CounterState::default());
  let app = cx.new(|cx| CounterTab::new(1, shared, cx));
  cx.run_until_parked();
  let (a, b, summary, model) = app.read_with(cx, |app, cx| {
    (
      app.counters[0].clone(),
      app.counters[1].clone(),
      app.summary.clone(),
      app.summary.read(cx).model.clone(),
    )
  });
  let probe = cx.new(|cx| Probe {
    changes: [0; 4],
    _subscriptions: vec![
      cx.observe(&a, |probe: &mut Probe, _, _| probe.changes[0] += 1),
      cx.observe(&b, |probe: &mut Probe, _, _| probe.changes[1] += 1),
      cx.observe(&summary, |probe: &mut Probe, _, _| probe.changes[2] += 1),
      cx.observe(&app, |probe: &mut Probe, _, _| probe.changes[3] += 1),
    ],
  });
  cx.run_until_parked();
  (app, model, probe)
}

#[gpui_kit::test]
fn shared_model_notifies_views_and_resets_both_counts(cx: &mut TestAppContext) {
  let (_app, model, probe) = fixture(cx);
  model.update(cx, |model, cx| {
    model.increment(CounterId::A, cx);
    model.increment(CounterId::B, cx);
    model.increment(CounterId::A, cx);
  });
  // update 同步完成，无需等待事件队列才能读到新状态。
  model.read_with(cx, |model, _| {
    assert_eq!(model.count(CounterId::A), 2);
    assert_eq!(model.count(CounterId::B), 1);
    assert_eq!(model.total(), 3);
    assert_eq!(model.last_change(), "Counter A changed to 2");
  });
  cx.run_until_parked();
  probe.read_with(cx, |probe, _| {
    assert!(probe.changes[.. 3].iter().all(|&n| n > 0));
    assert_eq!(probe.changes[3], 0);
  });
  probe.update(cx, |probe, _| probe.changes = [0; 4]);
  model.update(cx, CounterState::reset);
  cx.run_until_parked();
  model.read_with(cx, |model, _| {
    assert_eq!(model.count(CounterId::A), 0);
    assert_eq!(model.count(CounterId::B), 0);
    assert_eq!(model.total(), 0);
  });
  probe.read_with(cx, |probe, _| {
    assert!(probe.changes[.. 3].iter().all(|&n| n > 0))
  });
}

#[gpui_kit::test]
fn global_settings_notify_consumers_without_changing_counts(cx: &mut TestAppContext) {
  let (_app, model, probe) = fixture(cx);
  cx.update(|cx| cx.update_global::<AppSettings, _>(|settings, _| settings.toggle_step()));
  cx.run_until_parked();
  probe.read_with(cx, |probe, _| {
    assert!(probe.changes[0] > 0 && probe.changes[1] > 0 && probe.changes[3] > 0);
    assert_eq!(probe.changes[2], 0); // Summary 不观察 Global。
  });
  model.read_with(cx, |model, _| assert_eq!(model.total(), 0));
  model.update(cx, |model, cx| model.increment(CounterId::B, cx));
  model.read_with(cx, |model, _| assert_eq!(model.count(CounterId::B), 5));
  model.update(cx, CounterState::reset);
  cx.update(|cx| assert_eq!(cx.global::<AppSettings>().step(), 5));
  cx.update(|cx| cx.update_global::<AppSettings, _>(|settings, _| settings.toggle_step()));
  model.update(cx, |model, cx| model.increment(CounterId::A, cx));
  model.read_with(cx, |model, _| assert_eq!(model.count(CounterId::A), 1));
}

#[gpui_kit::test]
fn views_own_model_and_subscriptions_do_not_keep_it_alive(cx: &mut TestAppContext) {
  let (app, model, _probe) = fixture(cx);
  let weak_model = model.downgrade();
  drop(model);
  assert!(weak_model.upgrade().is_some());
  // 在 App 更新周期中释放，让 GPUI 执行实体清理及级联释放。
  cx.update(|_| drop(app));
  cx.run_until_parked();
  assert!(weak_model.upgrade().is_none());
  // 视图已销毁，更新 Global 不会调用失效视图。
  cx.update(|cx| cx.update_global::<AppSettings, _>(|settings, _| settings.toggle_step()));
  cx.run_until_parked();
}

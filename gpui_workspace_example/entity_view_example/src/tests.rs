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
  panel.read_with(cx, |panel, _| {
    assert_eq!(panel.tabs.len(), 4);
    assert_eq!(panel.active_tab, 3);
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
  panel.read_with(cx, |panel, _| {
    assert_eq!(panel.active_tab, 3);
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
    panel.active_tab = 3;
    panel.close_active_tab(cx);
    panel.active_tab = 2;
    panel.close_active_tab(cx);
    panel.active_tab = 0;
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
    assert_eq!(panel.active_tab, 2);
    let tab = panel.tabs[2].counter().read(cx);
    assert_eq!(tab.local_clicks, 0);
    assert_eq!(tab.summary.read(cx).model.read(cx).total(), 6);
  });
  let closed = panel.read_with(cx, |panel, _| panel.tabs[2].counter().downgrade());
  cx.update_window(window.into(), |_, window, cx| window.click("close-tab", cx))
    .unwrap();
  cx.run_until_parked();
  assert!(closed.upgrade().is_none());
  panel.read_with(cx, |panel, _| assert_eq!(panel.active_tab, 1));
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
    assert_eq!(panel.active_tab, 0);
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

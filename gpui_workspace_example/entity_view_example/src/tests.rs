use gpui_kit::{AppContext, BorrowAppContext, Entity, Subscription, TestAppContext};

use crate::{
  CounterApp,
  state::{AppSettings, CounterId, CounterState},
};

struct Probe {
  changes: [usize; 4],
  _subscriptions: Vec<Subscription>,
}

fn fixture(cx: &mut TestAppContext) -> (Entity<CounterApp>, Entity<CounterState>, Entity<Probe>) {
  cx.update(|cx| cx.set_global(AppSettings::default()));
  let app = cx.new(CounterApp::new);
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

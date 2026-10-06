use std::time::Duration;

use gpui_kit::{
  component::{button::*, *},
  *,
};
use topcoat_gpui_protocol::{CounterAction, CounterSnapshot};

pub(crate) struct CounterView {
  server: String,
  snapshot: Option<CounterSnapshot>,
  status: String,
  loading: bool,
  request: Option<Task<()>>,
  poll: Option<Task<()>>,
}

impl CounterView {
  pub(crate) fn new(server: String, cx: &mut Context<Self>) -> Self {
    let mut view = Self {
      server,
      snapshot: None,
      status: "Connecting…".into(),
      loading: false,
      request: None,
      poll: None,
    };
    view.refresh(None, cx);
    view.poll = Some(cx.spawn(async move |view, cx| {
      loop {
        cx.background_executor().timer(Duration::from_secs(2)).await;
        if view.update(cx, |view, cx| view.refresh(None, cx)).is_err() {
          break;
        }
      }
    }));
    view
  }

  fn refresh(&mut self, action: Option<CounterAction>, cx: &mut Context<Self>) {
    if self.loading {
      return;
    }
    self.loading = true;
    let server = self.server.clone();
    let work = gpui_topcoat_example::runtime::spawn(async move {
      gpui_topcoat_example::request(&server, action).await
    });
    self.request = Some(cx.spawn(async move |view, cx| {
      let result = work.await;
      let _ = view.update(cx, |view, cx| {
        view.loading = false;
        match result {
          Ok(snapshot) => {
            view.snapshot = Some(snapshot);
            view.status = "Connected · sync every 2 seconds".into();
          }
          Err(error) => {
            view.status = format!("Disconnected · displayed value may be stale. {error}")
          }
        }
        cx.notify();
      });
    }));
    cx.notify();
  }
}

impl Render for CounterView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .v_flex()
      .size_full()
      .p_6()
      .gap_4()
      .child("Topcoat × GPUI-kit")
      .child(format!("Server: {}", self.server))
      .child(self.snapshot.as_ref().map_or("Counter: —".into(), |s| {
        format!("Counter: {} · revision {}", s.value, s.revision)
      }))
      .child(
        div()
          .flex()
          .gap_3()
          .child(
            Button::new("increment")
              .primary()
              .label("+1")
              .disabled(self.loading)
              .on_click(cx.listener(|v, _, _, cx| v.refresh(Some(CounterAction::Increment), cx))),
          )
          .child(
            Button::new("reset")
              .label("Reset")
              .disabled(self.loading)
              .on_click(cx.listener(|v, _, _, cx| v.refresh(Some(CounterAction::Reset), cx))),
          )
          .child(
            Button::new("refresh")
              .label("Refresh")
              .disabled(self.loading)
              .on_click(cx.listener(|v, _, _, cx| v.refresh(None, cx))),
          ),
      )
      .child(self.status.clone())
      .child("Open the server URL in a browser. Both windows share the same counter.")
  }
}

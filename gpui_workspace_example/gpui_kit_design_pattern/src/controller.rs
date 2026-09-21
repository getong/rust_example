//! 集中式控制器：纯数据实体在构造时自监听，多个视图只发送命令。
//! 当前 GPUI 的自监听 API 是 subscribe_self，而非文档中的 listen。
use gpui_kit::{component::button::*, *};

enum ControllerSignal {
  ResetAllCounters,
  AddNewNotification(&'static str),
}

struct AppController {
  total_clicks: u32,
  last_notification: Option<&'static str>,
  _listener: Subscription,
}

impl EventEmitter<ControllerSignal> for AppController {}

impl AppController {
  fn new(cx: &mut Context<Self>) -> Self {
    let listener = cx.subscribe_self(|this, signal: &ControllerSignal, cx| {
      match signal {
        ControllerSignal::ResetAllCounters => {
          this.total_clicks = 0;
          this.last_notification = None;
        }
        ControllerSignal::AddNewNotification(message) => {
          this.total_clicks += 1;
          this.last_notification = Some(message);
        }
      }
      cx.notify();
    });
    Self {
      total_clicks: 0,
      last_notification: None,
      _listener: listener,
    }
  }
}

struct Sender {
  name: &'static str,
  controller: Entity<AppController>,
}

impl Render for Sender {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    Button::new("send")
      .label(format!("Send from {}", self.name))
      .on_click(cx.listener(|this, _, _, cx| {
        // 必须在目标实体的 Context 上 emit，不会自动广播到所有实体。
        this.controller.update(cx, |_, cx| {
          cx.emit(ControllerSignal::AddNewNotification(this.name));
        });
      }))
  }
}

pub(crate) struct ControllerDemo {
  controller: Entity<AppController>,
  senders: [Entity<Sender>; 2],
  _observer: Subscription,
}

impl ControllerDemo {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    let controller = cx.new(AppController::new);
    let senders = ["View A", "View B"].map(|name| {
      cx.new(|_| Sender {
        name,
        controller: controller.clone(),
      })
    });
    let observer = cx.observe(&controller, |_, _, cx| cx.notify());
    Self {
      controller,
      senders,
      _observer: observer,
    }
  }
}

impl Render for ControllerDemo {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let state = self.controller.read(cx);
    let summary = format!(
      "Total: {} | Last sender: {}",
      state.total_clicks,
      state.last_notification.unwrap_or("None")
    );
    div().flex().flex_col().gap_3().child(summary).child(
      div()
        .flex()
        .gap_3()
        .children(self.senders.iter().cloned())
        .child(
          Button::new("reset-controller")
            .label("Reset all")
            .on_click(cx.listener(|this, _, _, cx| {
              this
                .controller
                .update(cx, |_, cx| cx.emit(ControllerSignal::ResetAllCounters));
            })),
        ),
    )
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{AppContext, TestAppContext};

  use super::{AppController, ControllerSignal};

  #[gpui_kit::test]
  fn self_listener_handles_multiple_senders_and_reset(cx: &mut TestAppContext) {
    let controller = cx.new(AppController::new);
    let other_sender = controller.clone();
    controller.update(cx, |_, cx| {
      cx.emit(ControllerSignal::AddNewNotification("A"))
    });
    other_sender.update(cx, |_, cx| {
      cx.emit(ControllerSignal::AddNewNotification("B"))
    });
    cx.run_until_parked();
    controller.read_with(cx, |state, _| {
      assert_eq!(state.total_clicks, 2);
      assert_eq!(state.last_notification, Some("B"));
    });
    other_sender.update(cx, |_, cx| cx.emit(ControllerSignal::ResetAllCounters));
    cx.run_until_parked();
    controller.read_with(cx, |state, _| {
      assert_eq!(state.total_clicks, 0);
      assert_eq!(state.last_notification, None);
    });
  }
}

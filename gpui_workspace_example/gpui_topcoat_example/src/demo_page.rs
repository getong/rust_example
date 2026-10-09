use std::time::Duration;

use gpui_kit::{
  component::{
    button::*,
    input::{Input, InputState},
    *,
  },
  *,
};
use gpui_topcoat_example::{DemoCommand, demo_request};
use topcoat_gpui_protocol::{DemoSnapshot, Profile, TodoCommand};

#[derive(Clone, Copy)]
pub(crate) enum PageKind {
  Todos,
  Echo,
  Profile,
}
impl PageKind {
  pub(crate) fn path(self) -> &'static str {
    match self {
      Self::Todos => "/todos",
      Self::Echo => "/echo",
      Self::Profile => "/profile",
    }
  }
  pub(crate) fn label(self) -> &'static str {
    match self {
      Self::Todos => "待办事项",
      Self::Echo => "JSON 回显",
      Self::Profile => "表单提交",
    }
  }
}

pub(crate) struct DemoPage {
  kind: PageKind,
  server: String,
  input: Entity<InputState>,
  age: Entity<InputState>,
  snapshot: DemoSnapshot,
  status: String,
  loading: bool,
  _request: Option<Task<()>>,
  _poll: Task<()>,
}
impl DemoPage {
  pub(crate) fn new(
    kind: PageKind,
    server: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let input = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder(match kind {
          PageKind::Todos => "输入待办事项",
          PageKind::Echo => "输入 JSON",
          PageKind::Profile => "姓名",
        })
        .default_value(if matches!(kind, PageKind::Echo) {
          "{\"message\":\"Hello from GPUI\"}"
        } else {
          ""
        })
    });
    let age = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("年龄 0–150")
        .default_value("18")
    });
    let poll = cx.spawn(async move |view, cx| {
      loop {
        cx.background_executor().timer(Duration::from_secs(2)).await;
        if view
          .update(cx, |view, cx| view.send(DemoCommand::Refresh, cx))
          .is_err()
        {
          break;
        }
      }
    });
    let mut view = Self {
      kind,
      server,
      input,
      age,
      snapshot: DemoSnapshot::default(),
      status: "正在连接…".into(),
      loading: false,
      _request: None,
      _poll: poll,
    };
    view.send(DemoCommand::Refresh, cx);
    view
  }

  fn send(&mut self, command: DemoCommand, cx: &mut Context<Self>) {
    // UI-only tests use an empty origin; real I/O has separate Tokio integration tests.
    #[cfg(test)]
    if self.server.is_empty() {
      return;
    }
    if self.loading {
      return;
    }
    self.loading = true;
    let server = self.server.clone();
    let work =
      gpui_topcoat_example::runtime::spawn(async move { demo_request(&server, command).await });
    self._request = Some(cx.spawn(async move |view, cx| {
      let result = work.await;
      let _ = view.update(cx, |view, cx| {
        view.loading = false;
        match result {
          Ok(snapshot) => {
            view.snapshot = snapshot;
            view.status = "已同步 · 每两秒刷新 · 切换 Tab 保留输入".into();
          }
          Err(error) => view.status = format!("请求失败，当前数据可能已过期：{error}"),
        }
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn submit(&mut self, cx: &mut Context<Self>) {
    let text = self.input.read(cx).value().to_string();
    let command = match self.kind {
      PageKind::Todos => Ok(DemoCommand::Todo(TodoCommand::Create { title: text })),
      PageKind::Echo => serde_json::from_str(&text)
        .map(DemoCommand::Echo)
        .map_err(|e| format!("JSON 格式错误：{e}")),
      PageKind::Profile => self
        .age
        .read(cx)
        .value()
        .parse::<u8>()
        .map(|age| {
          DemoCommand::Profile(Profile {
            username: text,
            age,
          })
        })
        .map_err(|_| "年龄必须为 0–150 的整数".to_owned()),
    };
    match command {
      Ok(command) => self.send(command, cx),
      Err(error) => {
        self.status = error;
        cx.notify();
      }
    }
  }
}
impl Render for DemoPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let mut editor = div().v_flex().gap_2().child(Input::new(&self.input));
    if matches!(self.kind, PageKind::Profile) {
      editor = editor.child(Input::new(&self.age));
    }
    let mut results = div().v_flex().gap_2();
    match self.kind {
      PageKind::Todos => {
        for todo in &self.snapshot.todos {
          let id = todo.id;
          let done = !todo.done;
          results = results.child(
            div()
              .flex()
              .gap_2()
              .child(format!(
                "{} {}",
                if todo.done { "✓" } else { "○" },
                todo.title
              ))
              .child(
                Button::new(("toggle", id))
                  .label(if todo.done { "恢复待办" } else { "完成" })
                  .disabled(self.loading)
                  .on_click(cx.listener(move |view, _, _, cx| {
                    view.send(DemoCommand::Todo(TodoCommand::SetDone { id, done }), cx)
                  })),
              )
              .child(
                Button::new(("delete", id))
                  .label("删除")
                  .disabled(self.loading)
                  .on_click(cx.listener(move |view, _, _, cx| {
                    view.send(DemoCommand::Todo(TodoCommand::Delete { id }), cx)
                  })),
              ),
          );
        }
        if self.snapshot.todos.is_empty() {
          results = results.child("暂无待办，请在网页或桌面添加。");
        }
      }
      PageKind::Echo => {
        results = results.child("最近 20 次 JSON 回显（两端共享）");
        for value in &self.snapshot.echoes {
          results = results.child(serde_json::to_string_pretty(value).unwrap_or_default());
        }
      }
      PageKind::Profile => {
        results = results.child("最近 20 次表单提交（两端共享）");
        for profile in &self.snapshot.profiles {
          results = results.child(format!("{} · {} 岁", profile.username, profile.age));
        }
      }
    }
    div()
      .id("demo-page")
      .v_flex()
      .size_full()
      .overflow_y_scroll()
      .p_4()
      .gap_3()
      .child(self.kind.label())
      .child(editor)
      .child(
        div()
          .flex()
          .gap_2()
          .child(
            Button::new("submit")
              .primary()
              .label("提交")
              .disabled(self.loading)
              .on_click(cx.listener(|view, _, _, cx| view.submit(cx))),
          )
          .child(
            Button::new("refresh")
              .label("刷新")
              .disabled(self.loading)
              .on_click(cx.listener(|view, _, _, cx| view.send(DemoCommand::Refresh, cx))),
          ),
      )
      .child(self.status.clone())
      .child(results)
  }
}

#[cfg(test)]
impl DemoPage {
  pub(crate) fn draft(&self) -> Entity<InputState> {
    self.input.clone()
  }
}

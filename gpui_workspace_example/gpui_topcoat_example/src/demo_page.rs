use std::time::Duration;

use gpui_kit::{
  component::{
    button::*,
    input::{Input, InputEvent, InputState},
    *,
  },
  *,
};
use gpui_topcoat_example::{DemoCommand, demo_request, suggest};
use topcoat_gpui_protocol::{
  DemoSnapshot, Profile, Suggestion, SuggestionKind, SuggestionSource, TodoCommand,
};

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
  suggestions: Vec<Suggestion>,
  suggestion_active: Option<usize>,
  suggestion_open: bool,
  suggestion_status: String,
  suggestion_generation: u64,
  _suggest_task: Option<Task<()>>,
  _input_subscription: Subscription,
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
    let subscription = cx.subscribe_in(
      &input,
      window,
      |view, _, event: &InputEvent, window, cx| match event {
        InputEvent::Change | InputEvent::Focus => view.queue_suggestions(window, cx),
        InputEvent::Blur => view.close_suggestions(cx),
        _ => {}
      },
    );
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
      suggestions: Vec::new(),
      suggestion_active: None,
      suggestion_open: false,
      suggestion_status: String::new(),
      suggestion_generation: 0,
      _suggest_task: None,
      _input_subscription: subscription,
    };
    view.send(DemoCommand::Refresh, cx);
    view
  }

  fn close_suggestions(&mut self, cx: &mut Context<Self>) {
    self.suggestion_generation += 1;
    self._suggest_task = None;
    self.suggestions.clear();
    self.suggestion_active = None;
    self.suggestion_open = false;
    cx.notify();
  }
  fn queue_suggestions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.close_suggestions(cx);
    let kind = match self.kind {
      PageKind::Todos => SuggestionKind::Todo,
      PageKind::Profile => SuggestionKind::Profile,
      PageKind::Echo => return,
    };
    let composing = self.input.update(cx, |input, cx| {
      input.marked_text_range(window, cx).is_some()
    });
    let query = self.input.read(cx).value().trim().to_owned();
    if composing || query.is_empty() || query.chars().count() > 120 {
      return;
    }
    #[cfg(test)]
    if self.server.is_empty() {
      return;
    }
    self.suggestion_open = true;
    self.suggestion_status = "正在查找建议…".into();
    let generation = self.suggestion_generation;
    let server = self.server.clone();
    self._suggest_task = Some(cx.spawn(async move |view, cx| {
      cx.background_executor()
        .timer(Duration::from_millis(180))
        .await;
      let requested = query.clone();
      let result =
        gpui_topcoat_example::runtime::spawn(
          async move { suggest(&server, kind, requested).await },
        )
        .await;
      let _ = view.update(cx, |view, cx| {
        if generation != view.suggestion_generation || view.input.read(cx).value().trim() != query {
          return;
        }
        match result {
          Ok(result) if result.query == query => {
            view.suggestions = result.items;
            view.suggestion_status = if view.suggestions.is_empty() {
              "暂无匹配，可以直接提交新内容".into()
            } else {
              format!(
                "{} 条建议 · ↑↓ 选择 · Enter 填入 · Esc 关闭",
                view.suggestions.len()
              )
            };
          }
          _ => view.suggestion_status = "建议暂不可用，仍可直接输入并提交".into(),
        }
        cx.notify();
      });
    }));
    cx.notify();
  }
  fn choose_suggestion(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
    let Some(item) = self.suggestions.get(index).cloned() else {
      return;
    };
    self.close_suggestions(cx);
    self.input.update(cx, |input, cx| {
      input.set_value(item.value, window, cx);
      input.focus(window, cx);
    });
  }
  fn suggestion_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    if !self.suggestion_open || !self.input.read(cx).focus_handle(cx).is_focused(window) {
      return;
    }
    if self.input.update(cx, |input, cx| {
      input.marked_text_range(window, cx).is_some()
    }) {
      return;
    }
    let count = self.suggestions.len();
    match event.keystroke.key.as_str() {
      "escape" | "tab" => {
        self.close_suggestions(cx);
        if event.keystroke.key == "tab" {
          return;
        }
      }
      "down" if count > 0 => {
        self.suggestion_active = Some(self.suggestion_active.map_or(0, |n| (n + 1) % count))
      }
      "up" if count > 0 => {
        self.suggestion_active = Some(
          self
            .suggestion_active
            .map_or(count - 1, |n| (n + count - 1) % count),
        )
      }
      "enter" if self.suggestion_active.is_some() => {
        self.choose_suggestion(self.suggestion_active.unwrap(), window, cx)
      }
      _ => return,
    }
    cx.stop_propagation();
    cx.notify();
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
    self.close_suggestions(cx);
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
    let mut editor = div()
      .v_flex()
      .gap_2()
      .capture_key_down(cx.listener(Self::suggestion_key))
      .child(Input::new(&self.input));
    if matches!(self.kind, PageKind::Todos | PageKind::Profile) {
      editor = editor.child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child(if matches!(self.kind, PageKind::Todos) {
            "输入「学」、rust 或 kf，查看实时建议"
          } else {
            "输入「张」、Alice 或 zxm，查找联系人"
          }),
      );
    }
    if self.suggestion_open {
      let mut dropdown = div()
        .id("suggestion-list")
        .v_flex()
        .p_2()
        .gap_1()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background);
      for (index, item) in self.suggestions.iter().enumerate() {
        dropdown = dropdown.child(
          div()
            .id(("suggestion", index))
            .v_flex()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .bg(if self.suggestion_active == Some(index) {
              cx.theme().accent
            } else {
              cx.theme().background
            })
            .hover(|style| style.bg(cx.theme().accent))
            .on_mouse_down(
              MouseButton::Left,
              cx.listener(move |view, _, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                view.choose_suggestion(index, window, cx);
              }),
            )
            .child(item.value.clone())
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                  "{} · {}",
                  if item.source == SuggestionSource::Shared {
                    "↔ 共享记录"
                  } else {
                    "✦ 常用建议"
                  },
                  item.detail
                )),
            )
            .test_support(),
        );
      }
      editor = editor.child(
        dropdown.child(
          div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(self.suggestion_status.clone()),
        ),
      );
    }
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

#[cfg(test)]
mod autocomplete_tests {
  use gpui_kit::{AppContext, TestAppContext, component::Root, px, size, test::TestWindowExt};
  use topcoat_gpui_protocol::{Suggestion, SuggestionSource};

  use super::{DemoPage, PageKind};

  #[gpui_kit::test]
  fn suggestions_fill_locally_with_keyboard_and_mouse(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut page = None;
    let window = cx.open_window(size(px(900.), px(800.)), |window, cx| {
      let view = cx.new(|cx| DemoPage::new(PageKind::Todos, String::new(), window, cx));
      page = Some(view.clone());
      Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| {
      page.update(cx, |view, cx| {
        view.input.update(cx, |input, cx| input.focus(window, cx));
      });
    })
    .unwrap();
    cx.run_until_parked();
    let seed = |cx: &mut TestAppContext| {
      page.update(cx, |view, cx| {
        view.suggestions = vec![Suggestion {
          value: "学习 Rust".into(),
          detail: "测试建议".into(),
          source: SuggestionSource::Suggested,
        }];
        view.suggestion_open = true;
        view.suggestion_active = None;
        cx.notify();
      });
      cx.run_until_parked();
    };
    seed(cx);
    cx.simulate_keystrokes(window.into(), "down enter");
    page.read_with(cx, |view, cx| {
      assert_eq!(view.input.read(cx).value().as_ref(), "学习 Rust");
      assert!(!view.suggestion_open);
      assert!(view.snapshot.todos.is_empty());
    });
    seed(cx);
    cx.simulate_keystrokes(window.into(), "escape");
    assert!(!page.read_with(cx, |view, _| view.suggestion_open));
    seed(cx);
    cx.update_window(window.into(), |_, window, cx| {
      window
        .within("suggestion-list")
        .click(("suggestion", 0usize), cx);
      assert_eq!(page.read(cx).input.read(cx).value().as_ref(), "学习 Rust");
      assert!(!page.read(cx).suggestion_open);
    })
    .unwrap();
  }
}

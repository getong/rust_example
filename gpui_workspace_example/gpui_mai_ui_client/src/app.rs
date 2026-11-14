use std::{
  cell::RefCell,
  path::PathBuf,
  rc::Rc,
  sync::mpsc::TryRecvError,
  time::{Duration, Instant},
};

use gpui_kit::{
  component::{
    button::*,
    input::{Input, InputState, Textarea, TextareaState},
    *,
  },
  *,
};
use gpui_mai_ui_client::{
  document,
  generation::{self, Event, Job},
  grounding::GroundingSession,
  session::Session,
};

use crate::{
  grounding_ui::GroundingPanel,
  preview::{self, Preview},
};

struct Running {
  job: Job,
  prompt: String,
  started: Instant,
  connected: bool,
  thinking_chars: usize,
  output_chars: usize,
}

pub struct Studio {
  grounding: Entity<GroundingPanel>,
  grounding_mode: bool,
  session: Session,
  path: PathBuf,
  prompt: Entity<TextareaState>,
  endpoint: Entity<InputState>,
  model: Entity<InputState>,
  max_tokens: Entity<InputState>,
  source: Entity<TextareaState>,
  preview: Rc<RefCell<Preview>>,
  running: Option<Running>,
  raw: String,
  last_preview: Instant,
  show_source: bool,
  choosing_file: bool,
  notice: String,
  error: Option<String>,
  _focus_subscriptions: Vec<Subscription>,
}

impl Studio {
  pub fn new(
    session: Session,
    grounding: GroundingSession,
    path: PathBuf,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let grounding_path = path.with_extension("grounding.json");
    let grounding = cx.new(|cx| GroundingPanel::new(grounding, grounding_path, window, cx));
    let prompt = cx.new(|cx| {
      let mut state =
        TextareaState::new(window, cx).placeholder("输入设计需求，或描述对当前版本的修改……");
      state.set_value(session.draft.clone(), window, cx);
      state
    });
    let endpoint = input(&session.settings.endpoint, window, cx);
    let model = input(&session.settings.model, window, cx);
    let max_tokens = input(&session.settings.max_tokens.to_string(), window, cx);
    let source = cx.new(|cx| TextareaState::new(window, cx));
    let preview = Preview::new();
    preview
      .borrow_mut()
      .set_document(session.current().map(|revision| revision.html.as_str()));
    let focus_handles = [
      prompt.read(cx).focus_handle(cx),
      endpoint.read(cx).focus_handle(cx),
      model.read(cx).focus_handle(cx),
      max_tokens.read(cx).focus_handle(cx),
      source.read(cx).focus_handle(cx),
    ];
    let mut focus_subscriptions: Vec<Subscription> = focus_handles
      .into_iter()
      .map(|handle| cx.on_focus_in(&handle, window, |this, _, cx| this.focus_editor(cx)))
      .collect();
    focus_subscriptions.push(cx.observe(&grounding, |_, _, cx| cx.notify()));
    let studio = Self {
      grounding,
      grounding_mode: false,
      session,
      path,
      prompt,
      endpoint,
      model,
      max_tokens,
      source,
      preview,
      running: None,
      raw: String::new(),
      last_preview: Instant::now(),
      show_source: false,
      choosing_file: false,
      notice: "就绪 · 输入提示词，开始生成设计".into(),
      error: None,
      _focus_subscriptions: focus_subscriptions,
    };
    let weak = cx.weak_entity();
    window.on_window_should_close(cx, move |_, cx| {
      weak
        .update(cx, |this, cx| {
          if let Err(error) = this.grounding.update(cx, |panel, cx| panel.save(cx)) {
            this.grounding.update(cx, |panel, _| panel.hide());
            this.grounding_mode = false;
            this.error = Some(format!("保存定位会话失败: {error:#}；请修复后再关闭"));
            cx.notify();
            return false;
          }
          this.capture_draft(cx);
          if let Err(error) = this.session.save(&this.path) {
            eprintln!("保存会话失败: {error:#}");
            this.error = Some(format!("保存会话失败: {error:#}；请先修复路径或权限再关闭"));
            cx.notify();
            return false;
          }
          this.running = None;
          true
        })
        .unwrap_or(true)
    });
    cx.spawn_in(window, async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(100))
          .await;
        if this
          .update_in(cx, |this, window, cx| this.poll(window, cx))
          .is_err()
        {
          break;
        }
      }
    })
    .detach();
    studio
  }

  fn focus_editor(&mut self, cx: &mut Context<Self>) {
    if let Err(error) = self.preview.borrow().focus_editor() {
      self.error = Some(format!("恢复编辑器输入焦点失败: {error:#}"));
      cx.notify();
    }
  }

  fn capture_draft(&mut self, cx: &App) {
    self.session.draft = self.prompt.read(cx).value().to_string();
    let mut settings = self.session.settings.clone();
    settings.endpoint = self.endpoint.read(cx).value().trim().to_owned();
    settings.model = self.model.read(cx).value().trim().to_owned();
    if let Ok(max_tokens) = self.max_tokens.read(cx).value().parse() {
      settings.max_tokens = max_tokens;
      if settings.validate().is_ok() {
        self.session.settings = settings;
      }
    }
  }

  fn persist(&mut self, cx: &App) {
    self.capture_draft(cx);
    if let Err(error) = self.session.save(&self.path) {
      self.error = Some(format!("会话保存失败，内存中的版本仍保留: {error:#}"));
    }
  }

  fn generate(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    if self.running.is_some() {
      return;
    }
    self.error = None;
    let result = (|| -> anyhow::Result<Running> {
      let mut settings = self.session.settings.clone();
      settings.endpoint = self.endpoint.read(cx).value().trim().to_owned();
      settings.model = self.model.read(cx).value().trim().to_owned();
      settings.max_tokens = self
        .max_tokens
        .read(cx)
        .value()
        .parse()
        .map_err(|_| anyhow::anyhow!("输出上限必须是整数"))?;
      settings.validate()?;
      self.session.settings = settings;
      let prompt = self.prompt.read(cx).value().trim().to_owned();
      let request = self.session.request(&prompt)?;
      let job = generation::start(
        self.session.settings.endpoint.clone(),
        request,
        std::env::var("MAI_UI_API_KEY").ok(),
      )?;
      Ok(Running {
        job,
        prompt,
        started: Instant::now(),
        connected: false,
        thinking_chars: 0,
        output_chars: 0,
      })
    })();
    match result {
      Ok(running) => {
        self.raw.clear();
        self.notice = "正在连接 Ollama，首次加载模型可能较慢……".into();
        self.running = Some(running);
        self.last_preview = Instant::now();
        self.persist(cx);
      }
      Err(error) => self.error = Some(format!("{error:#}")),
    }
    cx.notify();
  }

  fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.running.is_none() {
      return;
    }
    let mut events = Vec::new();
    let mut disconnected = false;
    if let Some(running) = &self.running {
      for _ in 0 .. 1024 {
        match running.job.events.try_recv() {
          Ok(event) => events.push(event),
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => {
            disconnected = true;
            break;
          }
        }
      }
    }
    let mut changed = false;
    for event in events {
      match event {
        Event::Connected => {
          if let Some(running) = &mut self.running {
            running.connected = true;
          }
        }
        Event::Delta {
          content,
          thinking_chars,
        } => {
          if let Some(running) = &mut self.running {
            running.output_chars += content.chars().count();
            running.thinking_chars += thinking_chars;
            self.raw.push_str(&content);
            changed |= !content.is_empty();
          }
        }
        Event::Finished(result) => {
          if let Some(running) = self.running.take() {
            match result {
              Ok(output) => match self
                .session
                .commit(running.prompt, output.content, output.tokens)
              {
                Ok(()) => {
                  self.notice = format!(
                    "版本 V{} 已完成 · {:.1}s · {} 个输出字符{}",
                    self.session.revisions.len(),
                    running.started.elapsed().as_secs_f64(),
                    running.output_chars,
                    output
                      .tokens
                      .map(|tokens| format!(" · {tokens} tokens"))
                      .unwrap_or_default()
                  );
                  self.persist(cx);
                }
                Err(error) => self.error = Some(format!("保存新版本失败: {error:#}")),
              },
              Err(error) => {
                self.error = Some(format!("生成失败: {error:#}"));
                self.notice = "生成未完成，已恢复上一个稳定版本；提示词可修改后重试".into();
              }
            }
            self.restore_preview();
          }
        }
      }
    }
    if disconnected && self.running.is_some() {
      self.running = None;
      self.error = Some("生成线程意外退出；已恢复上一个稳定版本".into());
      self.restore_preview();
    }
    if changed && self.show_source {
      self.update_source(window, cx);
    }
    if self.running.is_some()
      && changed
      && self.last_preview.elapsed() >= Duration::from_millis(900)
      && document::can_preview(&self.raw)
    {
      self.preview.borrow_mut().set_document(Some(&self.raw));
      self.last_preview = Instant::now();
    }
    cx.notify();
  }

  fn restore_preview(&mut self) {
    self.preview.borrow_mut().set_document(
      self
        .session
        .current()
        .map(|revision| revision.html.as_str()),
    );
  }

  fn update_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let value = if self.raw.is_empty() {
      self
        .session
        .current()
        .map(|revision| revision.html.clone())
        .unwrap_or_default()
    } else {
      self.raw.clone()
    };
    self
      .source
      .update(cx, |state, cx| state.set_value(value, window, cx));
  }

  fn select(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
    if self.running.is_some() {
      return;
    }
    if let Err(error) = self.session.select(index) {
      self.error = Some(format!("{error:#}"));
      return;
    }
    self.error = None;
    self.raw.clear();
    self.restore_preview();
    self.update_source(window, cx);
    self.notice = format!(
      "当前基线：{}；继续输入提示词将创建新分支，不删除已有版本",
      version(self.session.active)
    );
    self.persist(cx);
    cx.notify();
  }

  fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.choosing_file || self.running.is_some() {
      return;
    }
    let Some(revision) = self.session.current() else {
      return;
    };
    let html = document::preview(&revision.html);
    let name = format!(
      "mai-ui-v{}.html",
      self.session.active.unwrap_or_default() + 1
    );
    self.choosing_file = true;
    cx.spawn_in(window, async move |this, cx| {
      let file = rfd::AsyncFileDialog::new()
        .set_title("导出当前版本 · 离线 HTML")
        .add_filter("HTML", &["html"])
        .set_file_name(name)
        .save_file()
        .await;
      let result = if let Some(file) = file {
        let path = file.path().to_owned();
        Some(
          cx.background_executor()
            .spawn(async move { std::fs::write(&path, html).map(|_| path) })
            .await,
        )
      } else {
        None
      };
      let _ = this.update(cx, |this, cx| {
        this.choosing_file = false;
        match result {
          Some(Ok(path)) => this.notice = format!("已导出当前版本：{}", path.display()),
          Some(Err(error)) => this.error = Some(format!("导出失败: {error}")),
          None => {}
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn progress(&self) -> String {
    let Some(running) = &self.running else {
      return self.notice.clone();
    };
    let stage = if !running.connected {
      "连接 / 加载模型"
    } else if running.output_chars > 0 {
      "生成 HTML/CSS"
    } else if running.thinking_chars > 0 {
      "模型思考"
    } else {
      "等待首个输出"
    };
    format!(
      "{stage} · {:.0}s · {} 个输出字符 · {} 个思考字符（总量未知）",
      running.started.elapsed().as_secs_f64(),
      running.output_chars,
      running.thinking_chars
    )
  }

  fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let busy = self.running.is_some();
    let parent = self.session.current().and_then(|revision| revision.parent);
    div()
      .id("sidebar")
      .v_flex()
      .w(px(360.))
      .h_full()
      .flex_shrink_0()
      .overflow_y_scroll()
      .bg(rgb(0x151e2d))
      .p_4()
      .gap_3()
      .child(
        div()
          .text_xl()
          .font_weight(FontWeight::BOLD)
          .child("MAI-UI Studio"),
      )
      .child(label("GPUI × Ollama · 通用界面设计工作台"))
      .child(label("OpenAI 兼容接口"))
      .child(Input::new(&self.endpoint).disabled(busy))
      .child(label("模型"))
      .child(Input::new(&self.model).disabled(busy))
      .child(label("最大输出 tokens"))
      .child(Input::new(&self.max_tokens).disabled(busy))
      .child(div().mt_2().font_weight(FontWeight::BOLD).child(format!(
        "下一步提示词 · 基于 {}",
        version(self.session.active)
      )))
      .child(Textarea::new(&self.prompt).h(px(160.)).disabled(busy))
      .child(
        div()
          .h_flex()
          .gap_2()
          .child(
            Button::new("generate")
              .primary()
              .label("生成 / 继续修改")
              .disabled(busy)
              .on_click(cx.listener(|this, _, window, cx| this.generate(window, cx))),
          )
          .child(
            Button::new("cancel")
              .label("取消")
              .disabled(!busy)
              .on_click(cx.listener(|this, _, window, cx| {
                this.running = None;
                this.restore_preview();
                this.notice = "已取消并恢复稳定版本；提示词保留，可直接重试".into();
                this.update_source(window, cx);
                cx.notify();
              })),
          ),
      )
      .child(
        div()
          .h_flex()
          .gap_2()
          .child(
            Button::new("save-prompt")
              .label("保存提示词")
              .disabled(busy)
              .on_click(cx.listener(|this, _, _, cx| {
                let prompt = this.prompt.read(cx).value().to_string();
                if prompt.trim().is_empty() {
                  this.error = Some("提示词不能为空".into());
                } else {
                  this.session.remember_prompt(&prompt);
                  this.notice = "提示词已保存，可从下方一键复用".into();
                  this.persist(cx);
                }
                cx.notify();
              })),
          )
          .child(
            Button::new("back")
              .label("回退一步")
              .disabled(busy || self.session.active.is_none())
              .on_click(cx.listener(move |this, _, window, cx| this.select(parent, window, cx))),
          ),
      )
      .child(
        div()
          .mt_3()
          .font_weight(FontWeight::BOLD)
          .child("版本历史 · 点击切换基线"),
      )
      .child(
        Button::new("empty-baseline")
          .label("空白基线 / 新方向")
          .disabled(busy)
          .on_click(cx.listener(|this, _, window, cx| this.select(None, window, cx))),
      )
      .children(
        self
          .session
          .revisions
          .iter()
          .enumerate()
          .rev()
          .map(|(index, revision)| {
            let active = self.session.active == Some(index);
            div()
              .v_flex()
              .gap_1()
              .p_2()
              .rounded_md()
              .bg(if active { rgb(0x1e3a52) } else { rgb(0x1b2738) })
              .child(
                Button::new(("revision", index))
                  .label(format!(
                    "{}V{} ← {}",
                    if active { "● " } else { "" },
                    index + 1,
                    version(revision.parent)
                  ))
                  .disabled(busy)
                  .on_click(
                    cx.listener(move |this, _, window, cx| this.select(Some(index), window, cx)),
                  ),
              )
              .child(div().text_sm().child(short(&revision.prompt, 55)))
          }),
      )
      .child(
        div()
          .mt_3()
          .font_weight(FontWeight::BOLD)
          .child("提示词库 · 点击复用到编辑框"),
      )
      .children(
        self
          .session
          .prompts
          .iter()
          .enumerate()
          .rev()
          .map(|(index, prompt)| {
            let prompt = prompt.clone();
            Button::new(("prompt", index))
              .label(short(&prompt, 18))
              .disabled(busy)
              .on_click(cx.listener(move |this, _, window, cx| {
                this
                  .prompt
                  .update(cx, |state, cx| state.set_value(prompt.clone(), window, cx));
                this.persist(cx);
                this.notice = "已复用提示词；可编辑后再次生成".into();
                cx.notify();
              }))
          }),
      )
      .child(label("完成版本自动保存；回退和新分支不会覆盖旧版本。"))
  }
}

impl Studio {
  fn design(&self, cx: &mut Context<Self>) -> AnyElement {
    let busy = self.running.is_some();
    let preview_error = self.preview.borrow().error.clone();
    let content = if self.show_source {
      Textarea::new(&self.source)
        .readonly(true)
        .h_full()
        .into_any_element()
    } else if let Some(error) = preview_error {
      div().size_full().p_6().child(error).into_any_element()
    } else {
      preview::element(self.preview.clone()).into_any_element()
    };
    div()
      .h_flex()
      .size_full()
      .capture_any_mouse_down(cx.listener(|this, _, _, cx| this.focus_editor(cx)))
      .bg(rgb(0x0c1320))
      .text_color(rgb(0xe2e8f0))
      .child(self.sidebar(cx))
      .child(
        div()
          .v_flex()
          .flex_1()
          .min_w_0()
          .h_full()
          .child(
            div()
              .h_flex()
              .p_4()
              .gap_3()
              .items_center()
              .child(
                div()
                  .flex_1()
                  .text_lg()
                  .font_weight(FontWeight::BOLD)
                  .child(format!(
                    "{} · {}",
                    version(self.session.active),
                    if busy {
                      "实时生成预览"
                    } else {
                      "稳定版本预览"
                    }
                  )),
              )
              .child(
                Button::new("toggle-source")
                  .label(if self.show_source {
                    "显示预览"
                  } else {
                    "查看源码"
                  })
                  .on_click(cx.listener(|this, _, window, cx| {
                    this.show_source = !this.show_source;
                    if this.show_source {
                      this.preview.borrow_mut().hide();
                    }
                    this.update_source(window, cx);
                    cx.notify();
                  })),
              )
              .child(
                Button::new("export")
                  .primary()
                  .label("下载 HTML")
                  .disabled(busy || self.choosing_file || self.session.current().is_none())
                  .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
              ),
          )
          .child(
            div()
              .px_4()
              .pb_3()
              .text_sm()
              .text_color(rgb(0x93c5fd))
              .child(self.progress()),
          )
          .children(self.error.as_ref().map(|error| {
            div()
              .mx_4()
              .mb_3()
              .p_3()
              .rounded_md()
              .bg(rgb(0x47252d))
              .text_color(rgb(0xfecaca))
              .child(error.clone())
          }))
          .child(div().flex_1().min_h_0().overflow_hidden().child(content))
          .child(
            div()
              .px_4()
              .py_2()
              .text_xs()
              .text_color(rgb(0x94a3b8))
              .child(
                "视觉原型，不实现真实业务功能 · 流式预览可能暂时不完整 · 脚本与外部资源已禁用",
              ),
          ),
      )
      .into_any_element()
  }
}

impl Render for Studio {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let busy = self.running.is_some() || self.choosing_file || self.grounding.read(cx).is_busy();
    let content = if self.grounding_mode {
      self.grounding.clone().into_any_element()
    } else {
      self.design(cx)
    };
    div()
      .v_flex()
      .size_full()
      .bg(rgb(0x0c1320))
      .text_color(rgb(0xe2e8f0))
      .child(
        div()
          .h_flex()
          .p_2()
          .gap_3()
          .child(
            Button::new("design-mode")
              .label(if self.grounding_mode {
                "界面设计"
              } else {
                "● 界面设计"
              })
              .disabled(busy)
              .on_click(cx.listener(|this, _, _, cx| {
                this.grounding.update(cx, |panel, _| panel.hide());
                this.grounding_mode = false;
                cx.notify();
              })),
          )
          .child(
            Button::new("grounding-mode")
              .label(if self.grounding_mode {
                "● 截图定位"
              } else {
                "截图定位"
              })
              .disabled(busy)
              .on_click(cx.listener(|this, _, _, cx| {
                this.preview.borrow_mut().hide();
                this.grounding_mode = true;
                cx.notify();
              })),
          )
          .child(label(
            "MAI-UI 视觉代理用于截图定位；HTML 设计请选用支持代码生成的模型。",
          )),
      )
      .child(div().flex_1().min_h_0().child(content))
  }
}

fn input(value: &str, window: &mut Window, cx: &mut Context<Studio>) -> Entity<InputState> {
  cx.new(|cx| {
    let mut state = InputState::new(window, cx);
    state.set_value(value.to_owned(), window, cx);
    state
  })
}

fn label(text: &str) -> impl IntoElement {
  div()
    .text_sm()
    .text_color(rgb(0x94a3b8))
    .child(text.to_owned())
}

fn version(index: Option<usize>) -> String {
  index
    .map(|index| format!("V{}", index + 1))
    .unwrap_or_else(|| "空白".into())
}

fn short(text: &str, limit: usize) -> String {
  let mut result: String = text.chars().take(limit).collect();
  if text.chars().count() > limit {
    result.push('…');
  }
  result
}

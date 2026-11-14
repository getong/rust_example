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
  generation::{self, Event, Job, ResponseFormat},
  grounding::{GroundingSession, Screenshot},
  session::Settings,
};

use crate::preview::{self, Preview};

struct Running {
  job: Job,
  instruction: String,
  started: Instant,
  connected: bool,
  output_chars: usize,
  thinking_chars: usize,
}

pub struct GroundingPanel {
  session: GroundingSession,
  path: PathBuf,
  endpoint: Entity<InputState>,
  model: Entity<InputState>,
  max_tokens: Entity<InputState>,
  instruction: Entity<TextareaState>,
  response: Entity<TextareaState>,
  preview: Rc<RefCell<Preview>>,
  running: Option<Running>,
  raw: String,
  show_response: bool,
  choosing_file: bool,
  notice: String,
  error: Option<String>,
  _focus_subscriptions: Vec<Subscription>,
}

impl GroundingPanel {
  pub fn new(
    session: GroundingSession,
    path: PathBuf,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let endpoint = field(&session.settings.endpoint, window, cx);
    let model = field(&session.settings.model, window, cx);
    let max_tokens = field(&session.settings.max_tokens.to_string(), window, cx);
    let instruction = cx.new(|cx| {
      let mut state = TextareaState::new(window, cx).placeholder("例如：定位截图右上角的设置按钮");
      state.set_value(session.draft.clone(), window, cx);
      state
    });
    let response = cx.new(|cx| {
      let mut state = TextareaState::new(window, cx);
      state.set_value(
        session
          .selected()
          .map(|step| step.response.clone())
          .unwrap_or_default(),
        window,
        cx,
      );
      state
    });
    let handles = [
      endpoint.read(cx).focus_handle(cx),
      model.read(cx).focus_handle(cx),
      max_tokens.read(cx).focus_handle(cx),
      instruction.read(cx).focus_handle(cx),
      response.read(cx).focus_handle(cx),
    ];
    let subscriptions = handles
      .into_iter()
      .map(|handle| cx.on_focus_in(&handle, window, |this, _, cx| this.focus_editor(cx)))
      .collect();
    let panel = Self {
      session,
      path,
      endpoint,
      model,
      max_tokens,
      instruction,
      response,
      preview: Preview::new(),
      running: None,
      raw: String::new(),
      show_response: false,
      choosing_file: false,
      notice: "导入截图并描述目标；只定位，不操作真实设备".into(),
      error: None,
      _focus_subscriptions: subscriptions,
    };
    panel.restore_preview();
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
    panel
  }

  pub fn is_busy(&self) -> bool {
    self.running.is_some() || self.choosing_file
  }

  pub fn hide(&mut self) {
    self.preview.borrow_mut().hide();
  }

  pub fn session(&self) -> &GroundingSession {
    &self.session
  }

  pub fn return_input_focus(&self) -> anyhow::Result<()> {
    self.preview.borrow().focus_editor()
  }

  fn focus_editor(&mut self, cx: &mut Context<Self>) {
    if let Err(error) = self.preview.borrow().focus_editor() {
      self.error = Some(format!("恢复输入焦点失败: {error:#}"));
      cx.notify();
    }
  }

  fn settings(&self, cx: &App) -> anyhow::Result<Settings> {
    let settings = Settings {
      endpoint: self.endpoint.read(cx).value().trim().to_owned(),
      model: self.model.read(cx).value().trim().to_owned(),
      max_tokens: self
        .max_tokens
        .read(cx)
        .value()
        .parse()
        .map_err(|_| anyhow::anyhow!("输出上限必须是整数"))?,
    };
    settings.validate()?;
    Ok(settings)
  }

  pub fn save(&mut self, cx: &App) -> anyhow::Result<()> {
    self.session.draft = self.instruction.read(cx).value().to_string();
    if let Ok(settings) = self.settings(cx) {
      self.session.settings = settings;
    }
    self.session.save(&self.path)
  }

  fn persist(&mut self, cx: &App) {
    if let Err(error) = self.save(cx) {
      self.error = Some(format!("保存定位会话失败，内存记录仍保留: {error:#}"));
    }
  }

  fn restore_preview(&self) {
    let html = self
      .session
      .screenshot()
      .map(|screenshot| screenshot.preview(self.session.selected().map(|step| &step.result)))
      .unwrap_or_else(|| {
        "<!doctype html><html><head><style>body{background:#101827;color:#cbd5e1;font:18px \
         system-ui;padding:64px;line-height:2}</style></head><body><h1>截图定位 · MAI-UI \
         Grounding</h1><p>导入 PNG / JPEG \
         截图，输入要寻找的元素。<br>模型返回的位置会以红色圆点标注在截图上。\
         <br>截图会发送到所配置的模型接口，请勿上传敏感内容。</p></body></html>"
          .into()
      });
    self.preview.borrow_mut().set_document(Some(&html));
  }

  fn update_response(&self, window: &mut Window, cx: &mut Context<Self>) {
    let value = if self.raw.is_empty() && self.running.is_none() {
      self
        .session
        .selected()
        .map(|step| step.response.clone())
        .unwrap_or_default()
    } else {
      self.raw.clone()
    };
    self
      .response
      .update(cx, |state, cx| state.set_value(value, window, cx));
  }

  fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.is_busy() {
      return;
    }
    self.choosing_file = true;
    self.error = None;
    cx.spawn_in(window, async move |this, cx| {
      let file = rfd::AsyncFileDialog::new()
        .set_title("选择待定位截图（将发送到模型接口）")
        .add_filter("截图", &["png", "jpg", "jpeg"])
        .pick_file()
        .await;
      let result = if let Some(file) = file {
        let path = file.path().to_owned();
        Some(
          cx.background_executor()
            .spawn(async move { Screenshot::load(&path) })
            .await,
        )
      } else {
        None
      };
      let _ = this.update_in(cx, |this, window, cx| {
        this.choosing_file = false;
        match result {
          Some(Ok(screenshot)) => {
            this.session.import(screenshot);
            this.raw.clear();
            this.restore_preview();
            this.update_response(window, cx);
            this.notice = "截图已导入，请输入目标后点击「定位元素」".into();
            this.persist(cx);
          }
          Some(Err(error)) => this.error = Some(format!("导入截图失败: {error:#}")),
          None => {}
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn locate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.is_busy() {
      return;
    }
    self.error = None;
    let result = (|| -> anyhow::Result<Running> {
      let settings = self.settings(cx)?;
      let instruction = self.instruction.read(cx).value().trim().to_owned();
      let screenshot = self
        .session
        .screenshot()
        .ok_or_else(|| anyhow::anyhow!("请先导入截图"))?;
      let request = screenshot.request(&settings, &instruction)?;
      let job = generation::start_with_format(
        settings.endpoint.clone(),
        request,
        std::env::var("MAI_UI_API_KEY").ok(),
        ResponseFormat::Grounding,
      )?;
      self.session.settings = settings;
      Ok(Running {
        job,
        instruction,
        started: Instant::now(),
        connected: false,
        output_chars: 0,
        thinking_chars: 0,
      })
    })();
    match result {
      Ok(running) => {
        self.running = Some(running);
        self.raw.clear();
        self.update_response(window, cx);
        self.persist(cx);
      }
      Err(error) => self.error = Some(format!("{error:#}")),
    }
    cx.notify();
  }

  fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Some(running) = &self.running else {
      return;
    };
    let mut events = Vec::new();
    let mut disconnected = false;
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
            let result = result.and_then(|output| {
              self
                .session
                .commit(running.instruction, output.content, output.tokens)
            });
            match result {
              Ok(()) => {
                self.notice = format!(
                  "定位完成 · {:.1}s · {} 个输出字符",
                  running.started.elapsed().as_secs_f64(),
                  running.output_chars
                );
                self.persist(cx);
              }
              Err(error) => {
                self.error = Some(format!("定位失败: {error:#}"));
                self.notice = "未新增记录，原定位结果保留；可修改指令重试".into();
              }
            }
            self.restore_preview();
            changed = true;
          }
        }
      }
    }
    if disconnected && self.running.is_some() {
      self.running = None;
      self.error = Some("定位线程意外退出，原记录保留".into());
    }
    if changed {
      self.update_response(window, cx);
    }
    cx.notify();
  }

  fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.is_busy() {
      return;
    }
    let Some(screenshot) = self.session.screenshot() else {
      return;
    };
    let html =
      document::preview(&screenshot.preview(self.session.selected().map(|step| &step.result)));
    self.choosing_file = true;
    cx.spawn_in(window, async move |this, cx| {
      let file = rfd::AsyncFileDialog::new()
        .set_title("导出截图与定位标记")
        .add_filter("HTML", &["html"])
        .set_file_name("mai-ui-grounding.html")
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
          Some(Ok(path)) => this.notice = format!("已导出标注截图：{}", path.display()),
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
    if let Some(running) = &self.running {
      format!(
        "{} · {:.0}s · {} 个输出字符 · {} 个思考字符",
        if running.connected {
          "分析截图 / 输出定位"
        } else {
          "连接 / 加载视觉模型"
        },
        running.started.elapsed().as_secs_f64(),
        running.output_chars,
        running.thinking_chars
      )
    } else {
      self.notice.clone()
    }
  }
}

impl Render for GroundingPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let busy = self.is_busy();
    let preview_error = self.preview.borrow().error.clone();
    let content = if self.show_response {
      Textarea::new(&self.response)
        .readonly(true)
        .h_full()
        .into_any_element()
    } else if let Some(error) = preview_error {
      div().p_6().child(error).into_any_element()
    } else {
      preview::element(self.preview.clone()).into_any_element()
    };
    div()
      .h_flex()
      .size_full()
      .capture_any_mouse_down(cx.listener(|this, _, _, cx| this.focus_editor(cx)))
      .child(
        div()
          .id("grounding-sidebar")
          .v_flex()
          .w(px(360.))
          .h_full()
          .flex_shrink_0()
          .overflow_y_scroll()
          .p_4()
          .gap_3()
          .bg(rgb(0x151e2d))
          .child(
            div()
              .text_xl()
              .font_weight(FontWeight::BOLD)
              .child("MAI-UI 截图定位"),
          )
          .child(note("移植 Python MAIGroundingAgent 工作流"))
          .child(note("OpenAI 兼容接口（需支持视觉输入）"))
          .child(Input::new(&self.endpoint).disabled(busy))
          .child(note("视觉模型"))
          .child(Input::new(&self.model).disabled(busy))
          .child(note("最大输出 tokens"))
          .child(Input::new(&self.max_tokens).disabled(busy))
          .child(
            Button::new("import-screenshot")
              .label("导入截图 PNG / JPEG")
              .disabled(busy)
              .on_click(cx.listener(|this, _, window, cx| this.import(window, cx))),
          )
          .children(self.session.screenshot().map(|screenshot| {
            note(&format!(
              "{} · {} × {}",
              screenshot.name,
              screenshot.dimensions()[0],
              screenshot.dimensions()[1]
            ))
          }))
          .child(note("定位指令"))
          .child(Textarea::new(&self.instruction).h(px(120.)).disabled(busy))
          .child(
            div()
              .h_flex()
              .gap_2()
              .child(
                Button::new("locate")
                  .primary()
                  .label("定位元素")
                  .disabled(busy || self.session.screenshot().is_none())
                  .on_click(cx.listener(|this, _, window, cx| this.locate(window, cx))),
              )
              .child(
                Button::new("cancel-grounding")
                  .label("取消")
                  .disabled(self.running.is_none())
                  .on_click(cx.listener(|this, _, _, cx| {
                    this.running = None;
                    this.notice = "已取消定位；截图和历史结果保留".into();
                    cx.notify();
                  })),
              ),
          )
          .child(note(
            "截图会发送到上述接口并保存在本地会话中，请勿导入敏感内容。",
          ))
          .child(
            div()
              .mt_3()
              .font_weight(FontWeight::BOLD)
              .child("定位历史 · 点击恢复截图与结果"),
          )
          .children(
            self
              .session
              .steps()
              .iter()
              .enumerate()
              .rev()
              .map(|(index, step)| {
                let title: String = step.instruction.chars().take(20).collect();
                Button::new(("grounding-step", index))
                  .label(format!("#{} {title}", index + 1))
                  .disabled(busy)
                  .on_click(cx.listener(move |this, _, window, cx| {
                    if let Err(error) = this.session.select(index) {
                      this.error = Some(format!("{error:#}"));
                    } else {
                      this.raw.clear();
                      let instruction = this
                        .session
                        .selected()
                        .map(|step| step.instruction.clone())
                        .unwrap_or_default();
                      this
                        .instruction
                        .update(cx, |state, cx| state.set_value(instruction, window, cx));
                      this.restore_preview();
                      this.update_response(window, cx);
                      this.error = None;
                      this.notice = format!("已恢复定位记录 #{}", index + 1);
                      this.persist(cx);
                    }
                    cx.notify();
                  }))
              }),
          ),
      )
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
                  .child("截图与定位结果"),
              )
              .child(
                Button::new("toggle-grounding-response")
                  .label(if self.show_response {
                    "显示截图"
                  } else {
                    "原始响应"
                  })
                  .on_click(cx.listener(|this, _, window, cx| {
                    this.show_response = !this.show_response;
                    if this.show_response {
                      this.hide();
                    }
                    this.update_response(window, cx);
                    cx.notify();
                  })),
              )
              .child(
                Button::new("export-grounding")
                  .label("导出标注 HTML")
                  .disabled(busy || self.session.screenshot().is_none())
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
              .bg(rgb(0x47252d))
              .text_color(rgb(0xfecaca))
              .child(error.clone())
          }))
          .children(self.session.selected().zip(self.session.screenshot()).map(
            |(step, screenshot)| {
              let [point_x, point_y] = step.result.pixels(screenshot.dimensions());
              div()
                .px_4()
                .pb_3()
                .v_flex()
                .gap_2()
                .child(format!(
                  "归一化坐标（0–999）：{:?} · 原图像素：[{point_x}, {point_y}]{}",
                  step.result.coordinate,
                  step
                    .tokens
                    .map(|tokens| format!(" · {tokens} tokens"))
                    .unwrap_or_default()
                ))
                .children(
                  step
                    .result
                    .explanation
                    .as_ref()
                    .map(|explanation| note(explanation)),
                )
            },
          ))
          .child(div().flex_1().min_h_0().overflow_hidden().child(content))
          .child(div().px_4().py_2().child(note(
            "仅显示模型预测位置，不点击屏幕、不执行系统操作；预测可能不准确。",
          ))),
      )
  }
}

fn field(value: &str, window: &mut Window, cx: &mut Context<GroundingPanel>) -> Entity<InputState> {
  cx.new(|cx| {
    let mut state = InputState::new(window, cx);
    state.set_value(value.to_owned(), window, cx);
    state
  })
}

fn note(text: &str) -> Div {
  div()
    .text_sm()
    .text_color(rgb(0x94a3b8))
    .child(text.to_owned())
}

use gpui_kit::{
  component::{
    button::*,
    input::{Textarea, TextareaState},
    *,
  },
  prelude::FluentBuilder,
  *,
};
use gpui_mai_ui_client::projects::{ProjectData, ProjectStore};

use crate::{app::Studio, new_project::NewProject};

pub struct Workspace {
  projects: ProjectStore,
  studio: Entity<Studio>,
  new_project_window: Option<AnyWindowHandle>,
  prompts: Entity<TextareaState>,
  show_projects: bool,
  show_prompts: bool,
  error: Option<String>,
  _studio_subscription: Subscription,
  _focus_subscriptions: Vec<Subscription>,
}

fn studio(
  mut project: ProjectData,
  window: &mut Window,
  cx: &mut Context<Workspace>,
) -> Entity<Studio> {
  if let Ok(endpoint) = std::env::var("MAI_UI_ENDPOINT") {
    project.session.settings.endpoint = endpoint.clone();
    project.grounding.settings.endpoint = endpoint;
  }
  if let Ok(model) = std::env::var("MAI_UI_MODEL") {
    project.session.settings.model = model.clone();
    project.grounding.settings.model = model;
  }
  cx.new(|cx| Studio::new(project.session, project.grounding, project.path, window, cx))
}

impl Workspace {
  pub fn new(
    projects: ProjectStore,
    project: ProjectData,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let studio = studio(project, window, cx);
    let subscription = cx.observe(&studio, |_, _, cx| cx.notify());
    let prompts = cx.new(|cx| TextareaState::new(window, cx));
    let handles = [prompts.read(cx).focus_handle(cx)];
    let focus_subscriptions = handles
      .into_iter()
      .map(|handle| cx.on_focus_in(&handle, window, |this, _, cx| this.return_input_focus(cx)))
      .collect();
    let weak = cx.weak_entity();
    window.on_window_should_close(cx, move |_, cx| {
      weak
        .update(cx, |this, cx| {
          if let Err(error) = this.save(cx) {
            this.error = Some(format!(
              "保存当前工程失败: {error:#}；请修复路径或权限后再关闭"
            ));
            cx.notify();
            false
          } else {
            if let Some(handle) = this.new_project_window.take() {
              let _ = cx.update_window(handle, |_, window, _| window.remove_window());
            }
            true
          }
        })
        .unwrap_or(true)
    });
    Self {
      projects,
      studio,
      new_project_window: None,
      prompts,
      show_projects: true,
      show_prompts: false,
      error: None,
      _studio_subscription: subscription,
      _focus_subscriptions: focus_subscriptions,
    }
  }

  fn return_input_focus(&mut self, cx: &mut Context<Self>) {
    if let Err(error) = self.studio.read(cx).return_input_focus(cx) {
      self.error = Some(format!("恢复工程编辑器输入焦点失败: {error:#}"));
      cx.notify();
    }
    cx.refresh_windows();
  }

  fn save(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
    self.studio.update(cx, |studio, cx| studio.save(cx))
  }

  fn update_prompts(&self, window: &mut Window, cx: &mut Context<Self>) {
    let report = format!(
      "工程：{}\n\n{}",
      self.projects.active().name,
      self.studio.read(cx).prompt_report(cx)
    );
    self
      .prompts
      .update(cx, |state, cx| state.set_value(report, window, cx));
  }

  fn activate(&mut self, project: ProjectData, window: &mut Window, cx: &mut Context<Self>) {
    self
      .studio
      .update(cx, |studio, cx| studio.hide_previews(cx));
    let studio = studio(project, window, cx);
    self._studio_subscription = cx.observe(&studio, |_, _, cx| cx.notify());
    self.studio = studio;
    self.show_projects = false;
    self.error = None;
    if self.show_prompts {
      self.update_prompts(window, cx);
    }
    cx.notify();
  }

  fn open_new_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.studio.read(cx).is_busy(cx) {
      return;
    }
    if let Some(handle) = self.new_project_window
      && cx
        .update_window(handle, |_, window, _| window.activate_window())
        .is_ok()
    {
      return;
    }
    let parent = window.window_handle();
    let workspace = cx.weak_entity();
    let options = WindowOptions {
      kind: WindowKind::Dialog,
      titlebar: Some(TitlebarOptions {
        title: Some("新建工程".into()),
        ..Default::default()
      }),
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(480.), px(260.)),
        cx,
      ))),
      is_resizable: false,
      is_minimizable: false,
      ..Default::default()
    };
    match gpui_kit::open_window(options, cx, move |window, cx| {
      cx.new(|cx| NewProject::new(workspace, parent, window, cx))
    }) {
      Ok((handle, _)) => self.new_project_window = Some(handle),
      Err(error) => {
        self.error = Some(format!("打开工程命名窗口失败: {error:#}"));
        cx.notify();
      }
    }
  }

  pub(crate) fn create_named(
    &mut self,
    name: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> anyhow::Result<()> {
    anyhow::ensure!(
      !self.studio.read(cx).is_busy(cx),
      "请等待当前任务完成后再新建工程"
    );
    self.save(cx)?;
    let (settings, grounding_settings) = self.studio.read(cx).settings(cx);
    let project = self.projects.create(name, settings, grounding_settings)?;
    self.show_prompts = false;
    self.activate(project, window, cx);
    Ok(())
  }

  fn select(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
    if self.studio.read(cx).is_busy(cx) || id == self.projects.active().id {
      return;
    }
    let result = self.save(cx).and_then(|()| self.projects.select(id));
    match result {
      Ok(project) => self.activate(project, window, cx),
      Err(error) => {
        self.error = Some(format!("切换工程失败，仍保留当前工程: {error:#}"));
        cx.notify();
      }
    }
  }

  fn toggle_prompts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.studio.read(cx).is_busy(cx) {
      return;
    }
    if !self.show_prompts {
      if let Err(error) = self.save(cx) {
        self.error = Some(format!("保存提示词失败: {error:#}"));
        cx.notify();
        return;
      }
      self
        .studio
        .update(cx, |studio, cx| studio.hide_previews(cx));
      self.update_prompts(window, cx);
    }
    self.show_prompts = !self.show_prompts;
    self.error = None;
    cx.notify();
  }
}

impl Render for Workspace {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let busy = self.studio.read(cx).is_busy(cx);
    let content = if self.show_prompts {
      div()
        .v_flex()
        .size_full()
        .p_4()
        .gap_3()
        .child("当前工程的完整提示词（只读，可选择复制）；点击「返回编辑器」继续修改。")
        .child(
          div()
            .flex_1()
            .min_h_0()
            .child(Textarea::new(&self.prompts).readonly(true).h_full()),
        )
        .into_any_element()
    } else {
      self.studio.clone().into_any_element()
    };
    div()
      .v_flex()
      .size_full()
      .bg(rgb(0x0c1320))
      .text_color(rgb(0xe2e8f0))
      .child(
        div()
          .h_flex()
          .flex_shrink_0()
          .p_3()
          .gap_3()
          .items_center()
          .bg(rgb(0x1d293c))
          .capture_any_mouse_down(cx.listener(|this, _, _, cx| this.return_input_focus(cx)))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .overflow_hidden()
              .font_weight(FontWeight::BOLD)
              .child(format!("工程：{}", self.projects.active().name)),
          )
          .child(
            Button::new("create-project")
              .debug_selector(|| "new-project".into())
              .primary()
              .label("新建工程")
              .disabled(busy)
              .on_click(cx.listener(|this, _, window, cx| this.open_new_project(window, cx))),
          )
          .child(
            Button::new("project-list")
              .label(format!("选择工程（{}）", self.projects.projects().len()))
              .disabled(busy)
              .on_click(cx.listener(|this, _, _, cx| {
                this.show_projects = !this.show_projects;
                cx.notify();
              })),
          )
          .child(
            Button::new("project-prompts")
              .label(if self.show_prompts {
                "返回编辑器"
              } else {
                "查看提示词"
              })
              .disabled(busy)
              .on_click(cx.listener(|this, _, window, cx| this.toggle_prompts(window, cx))),
          ),
      )
      .when(self.show_projects, |root| {
        root.child(
          div()
            .id("project-list-panel")
            .v_flex()
            .h(px(144.))
            .flex_shrink_0()
            .overflow_y_scroll()
            .p_3()
            .gap_2()
            .capture_any_mouse_down(cx.listener(|this, _, _, cx| this.return_input_focus(cx)))
            .children(self.projects.projects().iter().map(|project| {
              let id = project.id;
              Button::new(("project", id))
                .label(format!(
                  "{}{}",
                  if id == self.projects.active().id {
                    "● "
                  } else {
                    ""
                  },
                  project.name
                ))
                .disabled(busy)
                .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)))
            })),
        )
      })
      .children(self.error.as_ref().map(|error| {
        div()
          .px_4()
          .py_2()
          .bg(rgb(0x47252d))
          .text_color(rgb(0xfecaca))
          .child(error.clone())
      }))
      .child(div().flex_1().min_h_0().child(content))
  }
}

use gpui_kit::{
  component::{
    button::*,
    input::{Input, InputEvent, InputState},
    *,
  },
  *,
};

use crate::workspace_ui::Workspace;

pub struct NewProject {
  workspace: WeakEntity<Workspace>,
  parent: AnyWindowHandle,
  name: Entity<InputState>,
  error: Option<String>,
  _subscription: Subscription,
}

impl NewProject {
  pub fn new(
    workspace: WeakEntity<Workspace>,
    parent: AnyWindowHandle,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("例如：我的中文工程"));
    let subscription = cx.subscribe_in(&name, window, |this, _, event, window, cx| match event {
      InputEvent::PressEnter { .. } => this.confirm(window, cx),
      InputEvent::Change => {
        this.error = None;
        cx.notify();
      }
      _ => {}
    });
    name.read(cx).focus_handle(cx).focus(window, cx);
    Self {
      workspace,
      parent,
      name,
      error: None,
      _subscription: subscription,
    }
  }

  fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let name = self.name.read(cx).value().to_string();
    let result = cx
      .update_window(self.parent, |_, window, cx| {
        self.workspace.update(cx, |workspace, cx| {
          workspace.create_named(&name, window, cx)
        })
      })
      .and_then(|result| result)
      .and_then(|result| result);
    match result {
      Ok(()) => window.remove_window(),
      Err(error) => {
        self.error = Some(format!("新建工程失败: {error:#}"));
        self.name.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
      }
    }
  }
}

impl Render for NewProject {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .v_flex()
      .size_full()
      .p_4()
      .gap_3()
      .bg(rgb(0x0c1320))
      .text_color(rgb(0xe2e8f0))
      .child(
        div()
          .text_lg()
          .font_weight(FontWeight::BOLD)
          .child("新建工程"),
      )
      .child("工程名称（支持中文，最多 80 个字符）")
      .child(Input::new(&self.name))
      .children(self.error.as_ref().map(|error| {
        div()
          .text_sm()
          .text_color(rgb(0xfecaca))
          .child(error.clone())
      }))
      .child(div().flex_1())
      .child(
        div()
          .h_flex()
          .justify_end()
          .gap_3()
          .child(
            Button::new("cancel-new-project")
              .debug_selector(|| "cancel-new-project".into())
              .label("取消")
              .on_click(cx.listener(|_, _, window, _| window.remove_window())),
          )
          .child(
            Button::new("confirm-new-project")
              .debug_selector(|| "confirm-new-project".into())
              .primary()
              .label("创建工程")
              .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))),
          ),
      )
  }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
  use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
  };

  use gpui_kit::{
    self as gpui, AppContext, Entity, EntityInputHandler, Focusable, TestAppContext,
    VisualTestContext,
  };
  use gpui_mai_ui_client::projects::ProjectStore;

  use super::{NewProject, Workspace};

  struct Directory(PathBuf);

  impl Directory {
    fn projects(&self) -> ProjectStore {
      ProjectStore::open(&self.0.join("session.json")).unwrap()
    }
  }

  impl Drop for Directory {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  fn workspace(cx: &mut TestAppContext) -> (VisualTestContext, Directory) {
    let directory = Directory(std::env::temp_dir().join(format!(
      "mai-project-dialog-{}-{}",
      std::process::id(),
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    )));
    fs::create_dir(&directory.0).unwrap();
    let projects = directory.projects();
    let project = projects.load_active().unwrap();
    cx.update(gpui::init);
    let (parent, _) = cx.update(|cx| {
      gpui::open_window(Default::default(), cx, |window, cx| {
        cx.new(|cx| Workspace::new(projects, project, window, cx))
      })
      .unwrap()
    });
    let main = VisualTestContext::from_window(parent, cx);
    main.run_until_parked();
    (main, directory)
  }

  fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector).unwrap();
    cx.simulate_click(bounds.center(), Default::default());
    cx.run_until_parked();
  }

  fn open_dialog(main: &mut VisualTestContext) -> (Entity<NewProject>, VisualTestContext) {
    let parent = main.update(|window, _| window.window_handle());
    click(main, "new-project");
    let handle = main
      .windows()
      .into_iter()
      .find(|handle| *handle != parent)
      .unwrap();
    let dialog = main
      .cx
      .update_window(handle, |root, _, cx| {
        let root = root.downcast::<gpui::base::Root>().unwrap();
        root
          .read(cx)
          .view()
          .clone()
          .downcast::<NewProject>()
          .unwrap()
      })
      .unwrap();
    let visual = VisualTestContext::from_window(handle, main);
    visual.run_until_parked();
    (dialog, visual)
  }

  #[gpui::test]
  fn clicking_new_project_opens_focused_window_and_creates_chinese_name(cx: &mut TestAppContext) {
    let (mut main, directory) = workspace(cx);
    let (dialog, mut visual) = open_dialog(&mut main);
    assert_eq!(main.windows().len(), 2);
    visual.update(|window, cx| {
      assert!(
        dialog
          .read(cx)
          .name
          .read(cx)
          .focus_handle(cx)
          .is_focused(window)
      );
    });
    visual.simulate_input("中文工程 Demo");
    visual.update(|_, cx| {
      assert_eq!(dialog.read(cx).name.read(cx).value(), "中文工程 Demo");
    });
    click(&mut visual, "confirm-new-project");
    assert_eq!(main.windows().len(), 1);
    let projects = directory.projects();
    assert_eq!(projects.projects().len(), 2);
    assert_eq!(projects.active().name, "中文工程 Demo");
  }

  #[gpui::test]
  fn project_name_keeps_preedit_during_parent_repaint_and_enter_creates(cx: &mut TestAppContext) {
    let (mut main, directory) = workspace(cx);
    let (dialog, mut visual) = open_dialog(&mut main);
    visual.update(|window, cx| {
      dialog.read(cx).name.clone().update(cx, |state, cx| {
        state.replace_and_mark_text_in_range(None, "zhong", Some(5 .. 5), window, cx);
      });
    });
    main.update(|window, _| window.refresh());
    main.run_until_parked();
    visual.update(|window, cx| {
      dialog.read(cx).name.clone().update(cx, |state, cx| {
        assert_eq!(state.marked_text_range(window, cx), Some(0 .. 5));
        state.replace_and_mark_text_in_range(None, "中文", Some(2 .. 2), window, cx);
      });
      window.refresh();
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
      dialog.read(cx).name.clone().update(cx, |state, cx| {
        assert_eq!(state.marked_text_range(window, cx), Some(0 .. 2));
        state.replace_text_in_range(None, "中文工程", window, cx);
        assert_eq!(state.marked_text_range(window, cx), None);
        assert_eq!(
          state.selected_text_range(false, window, cx).unwrap().range,
          4 .. 4
        );
      });
    });
    visual.simulate_keystrokes("enter");
    main.run_until_parked();
    assert_eq!(main.windows().len(), 1);
    assert_eq!(directory.projects().active().name, "中文工程");
  }

  #[gpui::test]
  fn invalid_name_stays_editable_and_can_be_corrected(cx: &mut TestAppContext) {
    let (mut main, directory) = workspace(cx);
    let (dialog, mut visual) = open_dialog(&mut main);
    click(&mut visual, "confirm-new-project");
    assert_eq!(main.windows().len(), 2);
    assert_eq!(directory.projects().projects().len(), 1);
    visual.update(|_, cx| assert!(dialog.read(cx).error.is_some()));
    visual.simulate_input("默认工程");
    click(&mut visual, "confirm-new-project");
    assert_eq!(directory.projects().projects().len(), 1);
    visual.update(|_, cx| {
      assert!(dialog.read(cx).error.is_some());
      assert_eq!(dialog.read(cx).name.read(cx).value(), "默认工程");
    });
    visual.simulate_keystrokes("cmd-a backspace");
    visual.simulate_input("修正后的工程");
    click(&mut visual, "confirm-new-project");
    assert_eq!(main.windows().len(), 1);
    assert_eq!(directory.projects().active().name, "修正后的工程");
  }

  #[gpui::test]
  fn cancelling_does_not_create_and_reopening_uses_a_fresh_input(cx: &mut TestAppContext) {
    let (mut main, directory) = workspace(cx);
    let (_, mut visual) = open_dialog(&mut main);
    visual.simulate_input("不应保存的工程");
    click(&mut visual, "cancel-new-project");
    assert_eq!(main.windows().len(), 1);
    assert_eq!(directory.projects().projects().len(), 1);
    let (dialog, mut visual) = open_dialog(&mut main);
    visual.update(|_, cx| assert_eq!(dialog.read(cx).name.read(cx).value(), ""));
    visual.simulate_input("重新命名");
    click(&mut visual, "confirm-new-project");
    assert_eq!(directory.projects().active().name, "重新命名");
  }

  #[gpui::test]
  fn opening_again_reuses_the_same_naming_window(cx: &mut TestAppContext) {
    let (mut main, directory) = workspace(cx);
    let (_, mut visual) = open_dialog(&mut main);
    visual.simulate_input("仍在命名");
    let (dialog, mut existing) = open_dialog(&mut main);
    assert_eq!(main.windows().len(), 2);
    existing.update(|_, cx| {
      assert_eq!(dialog.read(cx).name.read(cx).value(), "仍在命名");
    });
    click(&mut existing, "confirm-new-project");
    assert_eq!(directory.projects().projects().len(), 2);
    assert_eq!(directory.projects().active().name, "仍在命名");
  }
}

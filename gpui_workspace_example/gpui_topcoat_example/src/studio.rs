use std::time::Duration;

use gpui_kit::{
  component::{button::*, *},
  *,
};
use gpui_topcoat_example::studio_request;
use topcoat_gpui_protocol::{StudioCommand, StudioSnapshot, StudioTheme};

pub(crate) struct StudioPage {
  server: String,
  published: StudioSnapshot,
  theme: StudioTheme,
  intensity: u8,
  dirty: bool,
  busy: bool,
  status: String,
  _request: Option<Task<()>>,
  _poll: Task<()>,
}
impl StudioPage {
  pub(crate) fn new(server: String, cx: &mut Context<Self>) -> Self {
    let poll = cx.spawn(async move |view, cx| {
      loop {
        cx.background_executor().timer(Duration::from_secs(2)).await;
        if view.update(cx, |view, cx| view.sync(false, cx)).is_err() {
          break;
        }
      }
    });
    let mut view = Self {
      server,
      published: StudioSnapshot::default(),
      theme: StudioTheme::Ocean,
      intensity: 70,
      dirty: false,
      busy: false,
      status: "正在连接…".into(),
      _request: None,
      _poll: poll,
    };
    view.sync(false, cx);
    view
  }
  fn sync(&mut self, save: bool, cx: &mut Context<Self>) {
    // UI-only tests use an empty origin; real I/O has separate Tokio integration tests.
    #[cfg(test)]
    if self.server.is_empty() {
      return;
    }
    if self.busy {
      return;
    }
    self.busy = true;
    let server = self.server.clone();
    let command = save.then_some(StudioCommand::Apply {
      theme: self.theme,
      intensity: self.intensity,
    });
    let work =
      gpui_topcoat_example::runtime::spawn(async move { studio_request(&server, command).await });
    self._request = Some(cx.spawn(async move |view, cx| {
      let result = work.await;
      let _ = view.update(cx, |view, cx| {
        view.busy = false;
        match result {
          Ok(snapshot) => {
            if save {
              view.dirty = false;
            }
            if !view.dirty {
              view.theme = snapshot.theme;
              view.intensity = snapshot.intensity;
            }
            view.published = snapshot;
            view.status = "已同步 · 每两秒检查网页修改".into();
          }
          Err(error) => {
            view.status = format!("同步失败，显示数据可能已过期：{error}；发布不会自动重试")
          }
        }
        cx.notify();
      });
    }));
    cx.notify();
  }
}
impl Render for StudioPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .id("studio-page")
      .v_flex()
      .size_full()
      .p_4()
      .gap_3()
      .overflow_y_scroll()
      .child("配色实验室")
      .child("网页调整配色并发布，这里自动同步；你也可以在桌面预览并发布。")
      .child(
        Button::new("studio-web")
          .label("打开配色网页")
          .on_click(cx.listener(|view, _, _, cx| {
            cx.open_url(&format!("{}/studio", view.server.trim_end_matches('/')));
          })),
      )
      .child(
        div().flex().gap_2().children(
          [
            ("theme-ocean", StudioTheme::Ocean),
            ("theme-sunset", StudioTheme::Sunset),
            ("theme-forest", StudioTheme::Forest),
          ]
          .into_iter()
          .map(|(id, theme)| {
            Button::new(id)
              .label(theme.label())
              .disabled(self.busy)
              .on_click(cx.listener(move |view, _, _, cx| {
                view.theme = theme;
                view.dirty = true;
                cx.notify();
              }))
          }),
        ),
      )
      .child(format!("{} · 强度 {}%", self.theme.label(), self.intensity))
      .child(
        div()
          .h(px(160.))
          .w_full()
          .rounded_lg()
          .bg(rgb(self.theme.color()))
          .opacity(0.2 + self.intensity as f32 * 0.008),
      )
      .child(
        div()
          .flex()
          .gap_2()
          .child(
            Button::new("studio-less")
              .label("强度 −10")
              .disabled(self.busy)
              .on_click(cx.listener(|view, _, _, cx| {
                view.intensity = view.intensity.saturating_sub(10);
                view.dirty = true;
                cx.notify();
              })),
          )
          .child(
            Button::new("studio-more")
              .label("强度 +10")
              .disabled(self.busy)
              .on_click(cx.listener(|view, _, _, cx| {
                view.intensity = view.intensity.saturating_add(10).min(100);
                view.dirty = true;
                cx.notify();
              })),
          ),
      )
      .child(if self.dirty {
        "本地预览 · 点击发布同步到网页"
      } else {
        "正在展示已发布配色"
      })
      .child(
        Button::new("studio-publish")
          .primary()
          .label("发布到两端")
          .disabled(self.busy)
          .on_click(cx.listener(|view, _, _, cx| view.sync(true, cx))),
      )
      .child(
        Button::new("studio-refresh")
          .label("使用最新发布")
          .disabled(self.busy)
          .on_click(cx.listener(|view, _, _, cx| {
            view.dirty = false;
            view.theme = view.published.theme;
            view.intensity = view.published.intensity;
            view.sync(false, cx);
          })),
      )
      .child(format!(
        "两端共享：{} · 强度 {}% · 版本 {}",
        self.published.theme.label(),
        self.published.intensity,
        self.published.revision
      ))
      .child(self.status.clone())
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{AppContext, TestAppContext, component::Root, px, size, test::TestWindowExt};
  use topcoat_gpui_protocol::StudioTheme;

  use super::StudioPage;
  #[gpui_kit::test]
  fn palette_controls_change_only_local_preview(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut page = None;
    let window = cx.open_window(size(px(1000.), px(900.)), |window, cx| {
      let view = cx.new(|cx| StudioPage::new(String::new(), cx));
      page = Some(view.clone());
      Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| {
      page.update(cx, |view, cx| {
        view._request = None;
        view.busy = false;
        cx.notify();
      });
      window.within("studio-page").click("theme-sunset", cx);
      window.within("studio-page").click("studio-less", cx);
      let view = page.read(cx);
      assert_eq!(view.theme, StudioTheme::Sunset);
      assert_eq!(view.intensity, 60);
      assert!(view.dirty);
      assert_eq!(view.published.theme, StudioTheme::Ocean);
      assert_eq!(view.published.revision, 0);
    })
    .unwrap();
  }
}

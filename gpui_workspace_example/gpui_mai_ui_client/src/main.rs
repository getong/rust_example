mod app;
mod grounding_ui;
mod new_project;
mod preview;
mod workspace_ui;

use std::path::PathBuf;

use anyhow::Result;
use gpui_kit::{component::*, *};
use gpui_mai_ui_client::projects::ProjectStore;

fn main() -> Result<()> {
  let path = std::env::var_os("MAI_UI_SESSION")
    .map(PathBuf::from)
    .unwrap_or_else(|| {
      PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("studio-output/session.json")
    });
  let projects = ProjectStore::open(&path)?;
  let project = projects.load_active()?;
  gpui_kit::application()
    .with_assets(gpui_kit::assets::AllAssets)
    .run(move |cx| {
      gpui_kit::init(cx);
      Theme::change(ThemeMode::Dark, None, cx);
      cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
          cx.quit();
        }
      })
      .detach();
      let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
          title: Some("MAI-UI Studio · GPUI × Ollama".into()),
          ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
          None,
          size(px(1440.), px(960.)),
          cx,
        ))),
        window_min_size: Some(size(px(1080.), px(720.))),
        ..Default::default()
      };
      if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| workspace_ui::Workspace::new(projects, project, window, cx))
      }) {
        eprintln!("创建窗口失败: {error:#}");
        cx.quit();
      }
      cx.activate(true);
    });
  Ok(())
}

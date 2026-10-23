use std::{path::PathBuf, time::Duration};

use anyhow::{Context as _, Result, anyhow};
use gpui::{
  App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_video_player::{Url, Video, VideoOptions, video};

struct VideoPlayer {
  video: Video,
  error: Option<String>,
}

impl Render for VideoPlayer {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let viewport = window.viewport_size();
    let paused = self.video.paused();
    let muted = self.video.muted();
    div()
      .size_full()
      .flex()
      .flex_col()
      .bg(rgb(0x101218))
      .text_color(rgb(0xffffff))
      .child(
        video(self.video.clone())
          .id("main-video")
          .size(viewport.width, (viewport.height - px(96.0)).max(px(1.0)))
          .buffer_capacity(3),
      )
      .child(
        div()
          .flex()
          .gap_4()
          .p_4()
          .items_center()
          .child(
            div()
              .id("back")
              .cursor_pointer()
              .child("-5s")
              .on_click(cx.listener(|this, _, _, cx| {
                this.error = this
                  .video
                  .seek(
                    this.video.position().saturating_sub(Duration::from_secs(5)),
                    false,
                  )
                  .err()
                  .map(|error| error.to_string());
                cx.notify();
              })),
          )
          .child(
            div()
              .id("play-pause")
              .cursor_pointer()
              .child(if paused { "Play" } else { "Pause" })
              .on_click(cx.listener(|this, _, _, cx| {
                this.video.set_paused(!this.video.paused());
                cx.notify();
              })),
          )
          .child(
            div()
              .id("forward")
              .cursor_pointer()
              .child("+5s")
              .on_click(cx.listener(|this, _, _, cx| {
                let target = this
                  .video
                  .position()
                  .saturating_add(Duration::from_secs(5))
                  .min(this.video.duration());
                this.error = this
                  .video
                  .seek(target, false)
                  .err()
                  .map(|error| error.to_string());
                cx.notify();
              })),
          )
          .child(
            div()
              .id("mute")
              .cursor_pointer()
              .child(if muted { "Unmute" } else { "Mute" })
              .on_click(cx.listener(|this, _, _, cx| {
                this.video.set_muted(!this.video.muted());
                cx.notify();
              })),
          ),
      )
      .when_some(self.error.clone(), |element, error| {
        element.child(div().px_4().text_color(rgb(0xff8080)).child(error))
      })
  }
}

fn main() -> Result<()> {
  env_logger::init();
  configure_gstreamer_runtime();
  let source = std::env::args_os()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/test3.mp4"));
  let path = source
    .canonicalize()
    .with_context(|| format!("cannot open video: {}", source.display()))?;
  let uri =
    Url::from_file_path(&path).map_err(|_| anyhow!("invalid video path: {}", path.display()))?;
  gpui_platform::application().run(move |cx: &mut App| {
    // Start GStreamer after GPUI receives the macOS application launch callback.
    let video = match Video::new_with_options(
      &uri,
      VideoOptions {
        looping: Some(true),
        frame_buffer_capacity: Some(3),
        ..Default::default()
      },
    ) {
      Ok(video) => video,
      Err(error) => {
        eprintln!("failed to initialize video playback: {error}");
        cx.quit();
        return;
      }
    };
    cx.on_window_closed(|cx, _| {
      if cx.windows().is_empty() {
        cx.quit();
      }
    })
    .detach();
    if let Err(error) = cx.open_window(
      WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
          None,
          size(px(960.0), px(640.0)),
          cx,
        ))),
        titlebar: Some(gpui::TitlebarOptions {
          title: Some("GPUI Video Player".into()),
          ..Default::default()
        }),
        focus: true,
        ..Default::default()
      },
      |_, cx| cx.new(|_| VideoPlayer { video, error: None }),
    ) {
      eprintln!("failed to open player window: {error:#}");
      cx.quit();
    }
    cx.activate(true);
  });
  Ok(())
}

#[cfg(target_os = "macos")]
fn configure_gstreamer_runtime() {
  use std::ffi::OsString;

  let mut library_paths = vec![
    OsString::from("/opt/homebrew/lib"),
    OsString::from("/opt/homebrew/opt/glib/lib"),
    OsString::from("/opt/homebrew/opt/gettext/lib"),
    OsString::from("/opt/homebrew/opt/gstreamer/lib"),
  ];
  if let Some(existing) = std::env::var_os("DYLD_LIBRARY_PATH") {
    library_paths.push(existing);
  }
  let library_path = std::env::join_paths(library_paths)
    .expect("Homebrew library paths must not contain a separator");

  let mut typelib_paths = vec![OsString::from(
    "/opt/homebrew/opt/gstreamer/lib/girepository-1.0",
  )];
  if let Some(existing) = std::env::var_os("GI_TYPELIB_PATH") {
    typelib_paths.push(existing);
  }
  let typelib_path = std::env::join_paths(typelib_paths)
    .expect("GStreamer typelib paths must not contain a separator");

  // GStreamer launches gst-plugin-scanner as a child process. The child must
  // receive these paths because its GI typelib loader uses dlopen by name.
  unsafe {
    std::env::set_var("DYLD_LIBRARY_PATH", library_path);
    std::env::set_var("GI_TYPELIB_PATH", typelib_path);
  }
}

#[cfg(not(target_os = "macos"))]
fn configure_gstreamer_runtime() {}

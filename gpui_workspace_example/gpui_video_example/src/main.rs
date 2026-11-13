use std::{
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use gpu::GpuContext;
use gpui_kit::{
  component::{button::*, checkbox::Checkbox, slider::*, *},
  prelude::FluentBuilder as _,
  *,
};
use playback::{Playback, SUPPORTED_VIDEO_EXTENSIONS};
use video_view::VideoView;

mod audio;
mod demuxer;
mod gpu;
mod playback;
mod software;
mod video_view;

struct VideoPlayer {
  gpu: Option<GpuContext>,
  video_view: VideoView,
  paused: bool,
  volume: f32,
  muted: bool,
  choosing_file: bool,
  volume_slider: Entity<SliderState>,
  seek_slider: Entity<SliderState>,
  scrub_target: Option<Duration>,
  preview_pending: bool,
  playback: Option<Playback>,
  last_tick: Instant,
  error: Option<String>,
  _volume_subscription: Subscription,
  _seek_subscription: Subscription,
}

impl VideoPlayer {
  fn new(
    gpu: Option<GpuContext>,
    initial_path: Option<PathBuf>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let volume_slider = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(1.0)
        .step(0.01)
        .default_value(1.0)
    });
    let subscription = cx.subscribe(&volume_slider, |this, _, event, cx| {
      if let SliderEvent::Change(SliderValue::Single(volume)) = event {
        this.volume = *volume;
        this.update_volume();
        cx.notify();
      }
    });
    let seek_slider = cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.0001));
    let seek_subscription = cx.subscribe_in(&seek_slider, window, |this, _, event, window, cx| {
      let Some(playback) = &this.playback else {
        return;
      };
      match event {
        SliderEvent::Change(SliderValue::Single(fraction)) => {
          this.scrub_target = Some(
            playback
              .duration
              .mul_f64(f64::from(fraction.clamp(0.0, 1.0))),
          );
          playback.set_paused(true);
          this.last_tick = Instant::now();
        }
        SliderEvent::Release(SliderValue::Single(fraction)) => {
          let target = playback
            .duration
            .mul_f64(f64::from(fraction.clamp(0.0, 1.0)));
          this.seek_to(target, window);
        }
        _ => return,
      }
      cx.notify();
    });
    let mut player = Self {
      gpu,
      video_view: VideoView::default(),
      paused: false,
      volume: 1.0,
      muted: false,
      choosing_file: false,
      volume_slider,
      seek_slider,
      scrub_target: None,
      preview_pending: false,
      playback: None,
      last_tick: Instant::now(),
      error: None,
      _volume_subscription: subscription,
      _seek_subscription: seek_subscription,
    };
    if let Some(path) = initial_path {
      player.open_path(&path, window);
    }
    player
  }

  fn open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.choosing_file {
      return;
    }
    self.choosing_file = true;
    if let Some(playback) = &self.playback {
      playback.set_paused(true);
    }
    let selected_file = rfd::AsyncFileDialog::new()
      .set_parent(window)
      .set_title("Open an MP4 video")
      .add_filter("MP4 video", SUPPORTED_VIDEO_EXTENSIONS)
      .set_can_create_directories(false)
      .pick_file();
    cx.spawn_in(window, async move |this, cx| {
      let picked = selected_file.await;
      let _ = this.update_in(cx, |this, window, cx| {
        this.choosing_file = false;
        if let Some(file) = picked {
          this.open_path(file.path(), window);
        }
        if let Some(playback) = &this.playback {
          playback.set_paused(this.paused || this.preview_pending);
        }
        this.last_tick = Instant::now();
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn open_path(&mut self, path: &Path, window: &mut Window) {
    if let Some(playback) = &self.playback {
      playback.set_paused(true);
    }
    match Playback::new(self.gpu.as_ref(), path) {
      Ok(playback) => {
        playback.set_volume(if self.muted { 0.0 } else { self.volume });
        playback.set_paused(true);
        self.playback = Some(playback);
        self.scrub_target = None;
        self.preview_pending = true;
        self.error = self
          .video_view
          .clear(window)
          .err()
          .map(|error| format!("{error:#}"));
        window.set_window_title(&format!("GPUI Video Player — {}", path.display()));
      }
      Err(error) => {
        if let Some(playback) = &self.playback {
          playback.set_paused(self.paused || self.preview_pending);
        }
        self.error = Some(format!("Could not open {}\n\n{error:#}", path.display()));
      }
    }
    self.last_tick = Instant::now();
  }

  fn advance_playback(&mut self, window: &mut Window) -> Result<()> {
    self.video_view.poll(self.gpu.as_ref(), window)?;
    let now = Instant::now();
    let elapsed = now - self.last_tick;
    self.last_tick = now;
    if (!self.paused || self.preview_pending)
      && !self.choosing_file
      && self.scrub_target.is_none()
      && let Some(playback) = &mut self.playback
      && let Some(frame) = playback.next_frame(if self.preview_pending {
        Duration::ZERO
      } else {
        elapsed
      })?
    {
      self.video_view.update(self.gpu.as_ref(), frame, window)?;
      if self.preview_pending {
        self.preview_pending = false;
        playback.set_paused(self.paused);
      }
    }
    Ok(())
  }

  fn update_volume(&self) {
    if let Some(playback) = &self.playback {
      playback.set_volume(if self.muted { 0.0 } else { self.volume });
    }
  }

  fn seek_to(&mut self, position: Duration, window: &mut Window) {
    self.scrub_target = None;
    let Some(playback) = &mut self.playback else {
      return;
    };
    playback.set_paused(true);
    match playback.seek(self.gpu.as_ref(), position) {
      Ok(()) => {
        self.preview_pending = true;
        self.error = self
          .video_view
          .clear(window)
          .err()
          .map(|error| format!("{error:#}"));
      }
      Err(error) => {
        self.error = Some(format!("Could not seek the video\n\n{error:#}"));
        playback.set_paused(self.paused || self.preview_pending);
      }
    }
    self.last_tick = Instant::now();
  }

  fn timeline(&self) -> impl IntoElement {
    let duration = self
      .playback
      .as_ref()
      .map(|playback| playback.duration)
      .unwrap_or_default();
    let position = self.scrub_target.unwrap_or_else(|| {
      self
        .playback
        .as_ref()
        .map(|playback| playback.time)
        .unwrap_or_default()
    });
    div()
      .flex()
      .items_center()
      .gap_3()
      .px_3()
      .py_2()
      .bg(rgb(0x1b1e26))
      .child(div().text_sm().child(format_time(position)))
      .child(
        div().flex_1().min_w_0().child(
          Slider::new(&self.seek_slider)
            .horizontal()
            .accessibility_label("Playback position")
            .disabled(duration.is_zero() || self.choosing_file),
        ),
      )
      .child(div().text_sm().child(format_time(duration)))
  }

  fn toggle_paused(&mut self, cx: &mut Context<Self>) {
    self.paused = !self.paused;
    if let Some(playback) = &self.playback {
      playback.set_paused(self.paused || self.preview_pending);
    }
    self.last_tick = Instant::now();
    cx.notify();
  }

  fn controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let has_audio = self.playback.as_ref().is_some_and(Playback::has_audio);
    let status = self.playback.as_ref().map(|playback| {
      let resolution = self
        .video_view
        .size()
        .map(|(width, height)| format!(" | {width}x{height}"))
        .unwrap_or_default();
      format!(
        "{:.2}s | frame {}{} | {}",
        playback.time.as_secs_f64(),
        playback.frame_number,
        resolution,
        playback.backend_name()
      )
    });
    div()
      .flex()
      .flex_wrap()
      .items_center()
      .gap_3()
      .p_3()
      .bg(rgb(0x1b1e26))
      .child(
        Button::new("open")
          .primary()
          .label("Open MP4")
          .disabled(self.choosing_file || self.scrub_target.is_some())
          .on_click(cx.listener(|this, _, window, cx| this.open_file(window, cx))),
      )
      .child(
        Button::new("play-pause")
          .label(if self.paused { "Play" } else { "Pause" })
          .disabled(self.playback.is_none() || self.choosing_file || self.scrub_target.is_some())
          .on_click(cx.listener(|this, _, _, cx| this.toggle_paused(cx))),
      )
      .child(
        Checkbox::new("mute")
          .label("Mute")
          .checked(self.muted)
          .disabled(!has_audio)
          .on_change(cx.listener(|this, checked: &bool, _, cx| {
            this.muted = *checked;
            this.update_volume();
            cx.notify();
          })),
      )
      .child(
        div()
          .w(px(150.0))
          .child(Slider::new(&self.volume_slider).disabled(!has_audio)),
      )
      .child(format!("Volume {:.0}%", self.volume * 100.0))
      .when_some(status, |element, status| {
        element.child(div().text_sm().child(status))
      })
  }
}

impl Render for VideoPlayer {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    if let Err(error) = self.advance_playback(window) {
      eprintln!("Playback failed: {error:#}");
      self.error = Some(format!("Playback failed\n\n{error:#}"));
      self.playback = None;
      self.preview_pending = false;
      self.scrub_target = None;
      if let Err(error) = self.video_view.clear(window) {
        eprintln!("Failed to clear video output: {error:#}");
      }
    }
    if self.scrub_target.is_none() {
      let fraction = self
        .playback
        .as_ref()
        .filter(|playback| !playback.duration.is_zero())
        .map(|playback| (playback.time.as_secs_f64() / playback.duration.as_secs_f64()) as f32)
        .unwrap_or(0.0);
      if let SliderValue::Single(current) = self.seek_slider.read(cx).value()
        && (current - fraction).abs() > f32::EPSILON
      {
        self
          .seek_slider
          .update(cx, |slider, cx| slider.set_value(fraction, window, cx));
      }
    }
    if (self
      .playback
      .as_ref()
      .is_some_and(|playback| playback.time < playback.duration)
      && !self.paused
      && !self.choosing_file
      && self.scrub_target.is_none())
      || (self.preview_pending && !self.choosing_file && self.scrub_target.is_none())
      || self.video_view.has_pending_frame()
    {
      window.request_animation_frame();
    }
    div()
      .size_full()
      .flex()
      .flex_col()
      .bg(rgb(0x000000))
      .text_color(rgb(0xe6e8ef))
      .child(
        div()
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(self.video_view.element()),
      )
      .when_some(self.error.clone(), |element, message| {
        element.child(
          div()
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .bg(rgb(0x48242b))
            .child(div().flex_1().child(message))
            .child(
              Button::new("dismiss-error")
                .label("Dismiss")
                .on_click(cx.listener(|this, _, _, cx| {
                  this.error = None;
                  cx.notify();
                })),
            ),
        )
      })
      .child(self.timeline())
      .child(self.controls(cx))
  }
}

fn format_time(time: Duration) -> String {
  let seconds = time.as_secs();
  if seconds >= 3600 {
    format!(
      "{}:{:02}:{:02}",
      seconds / 3600,
      seconds / 60 % 60,
      seconds % 60
    )
  } else {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt().init();
  let initial_path = std::env::args_os().nth(1).map(PathBuf::from);
  let gpu = match GpuContext::new().await {
    Ok(gpu) => Some(gpu),
    Err(error) => {
      eprintln!("Hardware decoding unavailable, using FFmpeg: {error:#}");
      None
    }
  };
  gpui_kit::application().run(move |cx| {
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
        title: Some("GPUI Video Player".into()),
        ..Default::default()
      }),
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(1100.0), px(720.0)),
        cx,
      ))),
      window_min_size: Some(size(px(520.0), px(320.0))),
      ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
      cx.new(|cx| VideoPlayer::new(gpu, initial_path, window, cx))
    });
    if let Err(error) = opened.context("Failed to open the GPUI video player window") {
      eprintln!("{error:#}");
      cx.quit();
    }
    cx.activate(true);
  });
  Ok(())
}

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use engine::{Engine, Event, Operation, State};
use gpu::GpuContext;
use gpui_kit::{
  component::{
    button::*,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
    slider::*,
    *,
  },
  prelude::FluentBuilder as _,
  *,
};
use image::{Frame as ImageFrame, RgbaImage};
use media::{MediaInfo, SUPPORTED_VIDEO_EXTENSIONS};
use playback::PLAYBACK_SPEEDS;
use preview::Previewer;
use settings::Settings;
use subtitles::{SUPPORTED_SUBTITLE_EXTENSIONS, SubtitleTrack};
use video_view::VideoView;

mod audio;
mod demuxer;
mod engine;
mod gpu;
mod media;
mod playback;
mod preview;
mod process;
mod settings;
mod software;
mod subtitles;
mod video_view;

#[derive(Clone, Copy)]
enum OpenFileKind {
  Video,
  Subtitles,
}

struct VideoPlayer {
  gpu: Option<GpuContext>,
  engine: Engine,
  generation: u64,
  video_view: VideoView,
  state: Option<State>,
  info: Option<MediaInfo>,
  path: Option<PathBuf>,
  settings: Settings,
  config_path: Option<PathBuf>,
  last_save: Instant,
  loading: bool,
  choosing_file: bool,
  settings_open: bool,
  focus: FocusHandle,
  volume_slider: Entity<SliderState>,
  seek_slider: Entity<SliderState>,
  scrub_target: Option<Duration>,
  scrub_was_paused: bool,
  subtitle_track: Option<SubtitleTrack>,
  playlist: Vec<PathBuf>,
  error: Option<String>,
  notice: Option<String>,
  previewer: Previewer,
  thumbnail_id: u64,
  thumbnail: Option<(Arc<RenderImage>, Duration)>,
  hover_time: Option<Duration>,
  last_thumbnail: Instant,
  timeline_bounds: Bounds<Pixels>,
  _subscriptions: Vec<Subscription>,
}

impl VideoPlayer {
  fn new(
    gpu: Option<GpuContext>,
    initial_paths: Vec<PathBuf>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let config_path = Settings::path();
    let (settings, config_path, error) = match Settings::load(&config_path) {
      Ok(settings) => (settings, Some(config_path), None),
      Err(error) => (Settings::default(), None, Some(format!("{error:#}"))),
    };
    let volume_slider = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(1.0)
        .step(0.01)
        .default_value(settings.volume)
    });
    let volume_subscription = cx.subscribe(&volume_slider, |this, _, event, cx| {
      if let SliderEvent::Change(SliderValue::Single(volume)) = event {
        this.settings.volume = *volume;
        this.update_volume();
        this.save();
        cx.notify();
      }
    });
    let seek_slider = cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.0001));
    let seek_subscription = cx.subscribe_in(&seek_slider, window, |this, _, event, window, cx| {
      let Some(state) = &this.state else {
        return;
      };
      let duration = state.duration;
      match event {
        SliderEvent::Change(SliderValue::Single(fraction)) => {
          if this.scrub_target.is_none() {
            this.scrub_was_paused = state.paused;
          }
          this.scrub_target = Some(duration.mul_f64(f64::from(fraction.clamp(0.0, 1.0))));
          this.engine.send(Operation::Pause(true));
        }
        SliderEvent::Release(SliderValue::Single(fraction)) => {
          let paused = this.scrub_was_paused;
          this.seek_to(
            duration.mul_f64(f64::from(fraction.clamp(0.0, 1.0))),
            window,
          );
          this.engine.send(Operation::Pause(paused));
        }
        _ => return,
      }
      cx.notify();
    });
    let focus = cx.focus_handle();
    window.focus(&focus, cx);
    let engine = Engine::new(
      gpu.clone(),
      settings.speed,
      if settings.muted { 0.0 } else { settings.volume },
      settings.repeat,
    );
    let mut player = Self {
      gpu,
      engine,
      generation: 0,
      video_view: VideoView::default(),
      state: None,
      info: None,
      path: None,
      settings,
      config_path,
      last_save: Instant::now(),
      loading: false,
      choosing_file: false,
      settings_open: false,
      focus,
      volume_slider,
      seek_slider,
      scrub_target: None,
      scrub_was_paused: true,
      subtitle_track: None,
      playlist: initial_paths.clone(),
      error,
      notice: None,
      previewer: Previewer::new(),
      thumbnail_id: 0,
      thumbnail: None,
      hover_time: None,
      last_thumbnail: Instant::now() - Duration::from_secs(1),
      timeline_bounds: Bounds::default(),
      _subscriptions: vec![volume_subscription, seek_subscription],
    };
    let weak = cx.weak_entity();
    window.on_window_should_close(cx, move |_, cx| {
      let _ = weak.update(cx, |this, _| this.save());
      true
    });
    cx.spawn_in(window, async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(16))
          .await;
        if this
          .update_in(cx, |this, window, cx| {
            if this.poll(window, cx) {
              cx.notify();
            }
          })
          .is_err()
        {
          break;
        }
      }
    })
    .detach();
    if let Some(path) = initial_paths.first() {
      player.open_path(path, None, window);
    }
    player
  }

  fn save(&mut self) {
    if let (Some(path), Some(state)) = (&self.path, &self.state) {
      let subtitle = self
        .subtitle_track
        .as_ref()
        .map(|track| track.path().to_path_buf())
        .filter(|path| {
          path
            .extension()
            .is_some_and(|value| value.eq_ignore_ascii_case("srt"))
        });
      self.settings.remember(
        path,
        if state.ended {
          0.0
        } else {
          state.time.as_secs_f64()
        },
        subtitle,
      );
    }
    if let Some(path) = &self.config_path
      && let Err(error) = self.settings.save(path)
    {
      self.notice = Some(format!("Could not save settings: {error:#}"));
    }
    self.last_save = Instant::now();
  }

  fn issue(&mut self, operation: Operation, window: &mut Window) {
    self.generation = self.engine.send(operation);
    self.loading = true;
    self.scrub_target = None;
    while self.engine.frames.try_recv().is_ok() {}
    if let Err(error) = self.video_view.clear(window) {
      self.error = Some(format!("{error:#}"));
    }
  }

  fn open_path(&mut self, path: &Path, subtitle: Option<PathBuf>, window: &mut Window) {
    self.save();
    let path = if path.is_absolute() {
      path.to_path_buf()
    } else {
      std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
    };
    let recent = self.settings.recent.iter().find(|entry| entry.path == path);
    let position = recent
      .and_then(|entry| Duration::try_from_secs_f64(entry.position).ok())
      .unwrap_or_default();
    let subtitle = subtitle.or_else(|| recent.and_then(|entry| entry.subtitle.clone()));
    self.error = None;
    self.notice = None;
    self.issue(
      Operation::Open {
        path,
        position,
        subtitle,
      },
      window,
    );
  }

  fn open_files(&mut self, paths: Vec<PathBuf>, window: &mut Window) {
    let subtitle = paths
      .iter()
      .find(|path| {
        path
          .extension()
          .is_some_and(|value| value.eq_ignore_ascii_case("srt"))
      })
      .cloned();
    let videos = paths
      .into_iter()
      .filter(|path| media::supported(path))
      .map(absolute_path)
      .collect::<Vec<_>>();
    if let Some(path) = videos.first().cloned() {
      self.playlist = videos;
      self.open_path(&path, subtitle, window);
    } else if let Some(path) = subtitle
      && self.path.is_some()
    {
      self.engine.send(Operation::Subtitle(path));
    } else {
      self.notice = Some("Drop supported videos or an SRT subtitle file".into());
    }
  }

  fn open_file(&mut self, kind: OpenFileKind, window: &mut Window, cx: &mut Context<Self>) {
    if self.choosing_file {
      return;
    }
    self.choosing_file = true;
    let was_paused = self.state.as_ref().is_none_or(|state| state.paused);
    self.engine.send(Operation::Pause(true));
    let (title, filter, extensions) = match kind {
      OpenFileKind::Video => ("Open videos", "Video files", SUPPORTED_VIDEO_EXTENSIONS),
      OpenFileKind::Subtitles => (
        "Open SRT subtitles",
        "SubRip subtitles",
        SUPPORTED_SUBTITLE_EXTENSIONS,
      ),
    };
    let dialog = rfd::AsyncFileDialog::new()
      .set_parent(window)
      .set_title(title)
      .add_filter(filter, extensions)
      .set_can_create_directories(false);
    cx.spawn_in(window, async move |this, cx| {
      let paths = match kind {
        OpenFileKind::Video => dialog.pick_files().await.map(|files| {
          files
            .into_iter()
            .map(|file| file.path().to_path_buf())
            .collect::<Vec<_>>()
        }),
        OpenFileKind::Subtitles => dialog
          .pick_file()
          .await
          .map(|file| vec![file.path().to_path_buf()]),
      };
      let _ = this.update_in(cx, |this, window, cx| {
        this.choosing_file = false;
        let opened_video = paths.is_some() && matches!(kind, OpenFileKind::Video);
        if let Some(paths) = paths {
          match kind {
            OpenFileKind::Video => this.open_files(paths, window),
            OpenFileKind::Subtitles => {
              if let Some(path) = paths.first() {
                this.engine.send(Operation::Subtitle(path.clone()));
              }
            }
          }
        }
        if !opened_video {
          this.engine.send(Operation::Pause(was_paused));
        }
        cx.notify();
      });
    })
    .detach();
    cx.notify();
  }

  fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
    let mut changed = false;
    let mut play_next = false;
    while let Ok(event) = self.engine.events.try_recv() {
      match event {
        Event::Loaded {
          generation,
          path,
          info,
        } if generation == self.generation => {
          self.video_view.configure(&info);
          self.subtitle_track = None;
          self.info = Some(info);
          self.path = Some(path);
          self.state = None;
          window.set_window_title(&format!(
            "GPUI Video Player — {}",
            self
              .path
              .as_ref()
              .map(|path| path.display().to_string())
              .unwrap_or_default()
          ));
        }
        Event::State { generation, state } if generation == self.generation => {
          play_next = state.ended
            && !self.settings.repeat
            && self.state.as_ref().is_none_or(|old| !old.ended);
          self.loading = state.loading;
          self.state = Some(state);
        }
        Event::Subtitle {
          generation,
          track,
          activate,
        } if generation == self.generation => {
          self.subtitle_track = track;
          if activate && self.subtitle_track.is_some() {
            self.settings.subtitles.enabled = true;
          }
          self.notice = Some("Subtitle selection updated".into());
        }
        Event::Warning {
          generation,
          message,
        } if generation == self.generation => self.notice = Some(message),
        Event::Error {
          generation,
          message,
        } if generation == self.generation => {
          self.error = Some(message);
          self.loading = false;
        }
        _ => continue,
      }
      changed = true;
    }
    let mut latest = None;
    while let Ok(frame) = self.engine.frames.try_recv() {
      if frame.generation == self.generation {
        latest = Some(frame.frame);
      }
    }
    if let Some(frame) = latest {
      if let Err(error) = self.video_view.update(self.gpu.as_ref(), frame, window) {
        self.notice = Some(format!("GPU output unavailable: {error:#}"));
        self.issue(Operation::Software, window);
      }
      changed = true;
    }
    if self.video_view.has_pending_frame() {
      if let Err(error) = self.video_view.poll(self.gpu.as_ref(), window) {
        self.notice = Some(format!("GPU readback failed: {error:#}"));
        self.issue(Operation::Software, window);
      }
      changed = true;
    }
    while let Ok(thumbnail) = self.previewer.images.try_recv() {
      if thumbnail.id == self.thumbnail_id
        && self.hover_time.is_some()
        && let Some(buffer) =
          RgbaImage::from_raw(thumbnail.width, thumbnail.height, thumbnail.pixels)
      {
        if let Some((image, _)) = self.thumbnail.take() {
          let _ = window.drop_image(image);
        }
        self.thumbnail = Some((
          Arc::new(RenderImage::new(vec![ImageFrame::new(buffer)])),
          thumbnail.time,
        ));
        changed = true;
      }
    }
    if changed && self.scrub_target.is_none() {
      let fraction = self
        .state
        .as_ref()
        .map(|state| (state.time.as_secs_f64() / state.duration.as_secs_f64()) as f32)
        .unwrap_or_default();
      self.seek_slider.update(cx, |slider, cx| {
        slider.set_value(fraction.clamp(0.0, 1.0), window, cx)
      });
    }
    if play_next {
      self.navigate(1, window);
    }
    if self.path.is_some() && self.last_save.elapsed() >= Duration::from_secs(10) {
      self.save();
    }
    changed
  }

  fn seek_to(&mut self, position: Duration, window: &mut Window) {
    if let Some(state) = &mut self.state {
      state.time = position.min(state.duration);
    }
    self.issue(Operation::Seek(position), window);
  }

  fn toggle_paused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.loading || self.choosing_file {
      return;
    }
    if let Some(state) = &mut self.state {
      if state.ended {
        state.paused = false;
        self.seek_to(Duration::ZERO, window);
        self.engine.send(Operation::Pause(false));
      } else {
        state.paused = !state.paused;
        self.engine.send(Operation::Pause(state.paused));
      }
      cx.notify();
    }
  }

  fn update_volume(&self) {
    self.engine.send(Operation::Volume(if self.settings.muted {
      0.0
    } else {
      self.settings.volume
    }));
  }

  fn set_speed(&mut self, speed: f32, window: &mut Window) {
    if self.loading || !PLAYBACK_SPEEDS.contains(&speed) || speed == self.settings.speed {
      return;
    }
    self.settings.speed = speed;
    self.generation = self.engine.send(Operation::Speed(speed));
    self.loading = self.state.is_some();
    self.save();
    let _ = window;
  }

  fn navigate(&mut self, delta: isize, window: &mut Window) {
    if self.loading {
      return;
    }
    let current = self
      .path
      .as_ref()
      .and_then(|path| self.playlist.iter().position(|item| item == path));
    if let Some(index) = current.and_then(|index| index.checked_add_signed(delta))
      && let Some(path) = self.playlist.get(index).cloned()
    {
      self.open_path(&path, None, window);
    }
  }

  fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    if event.keystroke.key == "escape" && window.is_fullscreen() {
      window.toggle_fullscreen();
      cx.stop_propagation();
      return;
    }
    if self.choosing_file {
      return;
    }
    match event.keystroke.key.as_str() {
      "escape" if window.is_fullscreen() => window.toggle_fullscreen(),
      "f" if !event.is_held => window.toggle_fullscreen(),
      "space" if !event.is_held && self.focus.is_focused(window) => self.toggle_paused(window, cx),
      "left" | "right" => {
        if let Some(state) = &self.state {
          let step = Duration::from_secs(if event.keystroke.modifiers.shift {
            30
          } else {
            5
          });
          self.seek_to(
            if event.keystroke.key == "left" {
              state.time.saturating_sub(step)
            } else {
              state.time.saturating_add(step).min(state.duration)
            },
            window,
          );
        }
      }
      "up" | "down" => {
        let delta = if event.keystroke.key == "up" {
          0.05
        } else {
          -0.05
        };
        self.settings.volume = (self.settings.volume + delta).clamp(0.0, 1.0);
        self.volume_slider.update(cx, |slider, cx| {
          slider.set_value(self.settings.volume, window, cx)
        });
        self.update_volume();
        self.save();
      }
      "m" if !event.is_held => {
        self.settings.muted = !self.settings.muted;
        self.update_volume();
        self.save();
      }
      "." | "," if !self.loading && self.state.is_some() => {
        self.issue(Operation::Step(event.keystroke.key == "."), window)
      }
      _ => return,
    }
    cx.stop_propagation();
    cx.notify();
  }

  fn screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.choosing_file {
      return;
    }
    let Some(image) = self.video_view.snapshot() else {
      return;
    };
    let Some((width, height)) = self.video_view.size() else {
      return;
    };
    let was_paused = self.state.as_ref().is_none_or(|state| state.paused);
    self.choosing_file = true;
    self.engine.send(Operation::Pause(true));
    let selected = rfd::AsyncFileDialog::new()
      .set_parent(window)
      .set_title("Save current frame")
      .set_file_name("video-frame.png")
      .add_filter("PNG image", &["png"])
      .save_file();
    cx.spawn_in(window, async move |this, cx| {
      let picked = selected.await;
      let result = if let Some(file) = picked {
        Some(
          tokio::task::unconstrained(tokio::task::spawn_blocking(move || -> Result<()> {
            let mut pixels = image
              .as_bytes(0)
              .context("frame no longer available")?
              .to_vec();
            for pixel in pixels.as_chunks_mut::<4>().0 {
              pixel.swap(0, 2);
            }
            image::save_buffer_with_format(
              file.path(),
              &pixels,
              width,
              height,
              image::ColorType::Rgba8,
              image::ImageFormat::Png,
            )?;
            Ok(())
          }))
          .await,
        )
      } else {
        None
      };
      let _ = this.update_in(cx, |this, _, cx| {
        this.choosing_file = false;
        if let Some(result) = result {
          match result {
            Ok(Ok(())) => {
              this.notice = Some("Screenshot saved (video frame without subtitle overlay)".into())
            }
            other => this.error = Some(format!("Screenshot failed: {other:?}")),
          }
        }
        this.engine.send(Operation::Pause(was_paused));
        cx.notify();
      });
    })
    .detach();
  }

  fn timeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let (position, duration) = self
      .state
      .as_ref()
      .map(|state| (self.scrub_target.unwrap_or(state.time), state.duration))
      .unwrap_or_default();
    let player = cx.weak_entity();
    div()
      .flex()
      .items_center()
      .gap_3()
      .px_3()
      .py_2()
      .bg(rgb(0x1b1e26))
      .child(div().text_sm().child(format_time(position)))
      .child(
        div()
          .id("timeline-hover")
          .relative()
          .flex_1()
          .min_w_0()
          .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
            if this.loading || this.timeline_bounds.size.width <= px(0.0) {
              return;
            }
            let Some((path, state)) = this.path.as_ref().zip(this.state.as_ref()) else {
              return;
            };
            let fraction = ((event.position.x - this.timeline_bounds.origin.x)
              / this.timeline_bounds.size.width)
              .clamp(0.0, 1.0);
            let time = state.duration.mul_f64(f64::from(fraction));
            this.hover_time = Some(time);
            if this.last_thumbnail.elapsed() > Duration::from_millis(350) {
              this.thumbnail_id += 1;
              this
                .previewer
                .request(this.thumbnail_id, path.clone(), time);
              this.last_thumbnail = Instant::now();
            }
            cx.notify();
          }))
          .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
            if !hovered {
              this.hover_time = None;
              this.thumbnail_id += 1;
              if let Some((image, _)) = this.thumbnail.take() {
                let _ = window.drop_image(image);
              }
              cx.notify();
            }
          }))
          .child(
            Slider::new(&self.seek_slider)
              .horizontal()
              .accessibility_label("Playback position")
              .disabled(self.state.is_none() || self.loading || self.choosing_file),
          )
          .child(
            canvas(
              move |bounds, _, cx| {
                let _ = player.update(cx, |this, _| this.timeline_bounds = bounds);
              },
              |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
          )
          .when_some(self.hover_time, |element, time| {
            element.child(
              div()
                .absolute()
                .bottom(px(28.0))
                .left_0()
                .p_2()
                .bg(rgb(0x151820))
                .rounded_md()
                .when_some(self.thumbnail.clone(), |element, (image, sampled)| {
                  element.child(
                    div()
                      .child(img(image).w(px(240.0)))
                      .child(format_time(sampled)),
                  )
                })
                .child(format!("Jump to {}", format_time(time))),
            )
          }),
      )
      .child(div().text_sm().child(format_time(duration)))
  }

  fn controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let player = cx.weak_entity();
    let speed = self.settings.speed;
    let recent = self.settings.recent.clone();
    let recent_player = cx.weak_entity();
    let playlist = self.playlist.clone();
    let list_player = cx.weak_entity();
    let audio_tracks = self
      .info
      .as_ref()
      .map(|info| info.audio_tracks.clone())
      .unwrap_or_default();
    let audio = self.state.as_ref().and_then(|state| state.audio);
    let audio_player = cx.weak_entity();
    let subtitle_tracks = self
      .info
      .as_ref()
      .map(|info| info.subtitle_tracks.clone())
      .unwrap_or_default();
    let subtitle_player = cx.weak_entity();
    let busy = self.loading || self.choosing_file;
    let playing_label = if self.state.as_ref().is_some_and(|state| state.ended) {
      "Replay"
    } else if self.state.as_ref().is_none_or(|state| state.paused) {
      "Play"
    } else {
      "Pause"
    };
    div()
      .flex()
      .flex_wrap()
      .items_center()
      .gap_2()
      .p_3()
      .bg(rgb(0x1b1e26))
      .child(
        Button::new("open")
          .primary()
          .label("Open videos")
          .disabled(self.choosing_file)
          .on_click(
            cx.listener(|this, _, window, cx| this.open_file(OpenFileKind::Video, window, cx)),
          ),
      )
      .child(
        Button::new("recent")
          .label("Recent")
          .disabled(recent.is_empty() || busy)
          .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
            for entry in &recent {
              let path = entry.path.clone();
              let player = recent_player.clone();
              menu = menu.item(
                PopupMenuItem::new(format!(
                  "{}  [{}]",
                  path.display(),
                  format_time(Duration::from_secs_f64(entry.position))
                ))
                .on_click(move |_, window, cx| {
                  let _ = player.update(cx, |this, cx| {
                    this.playlist = vec![path.clone()];
                    this.open_path(&path, None, window);
                    cx.notify();
                  });
                }),
              );
            }
            menu
          }),
      )
      .child(
        Button::new("play-pause")
          .label(playing_label)
          .disabled(self.state.is_none() || busy)
          .on_click(cx.listener(|this, _, window, cx| this.toggle_paused(window, cx))),
      )
      .child(
        Button::new("previous")
          .label("Previous")
          .disabled(self.playlist.len() < 2 || busy)
          .on_click(cx.listener(|this, _, window, cx| {
            this.navigate(-1, window);
            cx.notify();
          })),
      )
      .child(
        Button::new("next")
          .label("Next")
          .disabled(self.playlist.len() < 2 || busy)
          .on_click(cx.listener(|this, _, window, cx| {
            this.navigate(1, window);
            cx.notify();
          })),
      )
      .child(
        Button::new("playlist")
          .label(format!("Playlist ({})", playlist.len()))
          .disabled(playlist.is_empty() || busy)
          .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
            for path in &playlist {
              let path = path.clone();
              let player = list_player.clone();
              menu = menu.item(
                PopupMenuItem::new(
                  path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                )
                .on_click(move |_, window, cx| {
                  let _ = player.update(cx, |this, cx| {
                    this.open_path(&path, None, window);
                    cx.notify();
                  });
                }),
              );
            }
            menu
          }),
      )
      .child(
        Checkbox::new("repeat")
          .label("Loop")
          .checked(self.settings.repeat)
          .on_change(cx.listener(|this, value: &bool, _, cx| {
            this.settings.repeat = *value;
            this.engine.send(Operation::Repeat(*value));
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("speed")
          .label(format!("Speed {speed}×"))
          .disabled(busy)
          .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
            for &value in PLAYBACK_SPEEDS {
              let player = player.clone();
              menu = menu.item(
                PopupMenuItem::new(format!("{value}×"))
                  .checked(value == speed)
                  .on_click(move |_, window, cx| {
                    let _ = player.update(cx, |this, cx| {
                      this.set_speed(value, window);
                      cx.notify();
                    });
                  }),
              );
            }
            menu
          }),
      )
      .child(
        Button::new("fullscreen")
          .label("Fullscreen")
          .on_click(cx.listener(|_, _, window, _| window.toggle_fullscreen())),
      )
      .child(
        Button::new("step")
          .label("Frame +")
          .disabled(self.state.is_none() || busy)
          .on_click(cx.listener(|this, _, window, cx| {
            this.issue(Operation::Step(true), window);
            cx.notify();
          })),
      )
      .child(
        Button::new("screenshot")
          .label("Screenshot")
          .disabled(self.video_view.size().is_none() || busy)
          .on_click(cx.listener(|this, _, window, cx| this.screenshot(window, cx))),
      )
      .child(
        Button::new("audio-track")
          .label("Audio track")
          .disabled(audio_tracks.is_empty() || busy)
          .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
            let player = audio_player.clone();
            menu = menu.item(
              PopupMenuItem::new("No audio")
                .checked(audio.is_none())
                .on_click(move |_, _, cx| {
                  let _ = player.update(cx, |this, cx| {
                    this.generation = this.engine.send(Operation::Audio(None));
                    this.loading = true;
                    cx.notify();
                  });
                }),
            );
            for track in &audio_tracks {
              let player = audio_player.clone();
              let index = track.index;
              menu = menu.item(
                PopupMenuItem::new(track.label.clone())
                  .checked(audio == Some(index))
                  .on_click(move |_, _, cx| {
                    let _ = player.update(cx, |this, cx| {
                      this.generation = this.engine.send(Operation::Audio(Some(index)));
                      this.loading = true;
                      cx.notify();
                    });
                  }),
              );
            }
            menu
          }),
      )
      .child(
        Button::new("open-subtitles")
          .label("Open SRT")
          .disabled(self.state.is_none() || busy)
          .on_click(
            cx.listener(|this, _, window, cx| this.open_file(OpenFileKind::Subtitles, window, cx)),
          ),
      )
      .child(
        Button::new("embedded-subtitles")
          .label("Embedded subtitles")
          .disabled(subtitle_tracks.is_empty() || busy)
          .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
            for track in &subtitle_tracks {
              let player = subtitle_player.clone();
              let index = track.index;
              menu = menu.item(
                PopupMenuItem::new(track.label.clone())
                  .disabled(!track.text)
                  .on_click(move |_, _, cx| {
                    let _ = player.update(cx, |this, cx| {
                      this.engine.send(Operation::Embedded(index));
                      this.notice = Some("Extracting subtitles…".into());
                      cx.notify();
                    });
                  }),
              );
            }
            menu
          }),
      )
      .child(
        Checkbox::new("subtitles")
          .label("Subtitles")
          .checked(self.settings.subtitles.enabled)
          .on_change(cx.listener(|this, value: &bool, _, cx| {
            this.settings.subtitles.enabled = *value;
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("settings")
          .label("Subtitle settings")
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings_open = !this.settings_open;
            cx.notify();
          })),
      )
      .child(
        Checkbox::new("mute")
          .label("Mute")
          .checked(self.settings.muted)
          .on_change(cx.listener(|this, value: &bool, _, cx| {
            this.settings.muted = *value;
            this.update_volume();
            this.save();
            cx.notify();
          })),
      )
      .child(
        div()
          .w(px(120.0))
          .child(Slider::new(&self.volume_slider).accessibility_label("Volume")),
      )
      .child(format!("{:.0}%", self.settings.volume * 100.0))
      .when_some(self.state.as_ref(), |element, state| {
        element.child(div().text_sm().child(format!(
          "{} | frame {} | {}{}",
          if state.ended {
            "Ended"
          } else if state.paused {
            "Paused"
          } else {
            "Playing"
          },
          state.frame_number,
          state.backend,
          if state.audible { "" } else { " | silent" }
        )))
      })
  }

  fn subtitle_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let settings = &self.settings.subtitles;
    div()
      .flex()
      .flex_wrap()
      .items_center()
      .gap_2()
      .p_2()
      .bg(rgb(0x252a34))
      .child(format!("Subtitle delay {:+.1}s", settings.delay))
      .child(
        Button::new("subtitle-earlier")
          .label("Earlier −0.1s")
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles.delay = (this.settings.subtitles.delay - 0.1).max(-60.0);
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("subtitle-later")
          .label("Later +0.1s")
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles.delay = (this.settings.subtitles.delay + 0.1).min(60.0);
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("subtitle-smaller")
          .label("A−")
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles.size = (this.settings.subtitles.size - 2.0).max(12.0);
            this.save();
            cx.notify();
          })),
      )
      .child(format!("{}px", settings.size))
      .child(
        Button::new("subtitle-larger")
          .label("A+")
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles.size = (this.settings.subtitles.size + 2.0).min(64.0);
            this.save();
            cx.notify();
          })),
      )
      .child(
        Checkbox::new("subtitle-outline")
          .label("Outline")
          .checked(settings.outline)
          .on_change(cx.listener(|this, value: &bool, _, cx| {
            this.settings.subtitles.outline = *value;
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("subtitle-opacity")
          .label(format!("Background {:.0}%", settings.opacity * 100.0))
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles.opacity = if this.settings.subtitles.opacity >= 0.95 {
              0.0
            } else {
              (this.settings.subtitles.opacity + 0.2).min(1.0)
            };
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("subtitle-position")
          .label(["Top", "Center", "Bottom"][settings.position as usize])
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles.position = (this.settings.subtitles.position + 1) % 3;
            this.save();
            cx.notify();
          })),
      )
      .child(
        Button::new("subtitle-reset")
          .label("Reset")
          .on_click(cx.listener(|this, _, _, cx| {
            this.settings.subtitles = settings::SubtitleSettings::default();
            this.save();
            cx.notify();
          })),
      )
  }

  fn subtitle_overlay(&self, text: String) -> impl IntoElement {
    let settings = &self.settings.subtitles;
    let label = |text: String, color| {
      div()
        .text_size(px(settings.size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_center()
        .whitespace_normal()
        .text_color(rgb(color))
        .child(text)
    };
    let mut content = div()
      .relative()
      .max_w(relative(0.9))
      .px_3()
      .py_2()
      .rounded_md()
      .bg(rgba((settings.opacity * 255.0) as u32));
    if settings.outline {
      for (horizontal, vertical) in [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
      ] {
        content = content.child(
          div()
            .absolute()
            .left(px(12.0 + horizontal))
            .right(px(12.0 - horizontal))
            .top(px(8.0 + vertical))
            .child(label(text.clone(), 0x000000)),
        );
      }
    }
    content = content.child(label(text, 0xffffff));
    div()
      .absolute()
      .left_0()
      .right_0()
      .flex()
      .justify_center()
      .when(settings.position == 0, |element| element.top(px(24.0)))
      .when(settings.position == 1, |element| {
        element.top(relative(0.45))
      })
      .when(settings.position == 2, |element| element.bottom(px(24.0)))
      .child(content)
  }
}

impl Render for VideoPlayer {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let subtitle = if self.settings.subtitles.enabled && self.video_view.size().is_some() {
      self.state.as_ref().and_then(|state| {
        let seconds = state.time.as_secs_f64() - self.settings.subtitles.delay;
        Duration::try_from_secs_f64(seconds).ok().and_then(|time| {
          self
            .subtitle_track
            .as_ref()
            .and_then(|track| track.text_at(time))
        })
      })
    } else {
      None
    };
    div()
      .id("video-player")
      .size_full()
      .flex()
      .flex_col()
      .bg(rgb(0x000000))
      .text_color(rgb(0xe6e8ef))
      .track_focus(&self.focus)
      .on_key_down(cx.listener(Self::key_down))
      .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
        this.open_files(paths.paths().to_vec(), window);
        cx.notify();
      }))
      .child(
        div()
          .id("video-surface")
          .relative()
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, window, cx| {
              window.focus(&this.focus, cx);
              if event.click_count == 2 {
                window.toggle_fullscreen();
              }
            }),
          )
          .child(self.video_view.element())
          .when_some(subtitle, |element, text| {
            element.child(self.subtitle_overlay(text))
          })
          .when(self.loading, |element| {
            element.child(
              div()
                .absolute()
                .top_0()
                .left_0()
                .p_3()
                .bg(rgba(0x000000bb))
                .child("Loading / seeking…"),
            )
          }),
      )
      .when_some(
        self.error.clone().or_else(|| self.notice.clone()),
        |element, message| {
          element.child(
            div()
              .flex()
              .items_center()
              .gap_3()
              .p_2()
              .bg(rgb(if self.error.is_some() {
                0x48242b
              } else {
                0x263344
              }))
              .child(div().flex_1().text_sm().child(message))
              .child(
                Button::new("dismiss")
                  .label("Dismiss")
                  .on_click(cx.listener(|this, _, _, cx| {
                    this.error = None;
                    this.notice = None;
                    cx.notify();
                  })),
              ),
          )
        },
      )
      .child(self.timeline(cx))
      .when(self.settings_open, |element| {
        element.child(self.subtitle_settings(cx))
      })
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

fn absolute_path(path: PathBuf) -> PathBuf {
  if path.is_absolute() {
    path
  } else {
    std::env::current_dir()
      .unwrap_or_else(|_| PathBuf::from("."))
      .join(path)
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt().init();
  let paths = std::env::args_os()
    .skip(1)
    .map(PathBuf::from)
    .map(absolute_path)
    .collect::<Vec<_>>();
  let gpu = match GpuContext::new().await {
    Ok(gpu) => Some(gpu),
    Err(error) => {
      eprintln!("Hardware decoding unavailable: {error:#}");
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
        size(px(1200.0), px(800.0)),
        cx,
      ))),
      window_min_size: Some(size(px(640.0), px(480.0))),
      ..Default::default()
    };
    if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
      cx.new(|cx| VideoPlayer::new(gpu, paths, window, cx))
    }) {
      eprintln!("Could not open player: {error:#}");
      cx.quit();
    }
    cx.activate(true);
  });
  Ok(())
}

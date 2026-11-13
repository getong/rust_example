use std::{
  collections::VecDeque,
  path::PathBuf,
  process::Command,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
  },
  thread,
  time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

use crate::{
  gpu::GpuContext,
  media::MediaInfo,
  playback::{Frame, Playback},
  process,
  subtitles::SubtitleTrack,
};

pub enum Operation {
  Open {
    path: PathBuf,
    position: Duration,
    subtitle: Option<PathBuf>,
  },
  Seek(Duration),
  Speed(f32),
  Audio(Option<u32>),
  Subtitle(PathBuf),
  Embedded(u32),
  Pause(bool),
  Volume(f32),
  Repeat(bool),
  Step(bool),
  Software,
}

impl Operation {
  fn replaces_request(&self) -> bool {
    matches!(
      self,
      Self::Open { .. }
        | Self::Seek(_)
        | Self::Speed(_)
        | Self::Audio(_)
        | Self::Step(_)
        | Self::Software
    )
  }
}

pub struct State {
  pub time: Duration,
  pub duration: Duration,
  pub frame_number: u64,
  pub backend: &'static str,
  pub audio: Option<u32>,
  pub audible: bool,
  pub paused: bool,
  pub ended: bool,
  pub loading: bool,
}

pub enum Event {
  Loaded {
    generation: u64,
    path: PathBuf,
    info: MediaInfo,
  },
  State {
    generation: u64,
    state: State,
  },
  Subtitle {
    generation: u64,
    track: Option<SubtitleTrack>,
    activate: bool,
  },
  Warning {
    generation: u64,
    message: String,
  },
  Error {
    generation: u64,
    message: String,
  },
}

pub struct EngineFrame {
  pub generation: u64,
  pub frame: Frame,
}

pub struct Engine {
  commands: Sender<(u64, Operation)>,
  pub events: Receiver<Event>,
  pub frames: Receiver<EngineFrame>,
  newest: Arc<AtomicU64>,
}

impl Engine {
  pub fn new(gpu: Option<GpuContext>, speed: f32, volume: f32, repeat: bool) -> Self {
    let (commands, receiver) = mpsc::channel::<(u64, Operation)>();
    let (sender, events) = mpsc::channel();
    let (frame_sender, frames) = mpsc::sync_channel(2);
    let newest = Arc::new(AtomicU64::new(0));
    let current = Arc::clone(&newest);
    thread::spawn(move || {
      let mut playback: Option<Playback> = None;
      let mut path = PathBuf::new();
      let mut generation = 0;
      let mut speed = speed;
      let mut volume = volume;
      let mut repeat = repeat;
      let mut paused = true;
      let mut preview = false;
      let mut preview_started = Instant::now();
      let mut history = VecDeque::new();
      let mut last_tick = Instant::now();
      let mut last_state = Instant::now();
      loop {
        let elapsed = last_tick.elapsed();
        last_tick = Instant::now();
        let mut changed = false;
        loop {
          let (request, operation) = match receiver.try_recv() {
            Ok(command) => command,
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => return,
          };
          if let Operation::Speed(value) = &operation {
            speed = *value;
          }
          if operation.replaces_request() && request != current.load(Ordering::Relaxed) {
            continue;
          }
          if operation.replaces_request() {
            generation = request;
          }
          changed = true;
          if let Some(playback) = &mut playback
            && !matches!(
              operation,
              Operation::Pause(_) | Operation::Volume(_) | Operation::Repeat(_)
            )
          {
            playback.set_paused(true);
          }
          let result = (|| -> Result<()> {
            match operation {
              Operation::Open {
                path: selected,
                position,
                subtitle,
              } => {
                let mut opened = Playback::new(gpu.as_ref(), &selected)?;
                opened.set_speed(speed);
                opened.set_volume(volume);
                if !position.is_zero() {
                  opened.seek(
                    gpu.as_ref(),
                    position.min(opened.duration.saturating_sub(Duration::from_millis(1))),
                  )?;
                }
                let _ = sender.send(Event::Loaded {
                  generation,
                  path: selected.clone(),
                  info: opened.info.clone(),
                });
                let subtitles = if let Some(subtitle) = subtitle.filter(|path| path.is_file()) {
                  SubtitleTrack::load(&subtitle).map(Some)
                } else {
                  SubtitleTrack::for_video(&selected)
                };
                match subtitles {
                  Ok(track) => {
                    let _ = sender.send(Event::Subtitle {
                      generation,
                      track,
                      activate: false,
                    });
                  }
                  Err(error) => {
                    let _ = sender.send(Event::Warning {
                      generation,
                      message: format!("Subtitles unavailable: {error:#}"),
                    });
                  }
                }
                path = selected;
                playback = Some(opened);
                paused = false;
                preview = true;
                history.clear();
              }
              Operation::Seek(position) => {
                if let Some(playback) = &mut playback {
                  playback.seek(gpu.as_ref(), position)?;
                  preview = true;
                  history.clear();
                }
              }
              Operation::Speed(selected) => {
                speed = selected;
                if let Some(playback) = &mut playback {
                  playback.set_speed(speed);
                }
              }
              Operation::Audio(track) => {
                if let Some(playback) = &mut playback {
                  playback.select_audio(track);
                }
              }
              Operation::Subtitle(path) => {
                let track = SubtitleTrack::load(&path)?;
                let _ = sender.send(Event::Subtitle {
                  generation,
                  track: Some(track),
                  activate: true,
                });
              }
              Operation::Embedded(track) => {
                let result = process::output(
                  Command::new("ffmpeg")
                    .args(["-nostdin", "-v", "error", "-i"])
                    .arg(&path)
                    .args(["-map", &format!("0:{track}"), "-f", "srt", "pipe:1"]),
                  Duration::from_secs(20),
                )?;
                ensure!(
                  result.status.success(),
                  "could not extract text subtitles: {}",
                  String::from_utf8_lossy(&result.stderr)
                );
                let text =
                  String::from_utf8(result.stdout).context("invalid embedded subtitle text")?;
                let track = SubtitleTrack::from_srt(&path, &text)?;
                let _ = sender.send(Event::Subtitle {
                  generation,
                  track: Some(track),
                  activate: true,
                });
              }
              Operation::Pause(value) => paused = value,
              Operation::Volume(value) => {
                volume = value;
                if let Some(playback) = &mut playback {
                  playback.set_volume(volume);
                }
              }
              Operation::Repeat(value) => repeat = value,
              Operation::Step(forward) => {
                if let Some(playback) = &mut playback {
                  paused = true;
                  if !forward {
                    let target = history
                      .iter()
                      .rev()
                      .copied()
                      .find(|time| *time < playback.time)
                      .unwrap_or_else(|| {
                        playback
                          .time
                          .saturating_sub(Duration::from_secs_f64(1.0 / playback.info.fps))
                      });
                    playback.seek(gpu.as_ref(), target)?;
                  }
                  preview = true;
                }
              }
              Operation::Software => {
                if let Some(playback) = &mut playback {
                  playback.force_software()?;
                  preview = true;
                }
              }
            }
            Ok(())
          })();
          if let Err(error) = result {
            preview = false;
            let _ = sender.send(Event::Error {
              generation,
              message: format!("{error:#}"),
            });
          }
          if let Some(playback) = &mut playback {
            playback.set_paused(paused || preview);
          }
          if preview {
            preview_started = Instant::now();
          }
          last_tick = Instant::now();
        }
        if let Some(playback) = &mut playback {
          if preview && preview_started.elapsed() > Duration::from_secs(10) {
            preview = false;
            paused = true;
            playback.set_paused(true);
            changed = true;
            let _ = sender.send(Event::Error {
              generation,
              message: "Timed out waiting for a decoded video frame".into(),
            });
          }
          let result = if preview {
            playback.preview_frame()
          } else if !paused {
            playback.next_frame(if changed { Duration::ZERO } else { elapsed })
          } else {
            Ok(None)
          };
          match result {
            Ok(Some(frame)) => {
              let pts = frame.pts();
              if history.back() != Some(&pts) {
                history.push_back(pts);
                if history.len() > 120 {
                  history.pop_front();
                }
              }
              let _ = frame_sender.try_send(EngineFrame { generation, frame });
              if preview {
                preview = false;
                playback.set_paused(paused);
                changed = true;
              }
            }
            Ok(None) => {}
            Err(error) => {
              paused = true;
              preview = false;
              playback.set_paused(true);
              let _ = sender.send(Event::Error {
                generation,
                message: format!("Playback failed: {error:#}"),
              });
              changed = true;
            }
          }
          let ended = playback.time >= playback.duration;
          if ended && !paused {
            if repeat {
              if let Err(error) = playback.seek(gpu.as_ref(), Duration::ZERO) {
                paused = true;
                let _ = sender.send(Event::Error {
                  generation,
                  message: format!("Could not repeat: {error:#}"),
                });
              } else {
                preview = true;
                preview_started = Instant::now();
                history.clear();
              }
            } else {
              paused = true;
            }
            playback.set_paused(true);
            changed = true;
          }
          for message in playback.take_warnings() {
            let _ = sender.send(Event::Warning {
              generation,
              message,
            });
          }
          if changed || (!paused && last_state.elapsed() >= Duration::from_millis(40)) {
            let _ = sender.send(Event::State {
              generation,
              state: State {
                time: playback.time,
                duration: playback.duration,
                frame_number: playback.frame_number,
                backend: playback.backend_name(),
                audio: playback.selected_audio,
                audible: playback.has_audio(),
                paused,
                ended: playback.time >= playback.duration,
                loading: preview,
              },
            });
            last_state = Instant::now();
          }
        }
        thread::sleep(Duration::from_millis(8));
      }
    });
    Self {
      commands,
      events,
      frames,
      newest,
    }
  }

  pub fn send(&self, operation: Operation) -> u64 {
    let generation = if operation.replaces_request() {
      self.newest.fetch_add(1, Ordering::Relaxed) + 1
    } else {
      self.newest.load(Ordering::Relaxed)
    };
    let _ = self.commands.send((generation, operation));
    generation
  }
}

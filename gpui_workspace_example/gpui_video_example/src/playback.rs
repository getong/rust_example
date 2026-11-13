use std::{
  collections::VecDeque,
  path::{Path, PathBuf},
  sync::mpsc::Receiver,
  time::Duration,
};

use anyhow::{Context, Result};
use gpu_video::{
  EncodedInputChunk, OutputFrame, VideoDeviceExt, VideoTexture, WgpuTexturesDecoderH264,
  parameters::{DecoderParameters, InputBytestreamFormatH264},
};

use crate::{
  audio::AudioPlayback,
  demuxer::Mp4Demuxer,
  gpu::GpuContext,
  media::MediaInfo,
  software::{SoftwareFrame, SoftwarePlayback},
};

pub const PLAYBACK_SPEEDS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0];

pub enum Frame {
  Gpu(OutputFrame<VideoTexture>),
  Software(SoftwareFrame),
}

impl Frame {
  pub fn pts(&self) -> Duration {
    match self {
      Self::Gpu(frame) => Duration::from_micros(frame.metadata.pts.unwrap_or(0)),
      Self::Software(frame) => frame.pts,
    }
  }
}

enum Backend {
  Gpu(Box<GpuPlayback>),
  Software(SoftwarePlayback),
}

pub struct Playback {
  backend: Backend,
  audio: Option<AudioPlayback>,
  path: PathBuf,
  speed: f32,
  paused: bool,
  volume: f32,
  pub info: MediaInfo,
  pub selected_audio: Option<u32>,
  warnings: Vec<String>,
  pub duration: Duration,
  pub frame_number: u64,
  pub time: Duration,
}

impl Playback {
  pub fn new(gpu: Option<&GpuContext>, path: &Path) -> Result<Self> {
    let info = MediaInfo::probe(path)?;
    let backend = Self::open_backend(gpu, path, Duration::ZERO, &info)?;
    let selected_audio = info.audio_tracks.first().map(|track| track.index);
    let mut playback = Self {
      backend,
      audio: None,
      path: path.to_path_buf(),
      speed: 1.0,
      paused: true,
      volume: 1.0,
      duration: info.duration,
      info,
      selected_audio,
      warnings: Vec::new(),
      frame_number: 0,
      time: Duration::ZERO,
    };
    playback.select_audio(selected_audio);
    Ok(playback)
  }

  fn open_backend(
    gpu: Option<&GpuContext>,
    path: &Path,
    position: Duration,
    info: &MediaInfo,
  ) -> Result<Backend> {
    if info.hardware_compatible
      && let Some(gpu) = gpu
    {
      let hardware = (|| -> Result<GpuPlayback> {
        let (demuxer, _) = Mp4Demuxer::open_video(path)?;
        let mut playback = GpuPlayback::new(
          &gpu.device,
          &gpu.queue,
          demuxer.context("not an H.264 MP4")?,
        )?;
        playback.demuxer.seek(position)?;
        playback.seek_target = Some(position);
        Ok(playback)
      })();
      match hardware {
        Ok(playback) => return Ok(Backend::Gpu(Box::new(playback))),
        Err(error) => eprintln!("Using FFmpeg video decoding: {error:#}"),
      }
    }
    Ok(Backend::Software(SoftwarePlayback::new(
      path, position, info,
    )?))
  }

  pub fn seek(&mut self, gpu: Option<&GpuContext>, position: Duration) -> Result<()> {
    let position = position.min(self.duration);
    let backend = Self::open_backend(gpu, &self.path, position, &self.info)?;
    self.backend = backend;
    self.time = position;
    self.frame_number = 0;
    self.rebuild_audio();
    Ok(())
  }

  pub fn next_frame(&mut self, elapsed: Duration) -> Result<Option<Frame>> {
    if let Some(audio) = &self.audio
      && let Err(error) = audio.check_error()
    {
      self
        .warnings
        .push(format!("Audio disabled; continuing video: {error:#}"));
      self.audio = None;
    }
    let time = self
      .audio
      .as_ref()
      .and_then(AudioPlayback::position)
      .unwrap_or_else(|| {
        self
          .time
          .saturating_add(elapsed.mul_f64(f64::from(self.speed)))
      });
    self.time = self.time.max(time).min(self.duration);
    let mut latest = None;
    for _ in 0 .. 8 {
      let frame = self.decode_frame(self.time)?;
      let Some(frame) = frame else {
        break;
      };
      self.frame_number += 1;
      latest = Some(frame);
    }
    Ok(latest)
  }

  pub fn set_paused(&mut self, paused: bool) {
    self.paused = paused;
    if let Some(audio) = &mut self.audio {
      audio.set_paused(paused);
    }
  }

  pub fn set_volume(&mut self, volume: f32) {
    self.volume = volume;
    if let Some(audio) = &self.audio {
      audio.set_volume(volume);
    }
  }

  pub fn set_speed(&mut self, speed: f32) {
    if !PLAYBACK_SPEEDS.contains(&speed) || self.speed == speed {
      return;
    }
    self.speed = speed;
    self.rebuild_audio();
  }

  fn decode_frame(&mut self, time: Duration) -> Result<Option<Frame>> {
    match &mut self.backend {
      Backend::Gpu(playback) => match playback.next_frame(time) {
        Ok(frame) => Ok(frame.map(Frame::Gpu)),
        Err(error) => {
          self.warnings.push(format!(
            "Hardware decoding failed; switched to FFmpeg: {error:#}"
          ));
          self.backend =
            Backend::Software(SoftwarePlayback::new(&self.path, self.time, &self.info)?);
          Ok(None)
        }
      },
      Backend::Software(playback) => playback
        .next_frame(time)
        .map(|frame| frame.map(Frame::Software)),
    }
  }

  pub fn preview_frame(&mut self) -> Result<Option<Frame>> {
    let frame = self.decode_frame(Duration::MAX)?;
    if let Some(frame) = &frame {
      self.time = self.time.max(frame.pts()).min(self.duration);
      self.frame_number += 1;
    }
    Ok(frame)
  }

  pub fn force_software(&mut self) -> Result<()> {
    self.backend = Backend::Software(SoftwarePlayback::new(&self.path, self.time, &self.info)?);
    self
      .warnings
      .push("GPU presentation failed; using FFmpeg".into());
    Ok(())
  }

  pub fn select_audio(&mut self, track: Option<u32>) {
    self.selected_audio = track.filter(|index| {
      self
        .info
        .audio_tracks
        .iter()
        .any(|track| track.index == *index)
    });
    self.audio = None;
    if let Some(track) = self.selected_audio {
      match AudioPlayback::new(&self.path, track, self.time, self.speed) {
        Ok(mut audio) => {
          audio.set_volume(self.volume);
          audio.set_paused(self.paused);
          self.audio = Some(audio);
        }
        Err(error) => self
          .warnings
          .push(format!("Audio unavailable; continuing silently: {error:#}")),
      }
    }
  }

  fn rebuild_audio(&mut self) {
    if self.time >= self.duration {
      self.audio = None;
      return;
    }
    if let (Some(audio), Some(track)) = (&mut self.audio, self.selected_audio)
      && let Err(error) = audio.seek(&self.path, track, self.time, self.speed)
    {
      self
        .warnings
        .push(format!("Audio disabled; continuing video: {error:#}"));
      self.audio = None;
    }
  }

  pub fn take_warnings(&mut self) -> Vec<String> {
    std::mem::take(&mut self.warnings)
  }

  pub fn has_audio(&self) -> bool {
    self.audio.is_some()
  }

  pub fn backend_name(&self) -> &'static str {
    match &self.backend {
      Backend::Gpu(_) => "GPU",
      Backend::Software(_) => "FFmpeg",
    }
  }
}

struct GpuPlayback {
  demuxer: Mp4Demuxer,
  decoder: WgpuTexturesDecoderH264,
  frame_receiver: Receiver<OutputFrame<VideoTexture>>,
  eof: bool,
  decoded_frames: VecDeque<OutputFrame<VideoTexture>>,
  seek_target: Option<Duration>,
  seek_frame: Option<OutputFrame<VideoTexture>>,
}

impl GpuPlayback {
  const MAX_DECODED_FRAMES: usize = 3;

  fn new(device: &wgpu::Device, queue: &wgpu::Queue, demuxer: Mp4Demuxer) -> Result<Self> {
    let (frame_sender, frame_receiver) = std::sync::mpsc::channel();
    let parameters = DecoderParameters {
      bytestream_format: InputBytestreamFormatH264::Avcc {
        prefix_length: demuxer.prefix_length,
      },
      ..Default::default()
    };
    let decoder = device
      .video()
      .context("The GPU device does not support video decoding")?
      .create_wgpu_textures_decoder_h264(queue, parameters, move |frame| {
        let _ = frame_sender.send(frame);
      })
      .context("Failed to create the H.264 decoder")?;

    Ok(Self {
      demuxer,
      decoder,
      frame_receiver,
      eof: false,
      decoded_frames: VecDeque::new(),
      seek_target: None,
      seek_frame: None,
    })
  }

  fn next_frame(&mut self, time: Duration) -> Result<Option<OutputFrame<VideoTexture>>> {
    if let Some(target) = self.seek_target {
      for _ in 0 .. 64 {
        let Some(frame) = self.decode_next()? else {
          if self.eof {
            self.seek_target = None;
            return Ok(self.seek_frame.take());
          }
          return Ok(None);
        };
        if Duration::from_micros(
          frame
            .metadata
            .pts
            .context("Missing decoded frame timestamp")?,
        ) >= target
        {
          self.seek_target = None;
          self.seek_frame = None;
          return Ok(Some(frame));
        }
        self.seek_frame = Some(frame);
      }
      return Ok(None);
    }
    while self.decoded_frames.len() < Self::MAX_DECODED_FRAMES {
      let Some(frame) = self.decode_next()? else {
        break;
      };
      self.decoded_frames.push_back(frame);
    }

    let is_due = self
      .decoded_frames
      .front()
      .is_some_and(|f| Duration::from_micros(f.metadata.pts.unwrap()) <= time);
    if !is_due {
      return Ok(None);
    }
    Ok(self.decoded_frames.pop_front())
  }

  fn decode_next(&mut self) -> Result<Option<OutputFrame<VideoTexture>>> {
    loop {
      if let Ok(frame) = self.frame_receiver.try_recv() {
        return Ok(Some(frame));
      }

      if self.eof {
        return Ok(None);
      }

      match self.demuxer.next_chunk()? {
        Some(chunk) => self
          .decoder
          .decode(EncodedInputChunk {
            data: &chunk.data,
            pts: Some(chunk.pts.as_micros() as u64),
          })
          .context("Failed to decode an H.264 frame")?,
        None => {
          self
            .decoder
            .flush()
            .context("Failed to flush the H.264 decoder")?;
          self.eof = true;
        }
      }
    }
  }
}
